//! Measure evaluation after generated finite storage and codecs have been prepared.
use std::fmt::Write as _;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn generated_any_evaluation_uses_reserved_owning_buffers() -> Result<(), Box<dyn std::error::Error>>
{
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = vec![
        root.join("external/hgraph_std/hgl/hgraph/tests/any_values.hgl"),
        root.join("external/hgraph_std/hgl/hgraph/tests/any_bool_values.hgl"),
        root.join("crates/hgl-compiler/tests/fixtures/any_prepared.hgl"),
        root.join("crates/hgl-compiler/tests/fixtures/any_generator_bindings.hgl"),
        root.join("crates/hgl-compiler/tests/fixtures/any_stage67_order.hgl"),
        root.join("crates/hgl-compiler/tests/fixtures/any_stage67_comparisons.hgl"),
        root.join("crates/hgl-compiler/tests/fixtures/any_constructor_owners.hgl"),
    ];
    run(&root, &files, None, |source| source)
}

#[test]
fn constructor_retention_failure_precedes_later_effects() -> Result<(), Box<dyn std::error::Error>>
{
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    run(
        &root,
        &[root.join("crates/hgl-compiler/tests/fixtures/constructor_retention_failure.hgl")],
        Some("test.preflight_capacity"),
        |mut source| {
            // Deliberately undersized private reservations exercise the lower-level retention wall.
            // This is not a claim that these valid source constructors naturally exceed their bounds.
            for (name, field) in [("PreflightFirst", 0), ("PreflightReplacement", 1)] {
                let mut marker = String::from("GlobalStruct");
                for byte in format!("hgraph.std::{name}").bytes() {
                    write!(marker, "{byte:03}")
                        .unwrap_or_else(|_| unreachable!("String formatting"));
                }
                let needle =
                    format!("store.global_state().prepare_value::<{marker}>(\"\\0constructor:");
                if field == 0 {
                    assert!(
                        source.contains(&needle),
                        "missing private constructor reservation {name}"
                    );
                }
                source = source.replace(&needle, &format!("bounds.field{field}=0;{needle}"));
            }
            let needle = "hgl_kernel::EngineError::Node(e)=>hgl_semantics::harness_ir::Failure::Execution(e)";
            assert!(source.contains(needle));
            source.replace(needle,"hgl_kernel::EngineError::Node(mut e)=>{if e.message==\"prepared scalar capacity exceeded\"{e.code=Some(\"test.preflight_capacity\");}hgl_semantics::harness_ir::Failure::Execution(e)}")
        },
    )
}

fn run(
    root: &std::path::Path,
    files: &[PathBuf],
    expected_error: Option<&str>,
    transform: impl FnOnce(String) -> String,
) -> Result<(), Box<dyn std::error::Error>> {
    let libraries = vec![
        root.join("external/hgraph_std/hgl"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(files, &libraries)?;
    let mut suite = hgl_program::compile_module_suite(&sources)?;
    if let Some(code) = expected_error {
        for test in &mut suite.tests {
            test.steps = vec![hgl_semantics::harness_ir::Step::Raises(
                code.into(),
                std::mem::take(&mut test.steps),
            )];
        }
    }
    let source = hgl_program::emit_tests(&suite);
    let start = "store.prepare_collection_inputs();hgl_kernel::run_simulation(";
    let end = ").map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>";
    assert!(source.contains(start) && source.contains(end));
    let source=source.replace(start,"store.prepare_collection_inputs();let (run,allocations)=hgl_alloc_count::count_in(||hgl_kernel::run_simulation(")
        .replace(end,"));if run.is_ok(){assert_eq!(allocations,0,\"prepared Any evaluation allocated in {}\",module_path!());}run.map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>");
    let source = transform(source);
    let directory = std::env::temp_dir().join(format!(
        "hgl-any-allocation-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let mut manifest = format!(
        "[package]\nname=\"hgl-any-allocation-{}\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    let provider = native_provider();
    fs::write(
        directory.join("src/main.rs"),
        format!(
            "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;{provider}\n{source}"
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

fn native_provider() -> String {
    include_str!("fixtures/any_owner_provider.rs").replace(
        "pub use hgl_stdlib::std_native::*;",
        &format!(
            "pub use hgl_stdlib::std_native::*;mod helper {{{}}}pub use helper::*;",
            include_str!("fixtures/any_helper_provider.rs")
        ),
    )
}
