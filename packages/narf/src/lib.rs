//! NARF (NixOS Advanced Rebuild Flake) - Rebuilding nix flakes and creating commits.

pub mod cli;
pub mod flake;
mod git;
mod nix;
mod stages;

use std::{ffi::OsString, process::Command};

use clap::Parser;

/// Parses `args` (including the program name) and runs narf, returning the
/// exit code: 0 on success, 1 if a step failed, 2 for invalid arguments.
pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = match cli::Args::try_parse_from(args) {
        Ok(args) => args,
        Err(err) => {
            let _ = err.print();
            return err.exit_code();
        }
    };

    match stages::run(&args) {
        Ok(()) => {
            let _ = cliclack::outro("Finished successfully!");
            0
        }
        Err(err) => {
            let _ = cliclack::outro_cancel(err);
            1
        }
    }
}

/// Asks a yes/no question; aborting or a missing terminal count as "no".
fn confirm(default: bool, message: &str) -> bool {
    cliclack::confirm(message)
        .initial_value(default)
        .interact()
        .unwrap_or(false)
}

/// Runs a command with inherited stdio and reports whether it succeeded.
fn succeeds(cmd: &mut Command) -> bool {
    cmd.status().is_ok_and(|status| status.success())
}
