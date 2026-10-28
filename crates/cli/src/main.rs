//! `marshell`: the command line program. `list`, `send`, `new` and `focus` drive the running app (and `new` and
//! `focus` can start it), `uninstall` takes Marshell out.

mod args;
mod client;
mod focus;
mod list;
mod new;
mod send;
mod start;
mod uninstall;

use args::Command;
use std::io::{IsTerminal, stderr, stdin, stdout};
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
        Ok(Command::New {
            agent,
            cwd,
            prompt,
            mode,
            name,
            start,
        }) => {
            let current = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("Could not read the current folder: {e}");
                    return ExitCode::from(1);
                }
            };
            let options = new::Options {
                agent,
                cwd,
                prompt,
                mode,
                name,
                start,
            };
            let env = |name: &str| std::env::var(name).ok();
            let tty = stdin().is_terminal();
            let code = new::run(
                &env,
                options,
                &current,
                tty,
                &mut stdin().lock(),
                &mut stdout(),
                &mut stderr(),
            );
            exit(code)
        }
        Ok(Command::Focus { session, start }) => {
            let env = |name: &str| std::env::var(name).ok();
            let tty = stdin().is_terminal();
            let code = focus::run(
                &env,
                &session,
                start,
                tty,
                &mut stdin().lock(),
                &mut stdout(),
                &mut stderr(),
            );
            exit(code)
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
