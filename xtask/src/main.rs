//! Repository automation, run as `cargo xtask <task>`.
//!
//! `cargo xtask ci` runs every gate that CI runs, in the same order, and ends
//! with one line per gate. Work is not done until it passes.
#![expect(clippy::print_stdout, reason = "xtask is a command-line tool")]

use std::env;
use std::ffi::{OsStr, OsString};
use std::process::{Command, ExitCode, Stdio};

/// One check: the cargo arguments that run it and the environment it needs.
struct Gate {
    name: &'static str,
    args: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
    /// Reported as skipped, not failed, when the cargo subcommand is missing.
    optional: bool,
}

const GATES: &[Gate] = &[
    Gate {
        name: "format",
        args: &["fmt", "--all", "--", "--check"],
        env: &[],
        optional: false,
    },
    Gate {
        name: "clippy",
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        env: &[],
        optional: false,
    },
    Gate {
        name: "test",
        args: &["test", "--workspace", "--quiet"],
        env: &[],
        optional: false,
    },
    Gate {
        name: "docs",
        args: &["doc", "--workspace", "--no-deps", "--quiet"],
        env: &[("RUSTDOCFLAGS", "-D warnings")],
        optional: false,
    },
    Gate {
        name: "deny",
        args: &["deny", "check"],
        env: &[],
        optional: true,
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Passed,
    Failed,
    Skipped,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Passed => "ok",
            Self::Failed => "FAILED",
            Self::Skipped => "skipped (not installed)",
        }
    }
}

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() == Some("ci") {
        ci()
    } else {
        println!("usage: cargo xtask ci");
        ExitCode::FAILURE
    }
}

fn ci() -> ExitCode {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let outcomes: Vec<Outcome> = GATES.iter().map(|gate| run(&cargo, gate)).collect();

    println!();
    for (gate, outcome) in GATES.iter().zip(&outcomes) {
        println!("{:<8}{}", gate.name, outcome.label());
    }
    if outcomes.contains(&Outcome::Failed) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run(cargo: &OsStr, gate: &Gate) -> Outcome {
    if gate.optional && !installed(cargo, gate) {
        return Outcome::Skipped;
    }
    println!("== {}", gate.name);
    let status = Command::new(cargo)
        .args(gate.args)
        .envs(gate.env.iter().copied())
        .status();
    if status.is_ok_and(|status| status.success()) {
        Outcome::Passed
    } else {
        Outcome::Failed
    }
}

/// Whether the cargo subcommand behind `gate` exists on this machine.
fn installed(cargo: &OsStr, gate: &Gate) -> bool {
    let Some(subcommand) = gate.args.first() else {
        return false;
    };
    Command::new(cargo)
        .args([subcommand, "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
