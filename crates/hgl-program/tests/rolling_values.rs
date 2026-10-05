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
        "hgl-harness",
        "hgl-harness-ir",
        "hgl-rust-ir",
        "hgl-source",
        "hgl-value-eval",
        "hgl-time-context",
        "hgl-testkit",
        "hgl-std-native",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(dir.join("Cargo.toml"), manifest)?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
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
    }
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
