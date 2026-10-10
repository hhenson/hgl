//! Ordinary imported helpers bind data-selected code and validate images before execution.
use std::{fmt::Write as _, fs, path::PathBuf, process::Command, time::SystemTime};

fn root() -> Result<PathBuf, std::io::Error> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
}
fn sources() -> Result<Vec<(String, String)>, String> {
    let root = root().map_err(|e| e.to_string())?;
    let fixture = root.join("crates/hgl-compiler/tests/fixtures/checked_native_provider");
    hgl_program::library_files::sources(
        &[
            fixture.join("cases.hgl"),
            fixture.join("interface.hgl"),
            fixture.join("provider.hgl-native"),
            root.join("external/hgraph_std/hgl/hgraph/replay_record.hgl"),
            root.join("external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
        ],
        &[],
    )
}
#[test]
fn checked_helper_phases_and_missing_images_reject_before_execution() -> Result<(), String> {
    for (from, to, expected) in [
        (
            " image \"provider.rs\"",
            "",
            "missing native helper executable image contract",
        ),
        ("{evaluation}", "{start}", "unavailable in evaluation"),
    ] {
        let mut sources = sources()?;
        let (_, descriptor) = sources
            .iter_mut()
            .find(|(name, _)| name.ends_with(".hgl-native"))
            .ok_or("descriptor missing")?;
        *descriptor = descriptor.replace(from, to);
        let error = hgl_program::compile_module_suite(&sources)
            .err()
            .ok_or("unexpected admitted provider")?;
        assert!(error.contains(expected), "{error}");
    }
    let mut sources = sources()?;
    let (name, descriptor) = sources
        .iter_mut()
        .find(|(name, _)| name.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    *descriptor = descriptor.replace("provider.rs", "unavailable.rs");
    assert!(name.ends_with(".hgl-native"));
    hgl_program::compile_module_suite(&sources)?;
    Ok(())
}
#[test]
fn selected_provider_executes_without_tick_allocations_and_rejects_mismatched_images()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-checked-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let mut manifest = format!(
        "[package]\nname=\"hgl-checked-native-{}\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
        "hgl-alloc-count",
    ] {
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name).to_string_lossy()
        )?;
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    for mismatch in [false, true] {
        let mut sources = sources()?;
        if mismatch {
            let (_, descriptor) = sources
                .iter_mut()
                .find(|(name, _)| name.ends_with(".hgl-native"))
                .ok_or("descriptor missing")?;
            *descriptor = descriptor.replace("checked-lengths-v1", "wrong-image");
        }
        let source = hgl_program::emit_tests(&hgl_program::compile_module_suite(&sources)?);
        let start = "store.prepare_collection_inputs();hgl_kernel::run_simulation(";
        let end = ").map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>";
        assert!(source.contains(start) && source.contains(end));
        let source=source.replace(start,"store.prepare_collection_inputs();let (run,allocations)=hgl_alloc_count::count_in(||hgl_kernel::run_simulation(").replace(end,"));if run.is_ok(){assert_eq!(allocations,0,\"checked provider evaluation allocated\");}run.map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>");
        assert!(!source.contains("<crate::Provider as Native>"));
        fs::write(
            directory.join("src/main.rs"),
            format!(
                "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;\n{source}"
            ),
        )?;
        let mut command = Command::new(env!("CARGO"));
        command
            .args(["run", "--offline", "--quiet"])
            .arg("--target-dir")
            .arg(root.join("target/source-tests"))
            .current_dir(&directory);
        if !cfg!(debug_assertions) {
            command.arg("--release");
        }
        let output = command.output()?;
        let messages = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if mismatch {
            assert!(!output.status.success(), "mismatched image executed");
            assert!(
                messages.contains("native descriptor/provider identity or fingerprint mismatch"),
                "{messages}"
            );
        } else {
            assert!(output.status.success(), "{messages}");
        }
    }
    fs::remove_dir_all(directory)?;
    Ok(())
}
