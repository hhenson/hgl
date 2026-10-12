//! Native text inside a complete atomic Tuple retains a distinct prepared owner.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn native_reference_tuple_results_execute_without_successful_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = [root.join("crates/hgl-program/tests/fixtures/reference_native_tuple.hgl")];
    let libraries = [
        root.join("external/hgraph_std/hgl/hgraph"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(&files, &libraries)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 3);
    let project = regression_cache::open(&root, "reference-native-tuple")?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"reference-native-tuple\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    generated_cache::write(directory.join("Cargo.toml"), manifest)?;
    generated_cache::workspace_lock(&root, directory)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(directory);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("held_native_identity_and_text_are_independently_owned ... ok [executed]")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("reference tuple measured cycles="));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("3 executed tests, 5 evaluations, 0 failures")
    );
    Ok(())
}

const RUNTIME: &str = r#"
struct Provider;
mod native {pub use hgl_stdlib::std_native::*;}
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let reserved=store.global_state().values().slot_counts();
    let mut cycles=0;let mut total=0;let mut now=config.start_time;
    while !graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));result.map_err(hgl_kernel::EngineError::Node)?;
        assert_eq!(store.global_state().values().slot_counts(),reserved);total+=count;cycles+=1;
    }
    graph.stop(store,now).map_err(hgl_kernel::EngineError::Node)?;
    eprintln!("reference tuple measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"native tuple publication and recording allocated");Ok(cycles)
}
"#;
