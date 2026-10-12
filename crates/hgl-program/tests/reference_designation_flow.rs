//! Cold routes follow designation publications, excluding unrelated REF observations.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn designation_dataflow_executes_without_unrelated_views_or_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = [
        root.join("crates/hgl-program/tests/fixtures/reference_designation_flow.hgl"),
        root.join("external/hgraph_std/hgl/hgraph/replay_record.hgl"),
        root.join("external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
    ];
    let sources = hgl_program::library_files::sources(&files, &[])?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 5);
    check_metadata_routes(&suite, &sources)?;
    let project = regression_cache::open(&root, "reference-designation-flow")?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"reference-designation-flow\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name).to_string_lossy()
        )?;
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
            .contains("5 executed tests, 6 evaluations, 0 failures")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("designation flow measured cycles="));
    Ok(())
}

fn check_metadata_routes(
    suite: &hgl_program::Suite,
    sources: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = suite
        .plans
        .iter()
        .find(|plan| {
            plan.nodes
                .iter()
                .any(|node| node.name.ends_with("alias_route"))
        })
        .ok_or("missing alias route plan")?;
    let node = plan
        .nodes
        .iter()
        .find(|node| node.name.ends_with("alias_route"))
        .ok_or("missing alias route")?;
    let mut selected = hgl_program::compile_module_suite(sources)?;
    selected.select(&["same_shape_metadata_ref_is_not_a_returned_target"])?;
    let generated = hgl_program::emit_tests(&selected);
    assert!(generated.contains(&format!(
        "let target=store.bindings().fixed_output(built.outputs[{}]",
        node.inputs[0].1
    )));
    assert!(!generated.contains(&format!(
        "let target=store.bindings().fixed_output(built.outputs[{}]",
        node.inputs[1].1
    )));
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
    eprintln!("designation flow measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"designation successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
