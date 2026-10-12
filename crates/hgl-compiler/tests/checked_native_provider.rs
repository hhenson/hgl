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
fn portable_scalar_sources() -> Result<Vec<(String, String)>, String> {
    let mut sources = sources()?;
    let (_, interface) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with("interface.hgl"))
        .ok_or("interface missing")?;
    interface.push_str("\nexport native type Portable\n");
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    descriptor.push_str(
        "\nscalar Portable canonical \"canonical.Portable\" capabilities {copy,text,equality}\n",
    );
    Ok(sources)
}
#[test]
fn checked_helper_phases_and_missing_images_reject_before_execution() -> Result<(), String> {
    for (from, to, expected) in [
        (
            " image \"provider.rs\"",
            "",
            "missing native helper executable image contract",
        ),
        (
            "{setup,cold,evaluation}",
            "{start}",
            "unavailable in evaluation",
        ),
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
    for (body, phases, expected) in [
        (
            "module phase_case\nuse checked_lengths::{size}\ntest setup {\nlet count=size(\"setup\")\nassert count==5\n}\n",
            "{evaluation}",
            "unavailable in setup",
        ),
        (
            "module phase_case\nuse checked_lengths::{size}\nfn plus(value:i64,const extra:i64)->i64 {when {return value+extra}}\nfn cold(value:i64)->i64 {return plus(value,extra:size(\"cold\"))}\ntest cold_phase {assert eval(cold,value:[1])==[5]}\n",
            "{setup,evaluation}",
            "unavailable in cold",
        ),
    ] {
        let mut sources = sources()?;
        sources[0].1 = body.into();
        let (_, descriptor) = sources
            .iter_mut()
            .find(|(name, _)| name.ends_with(".hgl-native"))
            .ok_or("descriptor missing")?;
        *descriptor = descriptor.replace("{setup,cold,evaluation}", phases);
        let error = hgl_program::compile_module_suite(&sources)
            .err()
            .ok_or_else(|| format!("unexpected admitted phase: {expected}"))?;
        assert!(error.contains(expected), "{error}");
    }
    let mut constant = sources()?;
    constant[0].1 = "module required_constant\nuse checked_lengths::{size}\nfn bounded(value:list<i64,(size(\"checking\"))>)->i64 {when {return 1}}\ntest no_execution {assert eval(bounded,value:[[1]])==[1]}\n".into();
    let error = hgl_program::compile_module_suite(&constant)
        .err()
        .ok_or("native helper admitted as required constant")?;
    assert!(error.contains("constant"), "{error}");
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
        let mut sources = portable_scalar_sources()?;
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

#[test]
fn executable_scalar_marker_is_required_and_preflighted() -> Result<(), String> {
    let mut sources = sources()?;
    let (_, interface) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with("interface.hgl"))
        .ok_or("interface missing")?;
    interface.push_str("\nexport native type Token\n");
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    descriptor.push_str(
        "\nscalar Token canonical \"canonical.Token\" capabilities {copy,text,equality}\n",
    );
    sources[0].1 = sources[0].1.replace(
        "use checked_lengths::{size}",
        "use checked_lengths::{size,Token}",
    );
    sources[0].1.push_str(
        "\nfn token_configuration(value:i64,const tokens:list<Token>)->i64 {when {return value+len(tokens)}}\ntest scalar_configuration {let tokens:list<Token> =[]\nassert eval(token_configuration,value:[1],tokens:tokens)==[1]}\n",
    );
    let error = hgl_program::compile_module_suite(&sources)
        .err()
        .ok_or("missing marker admitted")?;
    assert!(
        error.contains("missing native scalar executable image or target marker: canonical.Token"),
        "{error}"
    );
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    *descriptor = descriptor.replace(
        "capabilities {copy,text,equality}",
        "capabilities {copy,text,equality} marker \"ProviderToken\"",
    );
    let suite = hgl_program::compile_module_suite(&sources)?;
    let generated = hgl_program::emit_tests(&suite);
    assert!(
        generated
            .contains("ProviderToken as hgl_store::native_values::NativeType>::CONTRACT.validate")
    );
    assert!(generated.contains("\"canonical.Token\",[true, true, true, false, false, false]"));
    Ok(())
}

#[test]
fn unused_scalar_descriptor_does_not_link_its_provider() -> Result<(), String> {
    let mut sources = sources()?;
    sources[0].1 = "module native_image_cases\nuse checked_lengths::{Token}\nfn identity(value:i64)->i64 {when {return value}}\ntest unused_scalar {assert eval(identity,[1])==[1]}\n".into();
    let (_, interface) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with("interface.hgl"))
        .ok_or("interface missing")?;
    interface.push_str("\nexport native type Token\n");
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(p, _)| p.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    descriptor.push_str(
        "\nscalar Token canonical \"canonical.Token\" capabilities {copy,text,equality} marker \"ProviderToken\"\n",
    );
    let generated = hgl_program::emit_tests(&hgl_program::compile_module_suite(&sources)?);
    assert!(!generated.contains("ProviderToken"));
    assert!(!generated.contains("canonical.Token"));
    assert!(!generated.contains("checked-lengths-v1"));
    assert!(!generated.contains("provider.rs"));
    Ok(())
}

#[test]
fn selected_helpers_do_not_require_markers_for_unused_portable_scalars() -> Result<(), String> {
    let sources = portable_scalar_sources()?;
    hgl_program::check_sources(&sources)?;
    let generated = hgl_program::emit_tests(&hgl_program::compile_module_suite(&sources)?);
    assert!(generated.contains("checked-lengths-v1") && generated.contains("provider.rs"));
    assert!(!generated.contains("canonical.Portable"));
    Ok(())
}
