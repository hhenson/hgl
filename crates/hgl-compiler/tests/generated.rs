//! Compile and execute emitted Rust independently of compiler internals.
use hgl_compiler::{Source, check, emit_rust};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn generated_nodes_and_graphs_execute_the_reference_cases() -> Result<(), Box<dyn std::error::Error>>
{
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-bootstrap-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    prepare(&root, &directory)?;
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .current_dir(&directory)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)?.replace("\r\n", "\n"),
        "42\n42\n-7\n42\n9\n42\n42\n-7\n"
    );
    fs::write(
        directory.join("src/main.rs"),
        include_str!("support/runtime.rs").replacen(
            "fn print_i64(value: i64)",
            "fn print_i64(value: bool)",
            1,
        ),
    )?;
    let wrong = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet"])
        .current_dir(&directory)
        .output()?;
    assert!(!wrong.status.success());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("incompatible type for trait"));
    fs::remove_dir_all(directory)?;
    Ok(())
}

#[expect(
    clippy::unnecessary_debug_formatting,
    reason = "TOML paths need quotes and escaped backslashes"
)]
fn prepare(
    root: &std::path::Path,
    directory: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let main = fs::read_to_string(root.join("examples/const-debug/main.hgl"))?;
    let part = fs::read_to_string(root.join("examples/const-debug/rust.hgl"))?;
    let cases = [
        ("generated", main.clone()),
        (
            "renamed",
            main.replace("const_(", "source(")
                .replace("debug_print", "sink")
                .replace("42", "-7"),
        ),
        ("dormant", main.replace("when scheduled()", "when")),
        (
            "nested",
            main.replace(
                "debug_print(value)",
                "debug_print(const_(9))\n    debug_print(value)",
            ),
        ),
    ];
    for (name, text) in cases {
        let sources = [
            Source {
                name: "main.hgl".into(),
                text,
            },
            Source {
                name: "rust.hgl".into(),
                text: part.clone(),
            },
        ];
        let checked = check(&sources).map_err(|e| format!("{e:?}"))?;
        fs::write(
            directory.join(format!("src/{name}.rs")),
            emit_rust(&checked).map_err(|e| format!("{e:?}"))?,
        )?;
    }
    fs::write(
        directory.join("src/main.rs"),
        include_str!("support/runtime.rs"),
    )?;
    let mut manifest = String::from(
        "[package]\nname = \"generated-bootstrap\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\n",
    );
    for dependency in [
        "hgl-describe",
        "hgl-kernel",
        "hgl-store",
        "hgl-types",
        "hgl-testkit",
    ] {
        let line = format!(
            "{dependency} = {{ path = {:?} }}\n",
            root.join("crates").join(dependency)
        );
        manifest.push_str(&line);
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    Ok(())
}
