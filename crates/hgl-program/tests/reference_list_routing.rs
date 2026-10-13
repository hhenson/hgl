//! Finite fixed REF-list routing cases run with an active tick allocator.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn fixed_reference_list_routes_execute_without_successful_cycle_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    run(
        &["reference_list_routing.hgl"],
        "reference-list-routing",
        5,
        7,
    )?;
    Ok(())
}
#[test]
fn selected_references_keep_nominal_and_constant_list_graph_projections_live()
-> Result<(), Box<dyn std::error::Error>> {
    run(
        &["reference_list_projection.hgl"],
        "reference-list-projection",
        6,
        7,
    )?;
    Ok(())
}
#[test]
fn graph_reference_children_project_the_inner_live_designation()
-> Result<(), Box<dyn std::error::Error>> {
    run(
        &["reference_ref_child_projection.hgl"],
        "reference-ref-child-projection",
        3,
        3,
    )?;
    Ok(())
}
#[test]
fn runtime_reference_indices_report_bounds_errors_before_selecting_children()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let mut code = String::new();
    let mut calls = String::new();
    for (case, index) in [-1_i64, 3, i64::MAX, 0, 2].into_iter().enumerate() {
        let source = format!(
            "module indices\nfn position()->i64 {{yield 0us:{index}}}\nfn first()->i64 {{yield 0us:7}}\nfn select(index:i64,values:list<ref<i64>,3>)->ref<i64> {{when valid(index) {{return values[index + 0]}}}}\nfn main()->ref<i64> => select(position(),[first(),first(),first()])"
        );
        let program = hgl_program::compile(&[("indices.hgl".into(), source)], "main")?;
        writeln!(
            code,
            "mod case{case} {{{}}}",
            hgl_program::emit_rust(&program)
        )?;
        write!(
            calls,
            "{{let mut registry=hgl_describe::Registry::new();case{case}::register(&mut registry).unwrap();let description=case{case}::main(&registry).unwrap();let mut store=hgl_store::Store::new();let mut built=hgl_describe::instantiate_complete(&description,&registry,&mut store).unwrap();let result=hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig {{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END}});"
        )?;
        if (0..3).contains(&index) {
            calls.push_str("result.unwrap();let reference=store.bindings().reference_value(built.outputs.last().copied().flatten().unwrap());assert!(store.bindings().resolve(reference).is_some());}");
        } else {
            calls.push_str("let failure=result.unwrap_err();assert!(format!(\"{failure:?}\").contains(\"temporal list index out of bounds\"));}");
        }
    }
    writeln!(code, "fn main() {{{calls}}}")?;
    execute(&root, "reference-list-bounds", code)?;
    Ok(())
}
#[test]
fn fixed_connection_lists_keep_ordinary_context_and_reject_mixed_or_wrong_entries() {
    let ordinary = "module contexts\nconst fn empty()->list<i64> => []\nfn count(const values:list<list<i64>,1>)->i64 {when true {return 1}}\nfn main()->i64 => count([[]])";
    for entries in ["[[]]", "[empty()]"] {
        hgl_program::compile(
            &[("contexts.hgl".into(), ordinary.replace("[[]]", entries))],
            "main",
        )
        .unwrap();
    }
    for (entries, expected) in [
        (
            "[first(),7,first()]",
            "mixed ordinary and temporal graph list entries",
        ),
        (
            "[first(),second(),first()]",
            "graph reference list entry type mismatch",
        ),
        ("[first(),first()]", "fixedness mismatch"),
    ] {
        let source = format!(
            "module contexts\nfn first()->i64 {{yield 0us:7}}\nfn second()->bool {{yield 0us:true}}\nfn select(values:list<ref<i64>,3>)->ref<i64> {{when {{return values[0]}}}}\nfn main()->ref<i64> => select({entries})"
        );
        let error = hgl_program::compile(&[("contexts.hgl".into(), source)], "main").unwrap_err();
        assert!(error.contains(expected), "{entries}: {error}");
    }
    let unsupported = "module contexts\nfn select(index:i64,values:list<ref<i64>>)->ref<i64> {when valid(index) {return values[index]}}";
    hgl_program::check_sources(&[("contexts.hgl".into(), unsupported.into())]).unwrap();
    let error = hgl_semantics::tuple_values::indexed(
        hgl_semantics::ir::Value::new(
            hgl_source::Ty::List(
                Box::new(hgl_source::Ty::Ref(Box::new(hgl_source::Ty::I64))),
                None,
            ),
            hgl_semantics::ir::Kind::Input(0, false),
        ),
        hgl_semantics::ir::Value::new(
            hgl_source::Ty::I64,
            hgl_semantics::ir::Kind::Input(1, false),
        ),
    )
    .unwrap_err();
    assert!(
        error.contains("Rust target runtime REF-list selection requires a fixed length"),
        "{error}"
    );
    let producer = "module contexts\nfn produce()->list<ref<i64>,3> {when true {}}\nfn main()->list<ref<i64>,3> => produce()";
    let program =
        hgl_program::compile(&[("contexts.hgl".into(), producer.into())], "main").unwrap();
    let code = hgl_program::emit_rust(&program);
    assert!(code.starts_with("compile_error!("));
    assert!(code.contains("Rust target cannot publish ordinary REF-list output"));
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
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(RUNTIME);
    let output = execute(&root, project_name, code)?;
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("reference matrix measured cycles=")
                && line.ends_with(" tick_allocations=0"))
            .count(),
        evaluations
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
        "{groups} executed tests, {evaluations} evaluations, 0 failures"
    )));
    Ok(output)
}
fn execute(
    root: &std::path::Path,
    project_name: &str,
    code: String,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let project = regression_cache::open(root, project_name)?;
    let directory = &project.path;
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
    generated_cache::workspace_lock(root, directory)?;
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
    Ok(output)
}

const RUNTIME: &str = r#"
struct Provider;
mod native {pub use hgl_stdlib::std_native::*;}
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    let (started,start_allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));
    started.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(start_allocations,0,"prepared reference-list startup allocated");
    let mut cycles=0;let mut total=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){total+=count;}outcome=result;cycles+=1;
    }
    let (stop,stop_allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));assert_eq!(stop_allocations,0,"prepared reference-list cleanup allocated");outcome=hgl_types::finish(outcome,stop);
    eprintln!("reference matrix measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"reference successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
