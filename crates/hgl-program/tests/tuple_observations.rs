//! Ordinary tuple construction and retained observations from the shared specification.
use hgl_program::compile_tests;

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("tuple_observations.hgl".into(), source.into()),
        (
            "tuple_example.hgl".into(),
            include_str!(
                "../../../external/hgraph_spec/language/examples/ordinary-tuple-values.hgl"
            )
            .into(),
        ),
        (
            "tuple_extra.hgl".into(),
            include_str!("fixtures/tuple_observations.hgl").into(),
        ),
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
fn all_shared_tuple_observations_cases_typecheck() {
    let result = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/tuple_observation_values.hgl"
    )));
    assert!(result.is_ok(), "{result:?}");
}
#[test]
fn shared_tuple_observations_cases_execute_in_debug_and_release()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(false)
}
#[test]
fn tuple_observations_generated_cycles_allocate_nothing() -> Result<(), Box<dyn std::error::Error>>
{
    run_shared(true)
}
fn run_shared(measure: bool) -> Result<(), Box<dyn std::error::Error>> {
    run_source(
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/tuple_observation_values.hgl"),
        measure,
        false,
    )
}
fn run_source(
    source: &str,
    measure: bool,
    expect_failure: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = scratch_dir("hgl-tuple-observations")?;
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
    eprintln!("tuple_observations measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"tuple_observations first/repeated publication and recording must allocate nothing");
    Ok(cycles)
}
"#);
    }
    code.push_str("struct Provider;mod native {pub fn echo_i64(value:i64)->i64 {value} pub fn text_len_str(value:&str)->i64 {value.len() as i64} pub fn both_str_str(first:&str,second:&str)->i64 {(first.len()+second.len()) as i64}}\n");

    fs::write(dir.join("src/main.rs"), code)?;
    manifest(&root, &dir)?;
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
    if expect_failure {
        assert!(
            !output.status.success(),
            "invalid embedded payload must fail"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("ordinary tuple input is invalid"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
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
fn ordinary_tuple_errors_do_not_borrow_delta_codes() {
    for source in [
        "module hgraph.std part tuple_wrong\nfn bad(value:i64)->tuple<i64,bool>{when{return(value,1)}}\ntest bad {eval(bad,[1])}",
        "module hgraph.std part tuple_wrong\nfn bad(value:i64)->i64 {when {let copy=value\nlet pair=(copy,)\nreturn pair[2]}}\ntest bad {eval(bad,[1])}",
    ] {
        let error = compile_tests(&sources(source)).expect_err("ordinary tuple misuse must reject");
        assert!(!error.contains("delta.type_mismatch"), "{error:?}");
    }
}

#[test]
fn published_tuple_example_executes_and_constant_controls_reject() {
    let example =
        include_str!("../../../external/hgraph_spec/language/examples/ordinary-tuple-values.hgl");
    let mut inputs = sources(example);
    inputs.retain(|(file, _)| file != "tuple_example.hgl");
    let suite = compile_tests(&inputs).expect("published tuple example");
    assert!(!hgl_program::emit_tests(&suite).is_empty());
    for source in [
        include_str!(
            "../../../external/hgraph_spec/compiler/tuple_construction/reject-dynamic-const-argument.hgl"
        ),
        include_str!(
            "../../../external/hgraph_spec/compiler/tuple_construction/reject-dynamic-default.hgl"
        ),
    ] {
        let diagnostics =
            hgl_program::diagnostics(&[("tuple_const_bad.hgl".into(), source.into())]);
        assert!(
            !diagnostics.is_empty(),
            "constant phase must reject: {source}"
        );
        assert!(
            diagnostics.iter().all(|d| d.issue.code.is_none()),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn unused_clock_and_temporal_alias_tuple_const_arguments_reject() {
    for source in [
        "module tuple_clock\nfn configured(const pair:tuple<datetime,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {inject clock\nwhen {let timestamp=clock.evaluation_time\nlet rejected=configured((timestamp,false))\nreturn value}}",
        "module tuple_branch\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {var alias:i64=0\nwhen {if value>0 {alias=value}\nlet rejected=configured((alias,false))\nreturn value}}",
        "module tuple_for_assignment\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64,members:set<i64>)->i64 {var alias:i64=0\nwhen {for member in elements(members,added) {alias=value}\nlet rejected=configured((alias,false))\nreturn value}}",
        "module tuple_for_member\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64,members:set<i64>)->i64 {when {for member in elements(members,added) {let rejected=configured((member,false))}\nreturn value}}",
        "module tuple_loop\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {var alias:i64=0\nwhen {while value>0 {alias=value}\nlet rejected=configured((alias,false))\nreturn value}}",
        "module tuple_global\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {inject global_state\nwhen {let pair:tuple<i64,bool> = get(global_state,\"pair\")\nlet rejected=configured(pair)\nreturn value}}",
        "module tuple_out\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {inject out\nwhen {let rejected=configured((out,false))\nreturn value}}",
        "module tuple_clock_default\nfn unused(value:i64,const pair:tuple<datetime,bool> = (clock.evaluation_time,false))->i64 {inject clock\nwhen{return value}}",
        "module tuple_assignment\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {var alias:i64=0\nwhen {alias=value\nlet rejected=configured((alias,false))\nreturn value}}",
        "module tuple_state\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {state alias:i64=0\nwhen {let rejected=configured((alias,false))\nreturn value}}",
        "module tuple_cache\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {cache alias:i64=0\nwhen {let rejected=configured((alias,false))\nreturn value}}",
        "module tuple_retained_alias\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {let pair=(value,false)\nreturn configured(pair)}",
        "module tuple_nested_list\nfn configured(const pairs:list<tuple<i64,bool>,1>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {return configured([(value,false)])}",
        "module tuple_alias\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {let alias=value\nreturn configured((alias,false))}",
    ] {
        let diagnostics = hgl_program::diagnostics(&[("phase.hgl".into(), source.into())]);
        assert!(!diagnostics.is_empty(), "{source}");
        assert!(
            diagnostics.iter().all(|d| d.issue.code.is_none()),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn tuple_elements_run_once_in_written_order_and_failure_stops_later_elements()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = scratch_dir("hgl-tuple-order")?;
    fs::create_dir_all(dir.join("src"))?;
    let mut runner = include_str!("fixtures/tuple_order_runner.rs").to_owned();
    let mut calls = "fn main(){".to_owned();
    for (entry, trace, after) in [
        ("success", 21, Some(21)),
        ("nested", 345, Some(345)),
        ("failure", 9, None),
    ] {
        let program = hgl_program::compile(
            &[(
                "tuple_order.hgl".into(),
                include_str!("fixtures/tuple_order.hgl").into(),
            )],
            entry,
        )?;
        fs::write(
            dir.join(format!("src/{entry}.rs")),
            hgl_program::emit_rust(&program),
        )?;
        writeln!(
            runner,
            "mod {entry};impl {entry}::Native for Provider {{fn mark_i64(value:i64)->hgl_types::NodeResult<i64> {{mark(value)}}}}"
        )?;
        writeln!(
            calls,
            "{{let mut registry=hgl_describe::Registry::new();{entry}::register(&mut registry).unwrap();let description={entry}::main(&registry).unwrap();run(registry,description,{trace},{after:?});}}"
        )?;
    }
    runner += &calls;
    runner += "}";
    fs::write(dir.join("src/main.rs"), runner)?;
    manifest(&root, &dir)?;
    let profile = if cfg!(debug_assertions) {
        vec![]
    } else {
        vec!["--release"]
    };
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .args(profile)
        .env(
            "CARGO_TARGET_DIR",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"),
        )
        .current_dir(&dir)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn manifest(
    root: &std::path::Path,
    dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs};
    let mut manifest = String::from(
        "[package]\nname=\"tuple_observations-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, dir))?;
    Ok(())
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &std::path::Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}

/// A directory of this test's own under the temp root; see `NEXT_DIR`.
fn scratch_dir(prefix: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let serial = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(std::env::temp_dir().join(format!("{prefix}-{}-{tick}-{serial}", std::process::id())))
}

#[test]
fn unsupported_tuple_descendants_and_native_collection_publication_reject() {
    for source in [
        "module hgraph.std part unsupported_tuple\nfn copied(value:tuple<set<i64>,bool>)->tuple<set<i64>,bool>{when all_valid(value){let saved=value\nreturn saved}}\ntest invalid {eval(copied,[delta<tuple<set<i64>,bool>>(items:[0:delta<set<i64>>(added:[1]),1:true])])}",
        "module hgraph.std part unsupported_tuple\nconst fn same_text(text:str)->bool => text==\"held\"\nfn copied(value:tuple<str,bool>)->bool {when all_valid(value){let saved=value\nreturn same_text(saved[0])}}\ntest invalid {eval(copied,[(\"held\",false)])}",
        "module hgraph.std part unsupported_tuple\nfn copied(value:i64)->tuple<list<i64>,bool>{when {var values:list<i64> = []\npush(values,value)\nreturn (values,false)}}\ntest invalid {eval(copied,[1])}",
    ] {
        let error = compile_tests(&sources(source))
            .expect_err("unsupported physical ownership must reject");
        assert!(
            !error.contains("delta.type_mismatch") && !error.contains("syntax."),
            "{error}"
        );
    }
}

#[test]
fn embedding_an_invalid_current_tuple_fails_before_publication()
-> Result<(), Box<dyn std::error::Error>> {
    run_source(
        "module hgraph.std part tuple_invalid_root\nfn embed(value:tuple<i64,bool>,trigger:i64)->tuple<tuple<i64,bool>,bool>{when modified(trigger)&&valid(trigger){return(value,false)}}\ntest invalid_root {eval(embed,value:[_,(7,false)],trigger:[1,_])}",
        false,
        true,
    )
}

#[test]
fn inner_shadowing_does_not_make_an_outer_constant_tuple_member_runtime() {
    let source = "module tuple_shadow\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64)->i64 {let alias=0\nwhen {if value>0 {let alias=value}\nlet accepted=configured((alias,false))\nreturn value}}";
    let diagnostics = hgl_program::diagnostics(&[("shadow.hgl".into(), source.into())]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
#[test]
fn owning_tuple_observations_require_finite_adapter_preparation() {
    let source = "module tuple_standalone\nfn source()->tuple<i64,bool> {yield 0s:delta<tuple<i64,bool>>(items:[0:7,1:false])}\nfn copied(value:tuple<i64,bool>)->tuple<i64,bool>{when {let saved=value\nreturn saved}}\nfn main()->tuple<i64,bool> => copied(source())";
    let error = hgl_program::compile(&[("standalone.hgl".into(), source.into())], "main")
        .expect_err("standalone snapshot destinations have no preparation");
    assert!(
        error.contains("owning Tuple observations require finite prepared transport"),
        "{error}"
    );
    let source = "module hgraph.std part tuple_unproved\nfn source()->tuple<i64,bool> {var index:i64=0\nwhile index>=0 {yield 0s:delta<tuple<i64,bool>>(items:[0:7,1:false])\nindex+=1}}\nfn copied(value:tuple<i64,bool>)->tuple<i64,bool>{when {let saved=value\nreturn saved}}\nfn entry()->tuple<i64,bool> => copied(source())\ntest unproved {eval(entry)}";
    let error = compile_tests(&sources(source))
        .expect_err("unproved snapshot adapter must reject before emission");
    assert!(
        error.contains("owning Tuple observations require finite prepared transport"),
        "{error}"
    );
}

#[test]
fn temporal_loop_binding_shadows_an_outer_constant_without_changing_it() {
    let source = "module tuple_loop_shadow\nfn configured(const pair:tuple<i64,bool>)->i64 {yield 0s:1}\nfn unused(value:i64,members:set<i64>)->i64 {let alias=0\nwhen {for alias in elements(members,added) {let inner=alias}\nlet accepted=configured((alias,false))\nreturn value}}";
    let diagnostics = hgl_program::diagnostics(&[("shadow.hgl".into(), source.into())]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
