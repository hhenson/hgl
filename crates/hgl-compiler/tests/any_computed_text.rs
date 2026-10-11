//! Measure evaluation after generated finite storage and codecs have been prepared.
use std::fmt::Write as _;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn computed_text_boxes_use_exact_prepared_bounds() -> Result<(), Box<dyn std::error::Error>> {
    run("any_computed_text", true)
}
#[test]
fn formatter_helper_boxes_retain_complete_text() -> Result<(), Box<dyn std::error::Error>> {
    // One replay item avoids funding the helper box through unrelated list frame bounds.
    run("any_helper_text", false)
}
fn run(fixture: &str, allocation_free: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = vec![root.join(format!("crates/hgl-compiler/tests/fixtures/{fixture}.hgl"))];
    let libraries = vec![
        root.join("external/hgraph_std/hgl"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(&files, &libraries)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    let source = hgl_program::emit_tests(&suite);
    let start = "store.prepare_collection_inputs();hgl_kernel::run_simulation(";
    let end = ").map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>";
    assert!(source.contains(start) && source.contains(end));
    let allocation_check = if allocation_free {
        "assert_eq!(allocations,0,\"prepared Any evaluation allocated in {}\",module_path!());"
    } else {
        "println!(\"formatter helper {} allocations: {allocations}\",module_path!());"
    };
    let source=source.replace(start,"store.prepare_collection_inputs();let (run,allocations)=hgl_alloc_count::count_in(||hgl_kernel::run_simulation(")
        .replace(end,&format!("));if run.is_ok(){{{allocation_check}}}run.map_err(|e|match e {{hgl_kernel::EngineError::Node(e)=>"));
    let directory = std::env::temp_dir().join(format!(
        "hgl-any-computed-text-{fixture}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let mut manifest = format!(
        "[package]\nname=\"hgl-any-computed-text-{fixture}-{}\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
        std::process::id()
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-stdlib",
        "hgl-alloc-count",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    let provider = include_str!("fixtures/any_computed_text_provider.rs");
    fs::write(
        directory.join("src/main.rs"),
        format!(
            "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;struct Provider;mod native{{pub use hgl_stdlib::std_native::*;{provider}}}\n{source}"
        ),
    )?;
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .args(if cfg!(debug_assertions) {
            &[][..]
        } else {
            &["--release"][..]
        })
        .arg("--target-dir")
        .arg(root.join("target/source-tests"))
        .current_dir(&directory)
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&directory)?;
    Ok(())
}
