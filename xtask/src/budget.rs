//! Informational source sizes and enforced module dependencies.
//!
//! Crates may retain `line-budget` and module `budget` metadata as size
//! targets. Counts and targets are informational; missing targets are valid.
//! Module `uses` lists remain enforced architectural boundaries.
//!
//! Counts include every nonblank, noncomment line under `src/`. A module
//! includes `src/<name>.rs` and `src/<name>/`. Its `uses` list allows sibling
//! modules reached through `crate::`; the crate root is always reachable.

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
    pub(crate) modules: Vec<ModuleSize>,
    /// Sibling dependencies their cards do not allow.
    pub(crate) findings: Vec<String>,
}

impl Line {
    pub(crate) fn dependencies_allowed(&self) -> bool {
        self.findings.is_empty()
    }
}

/// A module's measured size and optional informational target.
pub(crate) struct ModuleSize {
    pub(crate) name: String,
    pub(crate) lines: usize,
    pub(crate) budget: Option<usize>,
}

/// A module declared in the manifest's `modules` table.
struct Module {
    name: String,
    budget: Option<usize>,
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
    let mut sizes = Vec::new();
    for module in &modules {
        let (size, violations) = check_module(&src, module, &modules)?;
        sizes.push(size);
        findings.extend(violations);
    }
    Ok(Line {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        lines,
        budget: declared(&manifest),
        modules: sizes,
        findings,
    })
}

fn check_module(
    src: &Path,
    module: &Module,
    all: &[Module],
) -> Result<(ModuleSize, Vec<String>), String> {
    let mut findings = Vec::new();
    let mut lines = 0;
    let mut reached = Vec::new();
    for file in module_files(src, &module.name)? {
        let text = read(&file)?;
        lines += code_lines(&text);
        reached.extend(crate_paths(&text));
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
    Ok((
        ModuleSize {
            name: module.name.clone(),
            lines,
            budget: module.budget,
        },
        findings,
    ))
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
    let budget =
        after_key(body, "budget").and_then(|value| value.split(',').next()?.trim().parse().ok());
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
    use super::{crate_paths, declared, declared_modules, is_code, measure_crate};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(targets: bool, dependency: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "hgl-size-policy-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("src/source")).unwrap();
            let crate_target = if targets { "line-budget = 0" } else { "" };
            let module_target = if targets { "budget = 0, " } else { "" };
            fs::write(
                path.join("Cargo.toml"),
                format!(
                    "[package.metadata.hgl]\n{crate_target}\n[package.metadata.hgl.modules]\n\
                     source = {{ {module_target}uses = [\"allowed\"] }}\n\
                     allowed = {{ uses = [] }}\nforbidden = {{ uses = [] }}\n"
                ),
            )
            .unwrap();
            fs::write(
                path.join("src/source.rs"),
                format!("use crate::{dependency}::Item;\n"),
            )
            .unwrap();
            fs::write(
                path.join("src/source/nested.rs"),
                "const VALUE: usize = 1;\n",
            )
            .unwrap();
            fs::write(path.join("src/allowed.rs"), "pub struct Item;\n").unwrap();
            fs::write(path.join("src/forbidden.rs"), "pub struct Item;\n").unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn oversized_and_missing_targets_are_informational() {
        for targets in [true, false] {
            let fixture = Fixture::new(targets, "allowed");
            let report = measure_crate(&fixture.0).unwrap();
            assert!(report.dependencies_allowed());
            assert_eq!(report.lines, 4);
            assert_eq!(report.budget, targets.then_some(0));
            assert_eq!(report.modules.len(), 3);
            let source = &report.modules[0];
            assert_eq!(source.name, "source");
            assert_eq!(source.lines, 2);
            assert_eq!(source.budget, targets.then_some(0));
        }
    }

    #[test]
    fn forbidden_dependency_still_fails_with_or_without_targets() {
        for targets in [true, false] {
            let fixture = Fixture::new(targets, "forbidden");
            let report = measure_crate(&fixture.0).unwrap();
            assert!(!report.dependencies_allowed());
            assert_eq!(
                report.findings,
                ["module source uses crate::forbidden, which its card does not allow"]
            );
        }
    }

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
        assert_eq!(modules[0].budget, Some(1100));
        assert_eq!(modules[0].uses, ["endpoints", "keys"]);
        assert_eq!(modules[1].budget, Some(40));
        assert!(modules[1].uses.is_empty());
    }

    #[test]
    fn finds_the_module_each_crate_path_reaches() {
        let text = "use crate::keys::Key;\nlet x = crate::Store::new(); crate::bindings::bind()";
        assert_eq!(crate_paths(text), ["keys", "Store", "bindings"]);
    }
}
