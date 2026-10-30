//! Calls to nix, nh and nixos-rebuild.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::{cli::Args, flake::Flake, succeeds};

pub fn rebuild(flake: &Flake, args: &Args) -> bool {
    let mut cmd = Command::new("nh");
    cmd.args(["os", flake.action]).arg(&flake.path).args([
        "-H",
        &flake.host,
        "--accept-flake-config",
    ]);

    if args.dry {
        cmd.arg("--dry");
    }
    if args.trace {
        cmd.arg("--show-trace");
    }

    succeeds(&mut cmd)
}

/// Updates the given inputs, or all of them if `inputs` is empty.
pub fn update_inputs(path: &Path, inputs: &[String]) -> bool {
    succeeds(
        Command::new("nix")
            .args(["flake", "update", "--flake"])
            .arg(path)
            .args(inputs),
    )
}

/// Describes the current generation, e.g.
/// `rebuild(host): Generation 42\n\nNixOS 26.11...\nLinux 7.2...\n\nBuilt: 2026-10-22T09:00:00`.
pub fn commit_message(host: &str) -> String {
    let Ok(output) = Command::new("nixos-rebuild")
        .arg("list-generations")
        .stderr(Stdio::null())
        .output()
    else {
        return String::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|fields| fields.len() == 8 && fields[7] == "True")
        .map(|f| {
            format!(
                "rebuild({host}): Generation {}\n\nNixOS {}\nLinux {}\n\nBuilt: {}T{}",
                f[0], f[3], f[4], f[1], f[2]
            )
        })
        .unwrap_or_default()
}
