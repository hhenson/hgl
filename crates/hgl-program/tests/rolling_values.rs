//! Finite rolling atomic shared semantics and actual evaluation allocation checks.
use hgl_program::compile_tests;
fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("rolling.hgl".into(), source.into()),
        (
            "pass.hgl".into(),
            "module hgraph.std\nfn pass_through<T>(value:T)->T {when {return delta_value(value)}}"
                .into(),
        ),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}
#[test]
fn shared_rolling_cases_typecheck() {
    let result = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/rolling_values.hgl"
    )));
    assert!(result.is_ok(), "{result:?}");
}
#[test]
fn shared_rolling_cases_execute_without_tick_allocations() -> Result<(), Box<dyn std::error::Error>>
{
    run_shared(
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/rolling_values.hgl"),
        true,
    )
}
fn run_shared(source: &str, measure: bool) -> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(source, measure, "")
}
fn run_with_runtime(
    source: &str,
    measure: bool,
    runtime: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-rolling-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    let suite = compile_tests(&sources(source))?;
    let mut code = hgl_program::emit_tests(&suite);
    if measure {
        code = code.replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
        code.push_str(r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph, store:&mut hgl_store::Store, config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError> {
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let mut cycles=0; let mut total=0; let mut now=config.start_time;
    while !graph.stop_requested() {
        let next=graph.next_scheduled_time(); if next>=config.end_time {break;} now=next;
        let (result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        result.map_err(hgl_kernel::EngineError::Node)?; total+=count; cycles+=1;
    }
    graph.stop(store,now).map_err(hgl_kernel::EngineError::Node)?;
    eprintln!("rolling measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"rolling first/repeated publication and recording must allocate nothing");
    Ok(cycles)
}
"#);
    }
    code.push_str("struct Provider;\n");
    code.push_str(runtime);
    fs::write(dir.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"rolling-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-alloc-count",
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-stdlib",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, &dir))?;
    // The gate runs the suite in both profiles; each build follows the profile of this test binary.
    let profile: Vec<&str> = if cfg!(debug_assertions) {
        vec![]
    } else {
        vec!["--release"]
    };
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
        .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .args(profile)
        .current_dir(&dir)
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn invalid_size_kinds_boundaries_and_ordinary_window_payloads_are_rejected() {
    for shape in [
        "rolling<i64,0>",
        "rolling<i64,2,0>",
        "rolling<i64,2,3>",
        "rolling<i64,5us,1>",
        "rolling<i64,-1us>",
        "rolling<i64,2us,-1us>",
        "atomic<rolling<i64,2>>",
        "rolling<rolling<i64,2>,2>",
    ] {
        let source = format!(
            "module hgraph.std part bad_window\ntest {{fn id(value:{shape})->{shape} =>pass_through(value)\ntest bad {{assert eval(id,[])==[]}}}}"
        );
        assert!(
            compile_tests(&sources(&source)).is_err(),
            "admitted {shape}"
        );
    }
}
#[test]
fn equivalent_bounds_and_contextual_arrivals_preserve_exact_window_identity() -> Result<(), String>
{
    let source = "module hgraph.std part window_identity\ntest {fn id(value:rolling<i64,1s>)->rolling<i64,1000000us,1000ms> =>pass_through(value)\ntest same {assert eval(id,[1,_,1])==[1,_,1]}}";
    compile_tests(&sources(source))?;
    let source = "module hgraph.std part window_identity\ntest {fn ticks(value:rolling<i64,2>)->rolling<i64,2> =>pass_through(value)\nfn wrong(value:rolling<i64,2us>)->rolling<i64,2us> =>ticks(value)\ntest bad {assert eval(wrong,[1])==[1]}}";
    assert!(compile_tests(&sources(source)).is_err());
    Ok(())
}

#[test]
fn generic_payloads_and_structural_window_children_execute_without_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"
module hgraph.std part rolling_nested
fn identity<V>(value: rolling<V,2>) -> rolling<V,2> => pass_through(value)
fn nested(value: tuple<rolling<str,2>,i64>) -> tuple<rolling<str,2>,i64> => pass_through(value)
fn strings(value: rolling<str,2>) -> rolling<str,2> => identity(value)
test {
 test generic_payload { assert eval(strings,["one",_,"two","two"]) == ["one",_,"two","two"] }
 test nested_payload {
  assert eval(nested,[delta<tuple<rolling<str,2>,i64>>(items: [0:"one",1:1]),_,delta<tuple<rolling<str,2>,i64>>(items: [0:"two"])]) == [delta<tuple<rolling<str,2>,i64>>(items: [0:"one",1:1]),_,delta<tuple<rolling<str,2>,i64>>(items: [0:"two"])]
 }
}
"#,
        true,
    )
}

#[test]
fn scalar_text_composition_publishes_arrivals_into_rolling_storage()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"
module hgraph.std part rolling_text
fn concat(a:str,b:str)->rolling<str,2> {when {return a+"!"+b}}
fn scalar(a:str,b:str)->str {when {return a+"!"+b}}
fn ready(value:rolling<str,2>)->bool {when {return all_valid(value)}}
fn observed(a:str,b:str)->bool => ready(concat(a,b))
test {test concat {
 assert eval(scalar,a:["a","",_,"b"],b:["x","y",_,"z"])==["a!x","!y",_,"b!z"]
 assert eval(concat,a:["a","",_,"b"],b:["x","y",_,"z"])==["a!x","!y",_,"b!z"]
 assert eval(observed,a:["a","",_,"b"],b:["x","y",_,"z"])==[false,true,_,true]
}}
"#,
        true,
    )
}

#[test]
fn direct_finite_sources_prepare_tick_duration_and_owning_arrivals()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"
module hgraph.std part direct_sources
fn context(value:i64) {when {}}
fn ticks()->rolling<i64,2> {yield 1us:1
yield 1us:2}
fn duration()->rolling<i64,5us,0us> {yield 1us:1
yield 1us:2
yield 1us:3}
fn strings()->rolling<str,2> {yield 1us:"one"
yield 1us:"longer"
yield 1us:"one"}
fn tick_graph(value:i64)->rolling<i64,2> {context(value)
ticks()}
fn duration_graph(value:i64)->rolling<i64,5us,0us> {context(value)
duration()}
fn string_graph(value:i64)->rolling<str,2> {context(value)
strings()}
test {
 test direct_ticks {assert eval(tick_graph,value:[0,0,0,0]) == [_,1,2,_]}
 test direct_duration {assert eval(duration_graph,value:[0,0,0,0]) == [_,1,2,3]}
 test direct_strings {assert eval(string_graph,value:[0,0,0,0]) == [_,"one","longer","one"]}
}
"#,
        true,
    )
}
#[test]
fn finite_source_proof_never_executes_native_payloads_or_lifecycle_hooks()
-> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(
        r"
module hgraph.std part direct_once
native const fn mark(value:i64)->i64 throws
native const fn mark(value:i64)->i64 throws {}
native const fn configuration(value:i64)->i64 throws
native const fn configuration(value:i64)->i64 throws {}
native const fn begin(value:i64) throws
native const fn begin(value:i64) throws {}
native const fn end(value:i64) throws
native const fn end(value:i64) throws {}
fn context(value:i64) {start {begin(0)}
stop {end(0)}
when {}}
const fn seed()->i64 => configuration(1)
fn source(const seed:i64)->rolling<i64,2> {if true {yield 1us:mark(seed)
yield 1us:mark(2)} else {yield 1us:mark(3)}}
fn graph(value:i64)->rolling<i64,2> {context(value)
source(seed())}
test once {assert eval(graph,value:[0,0,0,0]) == [_,1,2,_]}
",
        true,
        r"
mod native {
static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static STARTS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static CONFIGS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
pub fn configuration_i64(value:i64)->hgl_types::NodeResult<i64> {assert_eq!(CONFIGS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0);Ok(value)}
pub fn mark_i64(value:i64)->hgl_types::NodeResult<i64> {CALLS.fetch_add(1,std::sync::atomic::Ordering::SeqCst);Ok(value)}
pub fn begin_i64(_:i64)->hgl_types::NodeResult {assert_eq!(STARTS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0);assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst),0);Ok(())}
pub fn end_i64(_:i64)->hgl_types::NodeResult {assert_eq!(CONFIGS.load(std::sync::atomic::Ordering::SeqCst),1);assert_eq!(STARTS.load(std::sync::atomic::Ordering::SeqCst),1);assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst),2);Ok(())}
}",
    )
}

#[test]
fn structural_fields_preserve_tick_and_duration_window_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"
module hgraph.std part rolling_fields
const fn width()->duration => 5us
struct WindowBox {value:rolling<i64,2>}
struct DurationBox {value:rolling<str,width(),0us>}
fn ticks(value:WindowBox)->WindowBox => pass_through(value)
fn durations(value:DurationBox)->DurationBox => pass_through(value)
test {test fields {
 assert eval(ticks,[delta<WindowBox>(value:1),_,delta<WindowBox>(value:1)]) == [delta<WindowBox>(value:1),_,delta<WindowBox>(value:1)]
 assert eval(durations,[delta<DurationBox>(value:"first"),_,delta<DurationBox>(value:"second")]) == [delta<DurationBox>(value:"first"),_,delta<DurationBox>(value:"second")]
}}
"#,
        true,
    )
}

#[test]
fn window_fields_do_not_make_ordinary_atomic_payloads() {
    for body in [
        "fn bad(value:atomic<WindowBox>)->atomic<WindowBox> => pass_through(value)\ntest {test bad {assert eval(bad,[])==[]}}",
        "test {test bad {let value=WindowBox(value:1)}}",
    ] {
        let source = format!(
            "module hgraph.std part rolling_payload_boundary\nstruct WindowBox {{value:rolling<i64,2>}}\n{body}"
        );
        assert!(compile_tests(&sources(&source)).is_err(), "admitted {body}");
    }
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &std::path::Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
