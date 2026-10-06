//! Repository automation, run as `cargo xtask <task>`.
//!
//! `cargo xtask ci` runs every gate that CI runs, in the same order, and ends
//! with one line per gate. Work is not done until it passes. `cargo xtask ci
//! test "test --release"` runs only the named gates, which is how CI spreads
//! the gates over parallel jobs.
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
    /// Run under a cap on the address space. Set for the gates that run this
    /// project's own tests, which is where an allocation can be sized from a
    /// computed value; a tool's own appetite is not this cap's business.
    capped: bool,
}

/// The cap a test gate runs under, in kilobytes as `ulimit` takes it.
const ADDRESS_SPACE_KB: &str = "8000000";

const GATES: &[Gate] = &[
    Gate {
        name: "format",
        args: &["fmt", "--all", "--", "--check"],
        env: &[],
        optional: false,
        capped: false,
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
        capped: false,
    },
    Gate {
        name: "test",
        args: &["test", "--workspace", "--quiet"],
        env: &[],
        optional: false,
        capped: true,
    },
    // Debug builds check the assertions; release builds run what ships, where
    // a wrong schedule can hang or allocate instead of asserting.
    Gate {
        name: "test --release",
        args: &["test", "--workspace", "--release", "--quiet"],
        env: &[],
        optional: false,
        capped: true,
    },
    Gate {
        name: "docs",
        args: &["doc", "--workspace", "--no-deps", "--quiet"],
        env: &[("RUSTDOCFLAGS", "-D warnings")],
        optional: false,
        capped: false,
    },
    Gate {
        name: "deny",
        args: &["deny", "check"],
        env: &[],
        optional: true,
        capped: false,
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
        ci(rest)
    } else if task == "bench" {
        report(bench::run(rest))
    } else {
        println!("usage: cargo xtask ci [GATE...] | bench");
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

/// Run every gate, or only the named ones: CI runs the two test profiles as
/// separate jobs so they overlap instead of following each other.
fn ci(selected: &[String]) -> ExitCode {
    let known = |name: &str| name == "budget" || GATES.iter().any(|gate| gate.name == name);
    if let Some(unknown) = selected.iter().find(|name| !known(name)) {
        let names: Vec<&str> = GATES.iter().map(|gate| gate.name).collect();
        println!(
            "unknown gate {unknown:?}; the gates are budget, {}",
            names.join(", ")
        );
        return ExitCode::FAILURE;
    }
    let wanted = |name: &str| selected.is_empty() || selected.iter().any(|chosen| chosen == name);
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let gates: Vec<&Gate> = GATES.iter().filter(|gate| wanted(gate.name)).collect();
    let outcomes: Vec<Outcome> = gates.iter().map(|gate| run(&cargo, gate)).collect();

    let budget = wanted("budget").then(|| {
        println!("== budget");
        budgets()
    });

    println!();
    for (gate, outcome) in gates.iter().zip(&outcomes) {
        println!("{:<16}{}", gate.name, outcome.label());
    }
    if let Some(budget) = budget {
        println!("{:<16}{}", "budget", budget.label());
    }
    if outcomes.contains(&Outcome::Failed) || budget == Some(Outcome::Failed) {
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
                for finding in &line.findings {
                    println!("{:<18}{finding}", "");
                }
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
    let status = command(cargo, gate).envs(gate.env.iter().copied()).status();
    if status.is_ok_and(|status| status.success()) {
        Outcome::Passed
    } else {
        Outcome::Failed
    }
}

/// The gate's command, under a cap on its address space where it asks for one:
/// a test that sizes an allocation from a computed value then fails the gate
/// instead of taking the machine down with it. Building and running the tests
/// fits well inside the cap. macOS cannot set the limit, so there every gate
/// runs uncapped.
fn command(cargo: &OsStr, gate: &Gate) -> Command {
    if !gate.capped || !cfg!(target_os = "linux") {
        let mut direct = Command::new(cargo);
        direct.args(gate.args);
        return direct;
    }
    // `bash -c SCRIPT NAME ARGS...` leaves the arguments in `"$@"`.
    let mut shell = Command::new("bash");
    shell
        .arg("-c")
        .arg(format!("ulimit -v {ADDRESS_SPACE_KB}; exec \"$@\""))
        .arg("xtask")
        .arg(cargo)
        .args(gate.args);
    shell
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
