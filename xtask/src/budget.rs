//! The line-budget gate.
//!
//! Every crate declares how large it may grow, in its own `Cargo.toml`:
//!
//! ```toml
//! [package.metadata.hgl]
//! line-budget = 500
//! ```
//!
//! What is counted is every line under `src/` that is neither blank nor a
//! comment, doc comments included in "comment". A crate over its budget, or
//! without one, fails the gate. A budget is raised by a decision, never to make
//! a build pass (`CLAUDE.md`).

use std::fs;
use std::path::{Path, PathBuf};

const KEY: &str = "line-budget";

/// One crate's size against what it declared.
pub(crate) struct Line {
    pub(crate) name: String,
    pub(crate) lines: usize,
    /// `None` when the crate declares no budget.
    pub(crate) budget: Option<usize>,
}

impl Line {
    pub(crate) fn within(&self) -> bool {
        self.budget.is_some_and(|budget| self.lines <= budget)
    }
}

/// Measure every crate of the workspace rooted at `root`.
pub(crate) fn measure(root: &Path) -> Result<Vec<Line>, String> {
    let mut crates = vec![root.join("xtask"), root.join("bench").join("twin")];
    crates.extend(directories(&root.join("crates"))?);
    crates.sort();
    crates.iter().map(|path| measure_crate(path)).collect()
}

fn measure_crate(path: &Path) -> Result<Line, String> {
    let manifest = read(&path.join("Cargo.toml"))?;
    let mut lines = 0;
    for file in rust_files(&path.join("src"))? {
        lines += read(&file)?.lines().filter(|line| is_code(line)).count();
    }
    Ok(Line {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        lines,
        budget: declared(&manifest),
    })
}

fn is_code(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with("//")
}

fn declared(manifest: &str) -> Option<usize> {
    manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix(KEY))
        .find_map(|rest| rest.trim().strip_prefix('=')?.trim().parse().ok())
}

fn directories(path: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(entries(path)?
        .into_iter()
        .filter(|entry| entry.is_dir())
        .collect())
}

fn rust_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in entries(path)? {
        if entry.is_dir() {
            files.extend(rust_files(&entry)?);
        } else if entry.extension().is_some_and(|extension| extension == "rs") {
            files.push(entry);
        }
    }
    Ok(files)
}

fn entries(path: &Path) -> Result<Vec<PathBuf>, String> {
    let listing = fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    listing
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{declared, is_code};

    #[test]
    fn blank_lines_and_comments_are_not_code() {
        assert!(is_code(
            "    let x = 1; // trailing comments still count as code"
        ));
        assert!(!is_code("   "));
        assert!(!is_code("    // a comment"));
        assert!(!is_code("/// a doc comment"));
        assert!(!is_code("//! a crate doc comment"));
    }

    #[test]
    fn reads_the_declared_budget() {
        assert_eq!(
            declared("[package.metadata.hgl]\nline-budget = 500\n"),
            Some(500)
        );
        assert_eq!(declared("[package]\nname = \"x\"\n"), None);
        assert_eq!(declared("line-budget = lots\n"), None);
    }
}
