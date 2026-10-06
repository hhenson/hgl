//! Public source-test execution through the same emitted Rust used by embedding callers.
use std::fmt::Write as _;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
/// Check, build and execute selected named source tests. Build failures never pass.
pub fn run(arguments: &[String], root: &Path) -> Result<(), String> {
    if arguments.first().is_some_and(|arg| arg.starts_with('-')) {
        return Err("test requires FILE; unknown option".into());
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
    let sources = hgl_library_files::sources(&files, &libraries)
        .map_err(|e| format!("infrastructure: {e}"))?;
    let mut plan =
        hgl_reject::Plan::prepare(sources, files.len()).map_err(|e| format!("admission: {e}"))?;
    hgl_diagnostics::ensure(hgl_program::module_diagnostics(&plan.sources))
        .map_err(|e| format!("admission: {e}"))?;
    let mut suite =
        hgl_program::compile_module_suite(&plan.sources).map_err(|e| format!("admission: {e}"))?;
    plan.select(
        &names,
        &suite
            .tests
            .iter()
            .map(|test| test.name.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| format!("admission: {e}"))?;
    suite
        .tests
        .retain(|test| hgl_reject::selected(&test.name, &names));
    let outcomes = plan.check();
    if suite.tests.is_empty() && outcomes.is_empty() {
        return Err("no tests found".into());
    }
    let rejected = report(&outcomes);
    if suite.tests.is_empty() {
        return rejected;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("infrastructure: {error}"))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("hgl-test-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).map_err(|error| format!("infrastructure: {error}"))?;
    let outcome = build_run(&dir, root, &hgl_program::emit_tests(&suite));
    let cleanup = std::fs::remove_dir_all(&dir).map_err(|error| format!("infrastructure: {error}"));
    outcome.and(cleanup).and(rejected)
}
fn build_run(dir: &Path, root: &Path, source: &str) -> Result<(), String> {
    let package = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("infrastructure: invalid temporary package name")?;
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
        writeln!(manifest, "{name}={{path=\"{path}\"}}")
            .map_err(|error| format!("infrastructure: {error}"))?;
    }
    std::fs::write(dir.join("Cargo.toml"), manifest)
        .map_err(|error| format!("infrastructure: {error}"))?;
    std::fs::write(dir.join("src/main.rs"), source)
        .map_err(|error| format!("infrastructure: {error}"))?;
    let status = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--target-dir"])
        .arg(root.join("target/source-tests"))
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .current_dir(root)
        .status()
        .map_err(|error| format!("infrastructure: test build: {error}"))?;
    if !status.success() {
        return Err(format!("infrastructure: test build failed: {status}"));
    }
    let binary = if cfg!(windows) {
        format!("{package}.exe")
    } else {
        package.into()
    };
    let status = Command::new(root.join("target/source-tests/debug").join(binary))
        .current_dir(root)
        .status()
        .map_err(|error| format!("infrastructure: test execution: {error}"))?;
    if status.success() {
        Ok(())
    } else if status.code() == Some(1) {
        Err("executed tests failed".into())
    } else {
        Err(format!("infrastructure: test process failed: {status}"))
    }
}
#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the source-test CLI reports independently checked rejection outcomes"
)]
fn report(outcomes: &[hgl_reject::Outcome]) -> Result<(), String> {
    let mut failed = 0;
    for outcome in outcomes {
        match &outcome.result {
            Ok(()) => println!("{} ... ok [rejection]", outcome.name),
            Err(error) => {
                failed += 1;
                println!("{} ... FAILED [rejection]", outcome.name);
                eprintln!("{}: {error}", outcome.name);
            }
        }
    }
    if !outcomes.is_empty() {
        println!("{} rejection cases, {failed} failures", outcomes.len());
    }
    if failed == 0 {
        Ok(())
    } else {
        Err(format!("{failed} rejection cases failed"))
    }
}
