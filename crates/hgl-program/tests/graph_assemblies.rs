//! Graph constructor paths preserve endpoint identity and measure every successful cycle.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn graph_constructor_admission_preserves_scalar_and_node_construction() {
    let prefix = "module assembly\nstruct Pair {left:i64\nright:i64}\nfn scalar(const value:i64)->i64 {yield 0us:value}\n";
    let sources = [
        "fn main()->i64 {let pair=Pair(right:9,left:7)\nscalar(pair.left)}",
        "fn ordinary(value:i64)->i64 {when {let pair=Pair(right:9,left:7)\nreturn value+pair.left}}\nfn main()->i64 => ordinary(scalar(3))",
    ];
    for body in sources {
        let program = hgl_program::compile(
            &[("assembly.hgl".into(), format!("{prefix}{body}"))],
            "main",
        )
        .unwrap();
        assert!(!hgl_program::emit_rust(&program).contains("hgl.assembly"));
    }
    let program = hgl_program::compile(
        &[(
            "assembly.hgl".into(),
            format!("{prefix}fn main()->Pair => Pair(right:scalar(9),left:scalar(7))"),
        )],
        "main",
    )
    .unwrap();
    let code = hgl_program::emit_rust(&program);
    assert!(code.contains("connect_path("));
    assert!(code.contains("active_inputs:Some(vec![])"));
}

#[test]
fn graph_constructor_field_checks_precede_assembly() {
    for fields in [
        "left:left",
        "left:left,left:right,right:right",
        "left:left,unknown:right",
        "left:true,right:right",
    ] {
        let source = format!(
            "module assembly\nstruct Pair {{left:i64\nright:i64}}\nfn source()->i64 {{yield 0us:7}}\nfn pair(left:i64,right:i64)->Pair => Pair({fields})\nfn main()->Pair => pair(source(),source())"
        );
        assert!(
            hgl_program::compile(&[("assembly.hgl".into(), source)], "main").is_err(),
            "{fields}"
        );
    }
}

#[test]
fn fixed_nominal_assemblies_execute_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let files = [root.join("crates/hgl-program/tests/fixtures/graph_assemblies.hgl")];
    let libraries = [
        root.join("external/hgraph_std/hgl/hgraph"),
        root.join("native/stdlib"),
    ];
    let sources = hgl_program::library_files::sources(&files, &libraries)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 6);
    let project = regression_cache::open(&root, "graph-assemblies")?;
    let directory = &project.path;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(")
        .replace(
            "store.prepare_collection_inputs();",
            "crate::verify_assemblies(&graph,&built,&store);store.prepare_collection_inputs();",
        );
    code.push_str(RUNTIME);
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"graph-assemblies\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
            .contains("6 executed tests, 8 evaluations, 0 failures")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("tick_allocations=0"));
    Ok(())
}

const RUNTIME: &str = r#"
struct Provider;
mod native {pub use hgl_stdlib::std_native::*;}
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn verify_assemblies(graph:&hgl_describe::GraphDescription,built:&hgl_describe::BuiltGraph,store:&hgl_store::Store){
    let mut count=0;
    for (index,node) in graph.nodes.iter().enumerate(){
        if !node.implementation.starts_with("hgl.assembly#"){continue;}
        count+=1;
        let value=built.inputs[index][0];
        assert!(store.bindings().input(value).source.is_none());
        assert!(store.bindings().input_reference(value).items.is_some());
        for edge in graph.edges.iter().filter(|edge|edge.target.node as usize==index){
            let child=built.input(&edge.target,store).expect("checked assembly field");
            let source=built.output(&edge.source,store).expect("checked source identity");
            assert_eq!(store.bindings().input(child).source,Some(source));
            assert_eq!(store.bindings().input_reference(child),store.reference(source));
            assert!(!store.bindings().input(child).active);
        }
    }
    assert!(count>0);
}
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
