//! Compile and execute the selected standard-library HGL, not copied Rust nodes.
use std::fmt::Write as _;
#[cfg(test)]
#[path = "support/helpers.rs"]
mod support;
use hgl_program::{compile, emit_rust};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
use std::{fs, process::Command, time::SystemTime};

#[test]
fn library_graph_runs_sources_sinks_and_fresh_instances() -> Result<(), Box<dyn std::error::Error>>
{
    let root = support::root();
    let dir = scratch_dir("hgl-stdlib")?;
    fs::create_dir_all(dir.join("src"))?;
    let mut runner = include_str!("support/runtime.rs").to_owned();
    let cases = [
        ("normal", "debug_print(\"answer\", const(42))", 42, 1),
        (
            "delayed",
            "debug_print(\"later\", const(-7, delay: 2us))",
            -7,
            3,
        ),
        (
            "sampled",
            "debug_print(\"sampled\", const(42), sample: 2)",
            42,
            1,
        ),
        ("renamed", "debug_print(\"renamed\", fixed(9))", 9, 1),
        ("changed", "debug_print(\"changed\", const(42))", 99, 1),
    ];
    let mut calls = String::new();
    for (name, body, value, time) in cases {
        let mut input = support::sources(&support::main_source(body));
        if name == "renamed" {
            for (_, source) in &mut input {
                *source = source
                    .replace("const<", "fixed<")
                    .replace("{const, debug_print}", "{fixed, debug_print}");
            }
        }
        if name == "changed" {
            for (_, source) in &mut input {
                *source = source.replace(
                    "start { schedule(alarm, delay) }\n    when { return value }",
                    "start { schedule(alarm, delay) }\n    when { return 99 }",
                );
            }
        }
        let program = compile(&input, "main")?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&program))?;
        write!(
            runner,
            "mod {name};\nimpl {name}::Native for Provider {{ fn as_str_i64(value:i64)->String {{value.to_string()}} fn print_line_str(text:&str) {{println!(\"{{text}}\");}} }}\n"
        )?;
        write!(
            calls,
            "let mut registry=Registry::new();\n{name}::register(&mut registry).unwrap();\nrun(&registry,{name}::main(&registry).unwrap(),{value},{time});\n"
        )?;
        if name == "normal" {
            calls.push_str("registry.register::<hgl_testkit::Replay>().unwrap();\nrun(&registry,normal::main(&registry).unwrap(),42,1);\nreplay(&mut registry);\n");
        }
        if name == "sampled" {
            calls.push_str("registry.register::<hgl_testkit::Replay>().unwrap();\nreplay(&mut registry);\nreplay(&mut registry);\n");
        }
    }
    write!(runner, "struct Provider;\nfn main() {{\n{calls}\n}}\n")?;
    fs::write(dir.join("src/main.rs"), runner)?;
    manifest(&root, &dir)?;
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
        .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(&dir)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)?.replace("\r\n", "\n"),
        "answer: 42\nanswer: 42\nanswer: 42\nanswer: 42\nanswer: -7\nlater: -7\n[2] sampled: 42\n[2] sampled: 42\nrenamed: 9\nchanged: 99\n"
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn manifest(
    root: &std::path::Path,
    dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"stdlib-graph-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
    ] {
        let path = root.join("crates").join(name);
        let escaped = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{escaped}\"}}")?;
    }
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, dir))?;
    Ok(())
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &std::path::Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}

/// A directory of this test's own under the temp root; see `NEXT_DIR`.
fn scratch_dir(prefix: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let tick = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos();
    let serial = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(std::env::temp_dir().join(format!("{prefix}-{}-{tick}-{serial}", std::process::id())))
}
