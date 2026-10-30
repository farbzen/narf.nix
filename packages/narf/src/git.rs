//! Git operations on the flake repository.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::succeeds;

fn git(path: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(path);
    cmd
}

/// Stdout of a command with trailing newlines stripped, like `$(...)`.
fn capture(cmd: &mut Command) -> String {
    cmd.stderr(Stdio::null())
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim_end_matches('\n')
                .to_owned()
        })
        .unwrap_or_default()
}

pub fn upstream(path: &Path) -> Option<String> {
    let branch = capture(git(path).args(["rev-parse", "--abbrev-ref", "@{upstream}"]));
    let branch = branch.trim();
    (!branch.is_empty()).then(|| branch.to_owned())
}

pub fn pull(path: &Path) -> bool {
    succeeds(git(path).arg("pull"))
}

pub fn stage_all(path: &Path) -> bool {
    succeeds(git(path).args(["add", "."]))
}

pub fn staged_diff(path: &Path) -> String {
    capture(
        Command::new("git")
            .arg("--no-pager")
            .arg("-C")
            .arg(path)
            .args(["diff", "-U0", "--staged"]),
    )
}

pub fn count_unpushed(path: &Path, branch: Option<&str>) -> usize {
    branch
        .and_then(|branch| {
            capture(git(path).args(["rev-list", "--count", &format!("{branch}..HEAD")]))
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

/// The messages of all commits not yet pushed to `branch`.
pub fn unpushed(path: &Path, branch: &str) -> Option<String> {
    git(path)
        .args(["log", &format!("{branch}..HEAD"), "--format=%B"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned()
        })
}

pub fn latest_message(path: &Path) -> String {
    capture(git(path).args(["log", "-1", "--pretty=%B"]))
}

pub fn commit(path: &Path, message: &str) -> bool {
    succeeds(git(path).args(["commit", "-m", message]))
}

/// Undoes the last `count` commits, keeping their changes staged.
pub fn squash(path: &Path, count: usize) -> bool {
    succeeds(git(path).args(["reset", "--soft", &format!("HEAD~{count}")]))
}

pub fn push(path: &Path) -> bool {
    succeeds(git(path).arg("push"))
}
