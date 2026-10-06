//! The line-budget gate.
//!
//! Every crate declares how large it may grow, in its own `Cargo.toml`, and
//! may declare the same for the modules that were crates of their own
//! (docs/decisions/0004):
//!
//! ```toml
//! [package.metadata.hgl]
//! line-budget = 500
//!
//! [package.metadata.hgl.modules]
//! bindings = { budget = 1100, uses = ["endpoints"] }
//! ```
//!
//! What is counted is every line under `src/` that is neither blank nor a
//! comment, doc comments included in "comment". A module is `src/<name>.rs`
//! plus `src/<name>/`; `uses` names the sibling modules its card lets it
//! reach through `crate::`, the crate root always being reachable. A crate
//! or module over its budget, a crate without one, or a module reaching a
//! sibling it did not declare fails the gate. A budget is raised by a
//! decision, never to make a build pass (`CLAUDE.md`).

use std::fs;
use std::path::{Path, PathBuf};

const KEY: &str = "line-budget";
const MODULES: &str = "[package.metadata.hgl.modules]";

/// One crate's size against what it declared.
pub(crate) struct Line {
    pub(crate) name: String,
    pub(crate) lines: usize,
    /// `None` when the crate declares no budget.
    pub(crate) budget: Option<usize>,
    /// What its modules got wrong: over budget, or a use their card lacks.
    pub(crate) findings: Vec<String>,
}

impl Line {
    pub(crate) fn within(&self) -> bool {
        self.budget.is_some_and(|budget| self.lines <= budget) && self.findings.is_empty()
    }
}

/// A module declared in the manifest's `modules` table.
struct Module {
    name: String,
    budget: usize,
    uses: Vec<String>,
}

/// Measure every crate of the workspace rooted at `root`.
pub(crate) fn measure(root: &Path) -> Result<Vec<Line>, String> {
    let mut crates = vec![root.join("xtask")];
    crates.extend(directories(&root.join("crates"))?);
    crates.sort();
    crates.iter().map(|path| measure_crate(path)).collect()
}

fn measure_crate(path: &Path) -> Result<Line, String> {
    let manifest = read(&path.join("Cargo.toml"))?;
    let src = path.join("src");
    let mut lines = 0;
    for file in rust_files(&src)? {
        lines += code_lines(&read(&file)?);
    }
    let modules = declared_modules(&manifest);
    let mut findings = Vec::new();
    for module in &modules {
        findings.extend(check_module(&src, module, &modules)?);
    }
    Ok(Line {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        lines,
        budget: declared(&manifest),
        findings,
    })
}

fn check_module(src: &Path, module: &Module, all: &[Module]) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    let mut lines = 0;
    let mut reached = Vec::new();
    for file in module_files(src, &module.name)? {
        let text = read(&file)?;
        lines += code_lines(&text);
        reached.extend(crate_paths(&text));
    }
    if lines > module.budget {
        findings.push(format!(
            "module {} is {lines} lines against a budget of {}",
            module.name, module.budget
        ));
    }
    reached.sort();
    reached.dedup();
    for sibling in reached {
        let is_module = all.iter().any(|candidate| candidate.name == sibling);
        if is_module && sibling != module.name && !module.uses.contains(&sibling) {
            findings.push(format!(
                "module {} uses crate::{sibling}, which its card does not allow",
                module.name
            ));
        }
    }
    Ok(findings)
}

fn module_files(src: &Path, name: &str) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let file = src.join(format!("{name}.rs"));
    if file.is_file() {
        files.push(file);
    }
    let dir = src.join(name);
    if dir.is_dir() {
        files.extend(rust_files(&dir)?);
    }
    Ok(files)
}

/// The first path segment after each `crate::` in `text`.
fn crate_paths(text: &str) -> Vec<String> {
    text.match_indices("crate::")
        .map(|(at, found)| {
            text[at + found.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .collect()
}

fn code_lines(text: &str) -> usize {
    text.lines().filter(|line| is_code(line)).count()
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

fn declared_modules(manifest: &str) -> Vec<Module> {
    let mut modules = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == MODULES;
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            modules.extend(parse_module(line));
        }
    }
    modules
}

/// `name = { budget = 100, uses = ["a", "b"] }`
fn parse_module(line: &str) -> Option<Module> {
    let (name, rest) = line.split_once('=')?;
    let body = rest.trim().strip_prefix('{')?.strip_suffix('}')?;
    let budget = after_key(body, "budget")?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()?;
    let uses = after_key(body, "uses")
        .and_then(|list| list.strip_prefix('[')?.split(']').next())
        .map_or_else(Vec::new, |list| {
            list.split(',')
                .map(|item| item.trim().trim_matches('"').to_owned())
                .filter(|item| !item.is_empty())
                .collect()
        });
    Some(Module {
        name: name.trim().to_owned(),
        budget,
        uses,
    })
}

/// The text after `key =` in an inline table, trimmed.
fn after_key<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let (_, after) = body.split_once(key)?;
    Some(after.trim().strip_prefix('=')?.trim())
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
    use super::{crate_paths, declared, declared_modules, is_code};

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

    #[test]
    fn reads_the_modules_table_only() {
        let manifest = "[package.metadata.hgl]\nline-budget = 500\n\n\
            [package.metadata.hgl.modules]\n\
            # a comment\n\
            bindings = { budget = 1100, uses = [\"endpoints\", \"keys\"] }\n\
            keys = { budget = 40, uses = [] }\n\n\
            [dependencies]\nother = { budget = 1 }\n";
        let modules = declared_modules(manifest);
        let names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["bindings", "keys"]);
        assert_eq!(modules[0].budget, 1100);
        assert_eq!(modules[0].uses, ["endpoints", "keys"]);
        assert_eq!(modules[1].budget, 40);
        assert!(modules[1].uses.is_empty());
    }

    #[test]
    fn finds_the_module_each_crate_path_reaches() {
        let text = "use crate::keys::Key;\nlet x = crate::Store::new(); crate::bindings::bind()";
        assert_eq!(crate_paths(text), ["keys", "Store", "bindings"]);
    }
}
