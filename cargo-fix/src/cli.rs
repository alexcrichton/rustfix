use std::env;
use std::process::Command;

use failure::{Error, ResultExt};
use clap::{Arg, App, SubCommand, AppSettings};

use lock::Server;
use super::exit_with;

pub fn run() -> Result<(), Error> {
    let matches = App::new("Cargo Fix")
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("fix")
                .version(env!("CARGO_PKG_VERSION"))
                .author("The Rust Project Developers")
                .about("Automatically apply rustc's suggestions about fixing code")
                .setting(AppSettings::TrailingVarArg)
                .arg(Arg::with_name("args").multiple(true))
                .arg(
                    Arg::with_name("broken-code")
                        .long("broken-code")
                        .help("Fix code even if it already has compiler errors")
                )
        )
        .get_matches();
    let matches = match matches.subcommand() {
        ("fix", Some(matches)) => matches,
        _ => bail!("unknown cli arguments passed"),
    };

    if matches.is_present("broken-code") {
        env::set_var("__CARGO_FIX_BROKEN_CODE", "1");
    }

    // Spin up our lock server which our subprocesses will use to synchronize
    // fixes.
    let _lockserver = Server::new()?.start()?;

    let cargo = env::var_os("CARGO").unwrap_or("cargo".into());
    let mut cmd = Command::new(&cargo);
    // TODO: shouldn't hardcode `check` here, we want to allow things like
    // `cargo fix bench` or something like that
    //
    // TODO: somehow we need to force `check` to actually do something here, if
    // `cargo check` was previously run it won't actually do anything again.
    cmd.arg("check");
    if let Some(args) = matches.values_of("args") {
        cmd.args(args);
    }

    // Override the rustc compiler as ourselves. That way whenever rustc would
    // run we run instead and have an opportunity to inject fixes.
    let me = env::current_exe()
        .context("failed to learn about path to current exe")?;
    cmd.env("RUSTC", &me)
        .env("__CARGO_FIX_NOW_RUSTC", "1");
    if let Some(rustc) = env::var_os("RUSTC") {
        cmd.env("RUSTC_ORIGINAL", rustc);
    }

    // An now execute all of Cargo! This'll fix everything along the way.
    //
    // TODO: we probably want to do something fancy here like collect results
    // from the client processes and print out a summary of what happened.
    let status = cmd.status()
        .with_context(|e| {
            format!("failed to execute `{}`: {}", cargo.to_string_lossy(), e)
        })?;
    exit_with(status);
}