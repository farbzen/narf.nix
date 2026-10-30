//! Information about the flake repository narf will operate on.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use narf::flake::{self, default_host, expand_tilde, home};

pub struct Status {
    pub flake: Result<PathBuf, String>,
    pub default_host: String,
    pub upstream: Option<String>,
    pub unpushed: Option<usize>,
    pub head: Option<String>,
    pub changes: Vec<String>,
    pub inputs: Vec<String>,
}

/// Shortens a path below $HOME to `~/...` for display.
pub fn display_path(path: &Path) -> String {
    match home().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Resolves the flake like narf: the `--flake` value if given, otherwise
/// the path saved in ~/.config/narf/flake.
fn resolve_flake(flake_field: &str) -> Result<PathBuf, String> {
    let raw = if flake_field.trim().is_empty() {
        flake::saved_flake().ok_or_else(|| {
            format!(
                "no flake configured ({} missing)",
                flake::config_file().display()
            )
        })?
    } else {
        expand_tilde(flake_field.trim())
    };

    std::fs::canonicalize(&raw).map_err(|err| format!("{raw}: {err}"))
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned()
    })
}

/// Names of the root inputs in the flake's lock file.
fn flake_inputs(path: &Path) -> Vec<String> {
    let Some(lock) = std::fs::read_to_string(path.join("flake.lock"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return Vec::new();
    };

    let root = lock["root"].as_str().unwrap_or("root");
    let mut inputs: Vec<String> = lock["nodes"][root]["inputs"]
        .as_object()
        .map(|inputs| inputs.keys().cloned().collect())
        .unwrap_or_default();
    inputs.sort();
    inputs
}

/// The flake's `nixosConfigurations`, evaluated in the background since nix
/// may have to copy the flake to the store first.
#[derive(Default)]
pub struct Hosts {
    pub names: Vec<String>,
    pub error: Option<String>,
    pending: Option<Receiver<Result<Vec<String>, String>>>,
}

impl Hosts {
    /// Starts evaluating the hosts of `flake`, keeping the old names until done.
    pub fn load(&mut self, flake: Option<&Path>) {
        let Some(path) = flake.map(Path::to_owned) else {
            *self = Self::default();
            return;
        };

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(eval_hosts(&path));
        });
        self.pending = Some(rx);
    }

    /// Picks up the result once the evaluation has finished.
    pub fn poll(&mut self) {
        let Some(rx) = &self.pending else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("evaluation aborted".to_owned()),
        };
        self.pending = None;
        match result {
            Ok(names) => {
                self.names = names;
                self.error = None;
            }
            Err(err) => {
                self.names.clear();
                self.error = Some(err);
            }
        }
    }

    pub fn loading(&self) -> bool {
        self.pending.is_some()
    }
}

fn eval_hosts(path: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("nix")
        .args(["eval", "--json", "--accept-flake-config"])
        .arg(format!("{}#nixosConfigurations", path.display()))
        .args(["--apply", "builtins.attrNames"])
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("could not run nix: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("nix eval failed");
        return Err(reason.trim().to_owned());
    }

    serde_json::from_slice(&output.stdout).map_err(|err| format!("unexpected nix output: {err}"))
}

pub fn load(flake_field: &str) -> Status {
    let flake = resolve_flake(flake_field);
    let mut status = Status {
        flake: flake.clone(),
        default_host: default_host(),
        upstream: None,
        unpushed: None,
        head: None,
        changes: Vec::new(),
        inputs: Vec::new(),
    };

    let Ok(path) = flake else {
        return status;
    };

    status.upstream =
        git(&path, &["rev-parse", "--abbrev-ref", "@{upstream}"]).filter(|s| !s.is_empty());
    if let Some(upstream) = &status.upstream {
        status.unpushed = git(
            &path,
            &["rev-list", "--count", &format!("{upstream}..HEAD")],
        )
        .and_then(|count| count.parse().ok());
    }
    status.head = git(&path, &["log", "-1", "--format=%h %s"]);
    status.changes = git(&path, &["status", "--porcelain", "--untracked-files=all"])
        .map(|out| out.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    status.inputs = flake_inputs(&path);
    status
}
