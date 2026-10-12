//! Immutable latest authored REF transport cases run with an active tick allocator.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn authored_reference_transport_matrix_executes_24_groups_and_50_evaluations_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = [
        "reference_values",
        "reference_projection_values",
        "reference_structural_values",
        "reference_leaf_values",
    ]
    .map(|name| {
        root.join(format!(
            "crates/hgl-program/tests/fixtures/reference_matrix/{name}.hgl"
        ))
    });
    let libraries = [
        root.join("external/hgraph_std/hgl/hgraph"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(&files, &libraries)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 24);
    let project = regression_cache::open(&root, "reference-matrix")?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"reference-matrix\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "reference_graph_projections_retarget_even_when_child_values_are_equal ... ok [executed]"
    ));
    assert!(String::from_utf8_lossy(&output.stderr).contains("reference matrix measured cycles="));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("24 executed tests, 50 evaluations, 0 failures")
    );
    Ok(())
}

const RUNTIME: &str = r#"
struct Provider;
mod native {pub use hgl_stdlib::std_native::*;}
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let mut cycles=0;let mut total=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){total+=count;}outcome=result;cycles+=1;
    }
    let stop=graph.stop(store,now);outcome=hgl_types::finish(outcome,stop);
    eprintln!("reference matrix measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"reference successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
