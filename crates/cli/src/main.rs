//! `marshell`: the command line program. `list` and `send` drive the running app, `uninstall` takes Marshell out.

mod args;
mod client;
mod list;
mod send;
mod uninstall;

use args::Command;
use std::io::{stderr, stdin, stdout};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match args::parse(&argv) {
        Ok(Command::Help) => {
            println!("{}", args::USAGE);
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("marshell {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::List { json }) => {
            let env = |name: &str| std::env::var(name).ok();
            exit(list::run(&env, json, &mut stdout(), &mut stderr()))
        }
        Ok(Command::Send { session, text }) => {
            let env = |name: &str| std::env::var(name).ok();
            exit(send::run(&env, &session, &text, &mut stdout(), &mut stderr()))
        }
        Ok(Command::Uninstall { yes }) => {
            let home = match marshell_core::paths::home_dir() {
                Ok(home) => home,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            let Some(user_home) = user_home() else {
                eprintln!("Could not find your home folder.");
                return ExitCode::from(1);
            };
            let env = |name: &str| std::env::var(name).ok();
            let code = uninstall::run(&home, &user_home, &env, &mut stdin().lock(), &mut stdout(), yes);
            exit(code)
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn exit(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// The person's home folder: where `.claude` lives.
fn user_home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}
