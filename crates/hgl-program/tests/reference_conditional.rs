//! Immutable latest authored REF transport cases run with an active tick allocator.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn authored_reference_ownership_matrix_executes_4_groups_and_8_evaluations_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    let output = run(
        &[
            "reference_matrix/reference_conditional_expiry.hgl",
            "reference_matrix/reference_owner_export_values.hgl",
        ],
        "reference-conditional",
        4,
        8,
    )?;
    assert!(String::from_utf8_lossy(&output.stdout).contains("reference_exported_child_tree_keeps_sparse_ticks_and_samples_changed_children ... ok [executed]"));
    Ok(())
}
#[test]
fn finite_activation_bound_includes_source_cycles_after_the_dense_input_horizon()
-> Result<(), Box<dyn std::error::Error>> {
    let output = run(&["conditional_arrivals.hgl"], "conditional-arrivals", 1, 1)?;
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("reference matrix measured cycles=8 tick_allocations=0")
    );
    Ok(())
}
#[test]
fn conditional_export_keeps_mutable_var_authority_and_ordinary_harness_assignment()
-> Result<(), Box<dyn std::error::Error>> {
    run(
        &["conditional_reassignment.hgl"],
        "conditional-reassignment",
        2,
        3,
    )?;
    Ok(())
}
fn run(
    names: &[&str],
    project_name: &str,
    groups: usize,
    evaluations: usize,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = names
        .iter()
        .map(|name| root.join("crates/hgl-program/tests/fixtures").join(name))
        .collect::<Vec<_>>();
    let libraries = [
        root.join("external/hgraph_std/hgl/hgraph"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(&files, &libraries)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), groups);
    let project = regression_cache::open(&root, project_name)?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = format!(
        "[package]\nname={project_name:?}\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n"
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("reference matrix measured cycles="));
    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
        "{groups} executed tests, {evaluations} evaluations, 0 failures"
    )));
    Ok(output)
}

const RUNTIME: &str = r#"
struct Provider;
mod native {pub use hgl_stdlib::std_native::*;}
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    let (started,start_allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));
    started.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(start_allocations,0,"prepared conditional startup allocated");
    let mut cycles=0;let mut total=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){total+=count;}outcome=result;cycles+=1;
    }
    let (stop,stop_allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));assert_eq!(stop_allocations,0,"prepared conditional cleanup allocated");outcome=hgl_types::finish(outcome,stop);
    eprintln!("reference matrix measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"reference successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
