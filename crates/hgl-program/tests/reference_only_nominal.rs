//! REF-only nominal assemblies retain exact shape markers without ordinary consumers.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn ref_only_assemblies_execute_without_tick_allocations() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = [
        root.join("crates/hgl-program/tests/fixtures/reference_only_nominal.hgl"),
        root.join("external/hgraph_std/hgl/hgraph/replay_record.hgl"),
        root.join("external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
    ];
    let sources = hgl_program::library_files::sources(&files, &[])?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 2);
    assert!(
        suite
            .plans
            .iter()
            .flat_map(|plan| &plan.nodes)
            .any(|node| node.name == "hgl.assembly")
    );
    for node in suite.plans.iter().flat_map(|plan| &plan.nodes) {
        if node.name != "hgl.assembly" {
            assert!(
                node.inputs
                    .iter()
                    .all(|(_, _, ty)| !matches!(ty, hgl_source::Ty::Struct(..))),
                "ordinary nominal consumer masks REF-only reachability: {}",
                node.name
            );
        }
    }
    let project = regression_cache::open(&root, "reference-only-nominal")?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"reference-only-nominal\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
            .contains("2 executed tests, 3 evaluations, 0 failures")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("REF-only nominal measured cycles="));
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
    eprintln!("REF-only nominal measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"REF-only nominal successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
