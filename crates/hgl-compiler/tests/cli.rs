//! CLI checking and failed emission must not execute or partially write a program.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_checks_emits_and_preserves_output_on_failure() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "hgl-cli-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(&directory)?;
    let source = directory.join("main.hgl");
    let part = directory.join("rust.hgl");
    let output = directory.join("out.rs");
    fs::write(
        &source,
        include_str!("../../../examples/const-debug/main.hgl"),
    )?;
    fs::write(
        &part,
        include_str!("../../../examples/const-debug/rust.hgl"),
    )?;
    let checked = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("check")
        .arg(&source)
        .output()?;
    assert!(checked.status.success());
    assert!(checked.stdout.is_empty());
    let emitted = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("emit-rust")
        .arg(&source)
        .arg("--part")
        .arg(&part)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(
        emitted.status.success(),
        "{}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    assert!(emitted.stdout.is_empty());
    let original = fs::read_to_string(&output)?;
    assert!(original.contains("impl hgl_kernel::Node"));
    let missing = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("emit-rust")
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(!missing.status.success());
    assert_eq!(fs::read_to_string(&output)?, original);
    fs::write(&source, "module bad\nfn broken(\n")?;
    let broken = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("emit-rust")
        .arg(&source)
        .arg("--part")
        .arg(&part)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(!broken.status.success());
    assert!(String::from_utf8_lossy(&broken.stderr).contains("main.hgl:3:1"));
    assert_eq!(fs::read_to_string(&output)?, original);
    fs::remove_dir_all(directory)?;
    Ok(())
}
