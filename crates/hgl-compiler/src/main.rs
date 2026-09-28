//! Command-line checking and Rust emission; no graph execution during compilation.
use hgl_compiler::{Source, check, emit_documentation, emit_rust};
use std::path::PathBuf;

#[expect(clippy::print_stderr, reason = "the compiler CLI renders diagnostics")]
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: hglc check FILE [--part FILE] | emit-rust FILE [--part FILE] --out FILE | doc FILE [--part FILE] --out FILE";
    let command = args.next().ok_or(usage)?;
    if !matches!(command.as_str(), "check" | "emit-rust" | "doc") {
        return Err(usage.into());
    }
    let mut files = vec![PathBuf::from(args.next().ok_or(usage)?)];
    let mut output = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--part" => files.push(PathBuf::from(args.next().ok_or(usage)?)),
            "--out" if output.is_none() => output = Some(PathBuf::from(args.next().ok_or(usage)?)),
            _ => return Err(usage.into()),
        }
    }
    if (command != "check") != output.is_some() {
        return Err(usage.into());
    }
    let sources = files
        .iter()
        .map(|path| {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Source {
                name: path.display().to_string(),
                text,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let checked = check(&sources).map_err(|errors| {
        errors
            .iter()
            .map(|issue| {
                let text = sources
                    .iter()
                    .find(|s| s.name == issue.source)
                    .map_or("", |s| s.text.as_str());
                let prefix = &text[..issue.start.min(text.len())];
                let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
                let column = prefix
                    .rsplit('\n')
                    .next()
                    .map_or(1, |s| s.chars().count() + 1);
                format!("{}:{line}:{column}: {}", issue.source, issue.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    if let Some(path) = output {
        std::fs::write(
            &path,
            if command == "doc" {
                emit_documentation(&checked)
            } else {
                emit_rust(&checked).map_err(|errors| {
                    errors
                        .iter()
                        .map(|error| format!("{}: {}", error.source, error.message))
                        .collect::<Vec<_>>()
                        .join("\n")
                })?
            },
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}
