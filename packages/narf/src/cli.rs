//! Command line arguments.

use clap::Parser;

/// NARF (NixOS Advanced Rebuild Flake) - Rebuilding nix flakes and creating commits
#[derive(Parser, Debug)]
#[command(name = "narf", version)]
pub struct Args {
    /// Build the new generation without switching
    #[arg(short, long)]
    pub build_only: bool,

    /// Add a comment to the resulting commit
    #[arg(short, long)]
    pub comment: Option<String>,

    /// Run in 'dry' mode. Only evaluates and states what WOULD be done. Does not take any effective action
    #[arg(short, long)]
    pub dry: bool,

    /// Path to the flake (must be a git repository). Checks configuration in ".config/narf/flake" by default
    #[arg(short, long)]
    pub flake: Option<String>,

    /// NixOS host configuration to switch to. Uses current hostname by default
    #[arg(short = 'H', long)]
    pub host: Option<String>,

    /// Do not rebuild if no changes were found
    #[arg(short = 'n', long)]
    pub skip_unchanged: bool,

    /// Pull new commits from origin before running
    #[arg(short, long)]
    pub pull: bool,

    /// Push new commits to origin
    #[arg(short = 'P', long)]
    pub push: bool,

    /// Don't squash commits before pushing to origin (only applicable when combined with -P/--push)
    #[arg(short = 'S', long)]
    pub dont_squash_commits: bool,

    /// Show trace of the rebuild (useful if the rebuild keeps failing)
    #[arg(short, long)]
    pub trace: bool,

    /// Update all inputs before running
    #[arg(short, long)]
    pub update: bool,

    /// Update a specific input before running. Can be used multiple times to specify more than one specific input
    #[arg(short = 'U', long)]
    pub update_input: Vec<String>,

    /// Say yes to all (Non-interactive)
    #[arg(short, long)]
    pub yes: bool,
}
