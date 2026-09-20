//! Repository automation, run as `cargo xtask <task>`.
//!
//! `cargo xtask ci` runs every gate that CI runs, in the same order, and ends
//! with one line per gate. Work is not done until it passes.
//!
//! `cargo xtask bench` measures one benchmark program; see [`mod@bench`].
#![expect(clippy::print_stdout, reason = "xtask is a command-line tool")]

mod bench;
mod budget;

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;
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
    // Debug builds check the assertions; release builds run what ships, where
    // a wrong schedule can hang or allocate instead of asserting.
    Gate {
        name: "test --release",
        args: &["test", "--workspace", "--release", "--quiet"],
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
    let args: Vec<String> = env::args().skip(1).collect();
    let (task, rest) = args
        .split_first()
        .map_or(("", &[][..]), |(task, rest)| (task.as_str(), rest));
    if task == "ci" {
        ci()
    } else if task == "bench" {
        report(bench::run(rest))
    } else {
        println!("usage: cargo xtask ci | bench");
        ExitCode::FAILURE
    }
}

fn report(outcome: Result<(), String>) -> ExitCode {
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            println!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> ExitCode {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let outcomes: Vec<Outcome> = GATES.iter().map(|gate| run(&cargo, gate)).collect();

    println!("== budget");
    let budget = budgets();

    println!();
    for (gate, outcome) in GATES.iter().zip(&outcomes) {
        println!("{:<16}{}", gate.name, outcome.label());
    }
    println!("{:<16}{}", "budget", budget.label());
    if outcomes.contains(&Outcome::Failed) || budget == Outcome::Failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Print every crate's size against its declared line budget.
fn budgets() -> Outcome {
    let root = env::var_os("CARGO_MANIFEST_DIR").map_or_else(
        || Path::new(".").to_path_buf(),
        |xtask| Path::new(&xtask).join(".."),
    );
    match budget::measure(&root) {
        Ok(lines) => {
            for line in &lines {
                let budget = line.budget.map_or_else(
                    || "no line-budget declared".to_owned(),
                    |budget| budget.to_string(),
                );
                let verdict = if line.within() { "" } else { "  <-- OVER" };
                println!("{:<18}{:>5} / {budget}{verdict}", line.name, line.lines);
            }
            if lines.iter().all(budget::Line::within) {
                Outcome::Passed
            } else {
                Outcome::Failed
            }
        }
        Err(message) => {
            println!("{message}");
            Outcome::Failed
        }
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
