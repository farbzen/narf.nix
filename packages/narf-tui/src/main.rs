//! narf-tui - Terminal frontend for narf.
//!
//! Lets you pick narf's options in a form, shows the state of the flake
//! repository and runs narf inside an embedded pseudo terminal, so its
//! interactive prompts keep working.

mod app;
mod runner;
mod status;
mod ui;

use std::ffi::OsString;

use anyhow::{Result, bail};

/// Hidden flag making the binary run narf itself; the TUI starts itself
/// with it inside the pseudo terminal.
const EXEC_FLAG: &str = "--exec";

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1).peekable();

    if args.next_if(|arg| arg == EXEC_FLAG).is_some() {
        std::process::exit(narf::run(
            std::iter::once(OsString::from("narf")).chain(args),
        ));
    }

    if let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "-h" | "--help" => {
                println!(
                    "narf-tui - TUI frontend for narf\n\n\
                     Usage: narf-tui [OPTIONS]\n\n\
                     Options:\n  \
                     -h, --help  Print help"
                );
                return Ok(());
            }
            other => bail!("unexpected argument '{other}'. For more information, try '--help'."),
        }
    }

    let mut app = app::App::new();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
