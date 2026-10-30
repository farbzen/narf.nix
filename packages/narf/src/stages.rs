//! The steps of a narf run: prepare, rebuild and commit, push.

use anyhow::{Result, bail, ensure};

use crate::{
    cli::Args,
    confirm,
    flake::{self, Flake},
    git, nix,
};

const REBUILD_FAILED: &str = "There was an error while rebuilding! Exiting...";
const COMMIT_FAILED: &str = "There was an error while commiting to the git repo! Exiting...";
const PUSH_FAILED: &str = "There was an error while trying to push! Exiting...";

pub fn run(args: &Args) -> Result<()> {
    let flake = flake::init(args)?;

    prep(args, &flake)?;
    rebuild(args, &flake)?;

    if !args.push || args.dry || args.build_only {
        return Ok(());
    }

    push(args, &flake)
}

/// Pulls, updates inputs and stages all changes.
fn prep(args: &Args, flake: &Flake) -> Result<()> {
    ensure!(
        !args.pull || git::pull(&flake.path),
        "There was an error while trying to pull! Exiting..."
    );

    if args.update || !args.update_input.is_empty() {
        ensure!(
            nix::update_inputs(&flake.path, &args.update_input),
            "There was an error while updating the inputs! Exiting..."
        );
    }

    ensure!(
        git::stage_all(&flake.path),
        "There was an error while staging all changes! Exiting..."
    );
    Ok(())
}

/// Rebuilds if anything changed (or on request) and commits the new generation.
fn rebuild(args: &Args, flake: &Flake) -> Result<()> {
    let diff = git::staged_diff(&flake.path);

    if diff.is_empty() && !args.pull {
        let _ = cliclack::log::info("No changes found!");

        if !args.skip_unchanged
            && (args.yes || confirm(false, "Would you like to run the rebuild anyway?"))
        {
            ensure!(nix::rebuild(flake, args), REBUILD_FAILED);
        }
        return Ok(());
    }

    if !diff.is_empty() {
        let _ = cliclack::note("Staged changes", &diff);
    }

    ensure!(nix::rebuild(flake, args), REBUILD_FAILED);

    if !args.dry && !args.build_only && !diff.is_empty() {
        let mut message = nix::commit_message(&flake.host);
        if let Some(comment) = &args.comment {
            message.push('\n');
            message.push_str(comment);
        }

        ensure!(git::commit(&flake.path, &message), COMMIT_FAILED);

        let _ = cliclack::log::success(format!(
            "New generation committed!\nThere are currently {} unpushed commits!",
            git::count_unpushed(&flake.path, flake.branch.as_deref())
        ));
    }
    Ok(())
}

/// Pushes unpushed commits, squashing them into one first unless disabled.
fn push(args: &Args, flake: &Flake) -> Result<()> {
    let path = &flake.path;
    let Some(branch) = flake.branch.as_deref() else {
        bail!("There was an error while getting the branch! Exiting...");
    };
    let count = git::count_unpushed(path, Some(branch));

    if count < 1 {
        let _ = cliclack::log::info("Nothing to push! Exiting...");
        return Ok(());
    }

    if count == 1 {
        let _ = cliclack::log::step("Pushing latest commit...");
        ensure!(git::push(path), PUSH_FAILED);
        return Ok(());
    }

    let Some(messages) = git::unpushed(path, branch) else {
        bail!("There was an error while getting unpushed_commits! Exiting...");
    };
    let _ = cliclack::note(format!("{count} new commits found"), messages);

    if args.dont_squash_commits {
        ensure!(git::push(path), PUSH_FAILED);
        return Ok(());
    }

    if !(args.yes
        || confirm(
            true,
            &format!("Would you like to squash the {count} new commits?"),
        ))
    {
        return Ok(());
    }

    let latest_message = git::latest_message(path);

    ensure!(
        git::squash(path, count),
        "There was an error while squashing commits! Exiting..."
    );

    if !git::commit(path, &latest_message) {
        if git::staged_diff(path).is_empty() {
            let _ = cliclack::log::info(
                "There was no diff after squashing! Seems like you really came full circle...",
            );
            return Ok(());
        }
        bail!(COMMIT_FAILED);
    }

    ensure!(git::push(path), PUSH_FAILED);
    Ok(())
}
