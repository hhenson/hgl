//! Public source-test execution through the same emitted Rust used by embedding callers.
use std::fmt::Write as _;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
/// Check, build and execute selected named source tests. Build failures never pass.
pub fn run(arguments: &[String], root: &Path) -> Result<(), String> {
    if arguments.first().is_some_and(|arg| arg == "--reject") {
        if arguments.len() != 2 {
            return Err("usage: hglc test --reject FILE".into());
        }
        return hgl_reject::reject(Path::new(&arguments[1]));
    }
    let mut args = arguments.iter();
    let mut files = vec![PathBuf::from(args.next().ok_or("test requires FILE")?)];
    let mut libraries = Vec::new();
    let mut names = Vec::new();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--part" => files.push(PathBuf::from(args.next().ok_or("--part requires FILE")?)),
            "--library" => {
                libraries.push(PathBuf::from(args.next().ok_or("--library requires DIR")?));
            }
            name if !name.starts_with('-') => names.push(name),
            _ => return Err("unknown test option".into()),
        }
    }
    let mut suite = hgl_program::compile_tests_files(&files, &libraries)?;
    suite.select(&names)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("hgl-test-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).map_err(|error| error.to_string())?;
    let outcome = build_run(&dir, root, &hgl_program::emit_tests(&suite), &names);
    let cleanup = std::fs::remove_dir_all(&dir).map_err(|error| error.to_string());
    outcome.and(cleanup)
}
fn build_run(dir: &Path, root: &Path, source: &str, names: &[&str]) -> Result<(), String> {
    let package = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid temporary package name")?;
    let mut manifest = format!(
        "[package]\nname={package:?}\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-harness",
        "hgl-harness-ir",
        "hgl-rust-ir",
        "hgl-source",
        "hgl-value-eval",
        "hgl-time-context",
        "hgl-testkit",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}").map_err(|error| error.to_string())?;
    }
    std::fs::write(dir.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    std::fs::write(dir.join("src/main.rs"), source).map_err(|error| error.to_string())?;
    let status = Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--target-dir"])
        .arg(root.join("target/source-tests"))
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .arg("--")
        .args(names)
        .current_dir(root)
        .status()
        .map_err(|error| format!("test build/run: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("test build/run failed: {status}"))
    }
}
