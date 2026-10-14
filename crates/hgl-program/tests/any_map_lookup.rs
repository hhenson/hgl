//! Nullable map reads keep absence separate from values and coded failures.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn any_keys_execute_with_exact_borrowed_identity() -> Result<(), Box<dyn std::error::Error>> {
    execute(SOURCE, RUNTIME, "any-map-lookup")
}

#[test]
fn composite_any_keys_execute_on_the_generic_owning_path() -> Result<(), Box<dyn std::error::Error>>
{
    execute(COMPOSITE, GENERIC_RUNTIME, "composite-any-map-lookup")
}

fn execute(source: &str, runtime: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let mut sources = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    sources.insert(0, ("any_lookup.hgl".into(), source.into()));
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert!(!suite.tests.is_empty());
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str("\nstruct Provider;\nmod native {pub use hgl_stdlib::std_native::*;}\n");
    code.push_str(runtime);
    let project = regression_cache::open(&root, name)?;
    generated_cache::write(project.path.join("src/main.rs"), code)?;
    let mut manifest = "[package]\nname=\"nullable-map-lookup\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_owned();
    manifest = manifest.replace("nullable-map-lookup", name);
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
    generated_cache::write(project.path.join("Cargo.toml"), manifest)?;
    generated_cache::workspace_lock(&root, &project.path)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target/source-tests"))
        .current_dir(&project.path);
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
    if name.starts_with("composite") {
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains("generic composite Any lookup:"), "{report}");
        std::fs::write(
            root.join("target/composite-any-lookup-allocation-report.txt"),
            report.as_bytes(),
        )?;
    }
    Ok(())
}

const SOURCE: &str = r#"module any_lookup
fn lookup(book:atomic<map<any,i64>>,key:any)->i64 {when {return get(book,key,default:9)}}
fn configured(const book:map<any,i64>,key:any)->i64 {when {return get(book,key,default:9)}}
test prepared_any_keys {
    let book=map<any,i64>(items:[any(1):7,any(2):0,any("text"):3])
    let empty=map<any,i64>(items:[])
    assert eval(lookup,book:[book,book,empty,book,book],key:[any(2),any(99),any(1),any("text"),any("unknown")])==[0,9,9,3,9]
    assert eval(configured,book:book,key:[any(2),any(99),any("text")])==[0,9,3]
    assert get(book,any(2),default:9)==0
    assert get(book,any(99),default:9)==9
}
"#;
const RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    let (started,allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));
    started.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(allocations,0,"Any lookup startup allocated");
    let mut cycles=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){assert_eq!(count,0,"successful Any lookup tick allocated");}outcome=result;cycles+=1;
    }
    let (stop,allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));assert_eq!(allocations,0,"Any lookup cleanup allocated");outcome=hgl_types::finish(outcome,stop);
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;

const COMPOSITE: &str = r#"module composite_any_lookup
struct Key {first:i64
boxed:any}
fn lookup(book:atomic<map<tuple<i64,any>,i64>>,const key:tuple<i64,any>)->i64 {when {return get(book,key,default:9)}}
fn structured(book:atomic<map<Key,i64>>,const key:Key)->i64 {when {return get(book,key,default:9)}}
test owning_composite_any_keys {
    let book=map<tuple<i64,any>,i64>(items:[(0,any(1)):0,(0,any("text")):7])
    let empty=map<tuple<i64,any>,i64>(items:[])
    assert eval(lookup,book:[book,empty,book],key:(0,any(1)))==[0,9,0]
    assert eval(lookup,book:[book],key:(0,any(99)))==[9]
    let cells=map<Key,i64>(items:[Key(first:0,boxed:any("text")):7])
    assert eval(structured,book:[cells],key:Key(first:0,boxed:any("text")))==[7]
    assert eval(structured,book:[cells],key:Key(first:0,boxed:any("unknown")))==[9]
}
"#;
const GENERIC_RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let mut cycles=0;let mut now=config.start_time;let mut allocations=0;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        allocations+=count;outcome=result;cycles+=1;
    }
    let stop=graph.stop(store,now);outcome=hgl_types::finish(outcome,stop);
    outcome.map_err(hgl_kernel::EngineError::Node)?;
    assert!(cycles>0);assert!(allocations>0,"generic composite owner extraction cost must be visible");
    println!("generic composite Any lookup: {cycles} cycles, {allocations} allocations");Ok(cycles)
}
"#;
