//! Nullable map reads keep absence separate from values and coded failures.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn specified_lookup_examples_execute() -> Result<(), Box<dyn std::error::Error>> {
    execute(false)
}

#[test]
fn extra_lookup_controls_execute() -> Result<(), Box<dyn std::error::Error>> {
    execute(true)
}

fn execute(extra: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let mut sources = hgl_program::library_files::sources(
        &[root.join("external/hgraph_spec/language/examples/nullable-map-lookup.hgl")],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    if extra {
        sources.insert(0, ("extra.hgl".into(), EXTRA.into()));
    }
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert!(!suite.tests.is_empty());
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str("\nstruct Provider;\nmod native {pub use hgl_stdlib::std_native::*;}\n");
    code.push_str(RUNTIME);
    let project = regression_cache::open(&root, "nullable-map-lookup")?;
    generated_cache::write(project.path.join("src/main.rs"), code)?;
    let mut manifest = "[package]\nname=\"nullable-map-lookup\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_owned();
    if extra {
        manifest = manifest.replace("nullable-map-lookup", "extra-map-lookup");
    }
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
    Ok(())
}

#[test]
fn unchecked_lookup_and_wrong_fallback_reject() {
    for (body, expected) in [
        (
            "let value=get(book,key)\nreturn value+1",
            "nullable.required_presence",
        ),
        (
            "let value=get(book,key)\nreturn value",
            "nullable.required_presence",
        ),
        ("return get(book,key,default: false)", "lookup.default_type"),
        (
            "let value=get(book,key)\nif value==null {return 0}\nlet second=get(book,key)\nreturn second",
            "nullable.required_presence",
        ),
        (
            "var value=get(book,key)\nreturn 0",
            "nullable.required_presence",
        ),
    ] {
        let source = format!(
            "module bad\nconst fn lookup(book:map<str,i64>,key:str)->i64 {{{body}}}\nfn main()->i64 => lookup(map<str,i64>(items:[\"X\":0]),\"X\")"
        );
        let error = hgl_program::compile(&[("bad.hgl".into(), source)], "main").unwrap_err();
        assert!(error.contains(expected), "{body}: {error}");
    }
}
const EXTRA: &str = r#"module lookup_extra
const fn flag(book:map<i64,bool>,key:i64)->bool {
    let value=get(book,key)
    if value==null {return true}
    return value
}
const fn text(book:map<i64,str>,key:i64)->str {
    let value=get(book,key)
    if value!=null {return value}
    return "fallback"
}
const fn strict(book:map<str,i64>,key:str)->i64 => at(book,key)
const fn eager(book:map<str,i64>)->i64 => get(book,"X",default: strict(book,"missing"))
fn fallback(book:atomic<map<str,i64>>,key:str)->i64 {when {return get(book,key,default: 9)}}
fn configured(const book:map<i64,i64>,key:i64)->i64 {when {let value=get(book,key)
if value==null {return 9}
return value}}
fn composite(book:atomic<map<tuple<i64,i64>,i64>>,const key:tuple<i64,i64>)->i64 {when {return get(book,key,default:9)}}
fn at_atomic(book:atomic<map<str,i64>>,key:str)->i64 {when {return at(book,key)}}
test composite_keys_and_atomic_at {
    let book=map<tuple<i64,i64>,i64>(items:[(0,1):7])
    assert eval(composite,book:[book],key:(0,1))==[7]
    assert eval(composite,book:[book],key:(1,0))==[9]
    assert raises("value.missing_key") {eval(at_atomic,book:[map<str,i64>(items:[])],key:["X"])}
}
test atomic_and_configuration_fallback {
    let zero=map<str,i64>(items:["X":0])
    let empty=map<str,i64>(items:[])
    assert eval(fallback,book:[zero,empty,zero],key:["X"])==[0,9,0]
    assert eval(configured,book:map<i64,i64>(items:[0:0]),key:[0,1,0])==[0,9,0]
}
const fn nested(book:map<i64,map<str,i64>>)->i64 {
    let child=get(book,0)
    if child==null {return 9}
    let value=get(child,"X")
    if value==null {return 9}
    return value
}
const fn collection(book:map<i64,list<i64>>)->list<i64> => get(book,0,default:[])
test zero_false_empty_and_eager_fallback {
    assert flag(map<i64,bool>(items:[0:false]),0)==false
    assert text(map<i64,str>(items:[0:""]),0)==""
    assert text(map<i64,str>(items:[]),0)=="fallback"
    assert nested(map<i64,map<str,i64>>(items:[0:map<str,i64>(items:["X":0])]))==0
    assert nested(map<i64,map<str,i64>>(items:[]))==9
    assert len(collection(map<i64,list<i64>>(items:[])))==0
    assert raises("value.missing_key") {eager(map<str,i64>(items:["X":7]))}
}
"#;

const RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    let (started,allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));
    started.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(allocations,0,"lookup startup allocated");
    let mut cycles=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){assert_eq!(count,0,"successful lookup tick allocated");}outcome=result;cycles+=1;
    }
    let (stop,allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));assert_eq!(allocations,0,"lookup cleanup allocated");outcome=hgl_types::finish(outcome,stop);
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
