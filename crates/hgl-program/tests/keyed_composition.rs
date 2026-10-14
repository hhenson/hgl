//! Outputless keyed children retain state and isolate explicitly captured failures.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("keyed.hgl".into(), source.into()),
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

const SOURCE: &str = r#"module keyed
native const fn record(value:i64)->i64
native const fn record(value:i64)->i64 {}
const fn checked(value:i64)->result<i64,error> {
    if value<0 {return err(error(code:"value.missing_key",message:"negative price"))}
    return ok(value)
}
fn required(value:i64)->i64 {when {return checked(value)?}}
fn consume_value(value:i64) {when {record(value)}}
const fn expected_failure(failure:error,now:datetime,event:bool)->bool {
    let origin=failure.context
    let code=failure.code
    if origin!=null {
        if code!=null {
            return code=="value.missing_key"&&failure.message=="negative price"&&origin.phase=="evaluate"&&origin.node!=""&&(!event||origin.evaluation_time==now)
        }
    }
    return false
}
fn presence(failure:error,step:i64) {
    inject clock
    when modified(step)&&valid(step) {
        if valid(failure)&&!expected_failure(failure,clock.evaluation_time,modified(failure)) {record(-999)}
        if valid(failure) {record(step*100+1)} else {record(step*100)}
    }
}
fn per_key(book:map<str,i64>,step:i64) {
    for key,value in items(book) {
        let seen=capture(required,value)
        consume_value(seen.value)
        presence(seen.error,step)
    }
}
fn uncaught(book:map<str,i64>) {
    for value in values(book) {consume_value(required(value))}
}
test keyed_capture_lifetimes {
    eval(per_key,book:[
        delta<map<str,i64>>(upsert:["A":-1,"B":7]),
        delta<map<str,i64>>(upsert:["A":8,"B":9]),
        delta<map<str,i64>>(remove:["A"]),
        delta<map<str,i64>>(upsert:["A":10]),
        delta<map<str,i64>>(upsert:["A":-1])
    ],step:[0,1,2,3,4])
}
test uncaught_keyed_failure {
    assert raises("value.missing_key") {
        eval(uncaught,book:[delta<map<str,i64>>(upsert:["A":-1,"B":7])])
    }
}
"#;

#[test]
fn keyed_capture_sibling_continuation_and_fresh_error_lifetimes_execute()
-> Result<(), Box<dyn std::error::Error>> {
    run(
        SOURCE,
        "keyed-composition",
        &[
            "keyed::keyed_capture_lifetimes",
            "keyed::uncaught_keyed_failure",
        ],
        "[0,1,100,101,200,300,300,400,401,7,8,9,10]",
    )
}

#[test]
fn whole_graph_capture_keeps_keyed_members_runnable_in_later_cycles()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE
        .split("test keyed_capture_lifetimes")
        .next()
        .ok_or("fixture declarations")?
        .to_string()
        + r#"
fn whole(book:map<str,i64>,step:i64) {
    let seen=capture(uncaught,book)
    presence(seen.error,step)
}
test whole_keyed_graph {
    eval(whole,book:[
        delta<map<str,i64>>(upsert:["A":-1,"B":7]),
        delta<map<str,i64>>(upsert:["A":8,"B":9]),
        delta<map<str,i64>>(upsert:["A":10,"B":11])
    ],step:[0,1,2])
}
"#;
    run(
        &source,
        "whole-keyed-composition",
        &["keyed::whole_keyed_graph"],
        "[1,101,201,8,9,10,11]",
    )
}

#[test]
fn capture_returning_ref_selects_prepared_bundle_without_copying_normal_values()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE
        .split("test keyed_capture_lifetimes")
        .next()
        .ok_or("fixture declarations")?
        .to_string()
        + r#"
fn route(value:ref<i64>)->ref<i64> {when {return value}}
fn selected(value:i64,other:i64,selector:bool)->i64 {
    var choice:ref<i64>
    if selector {choice=route(required(value)+0)} else {choice=route(other+0)}
    choice
}
fn error_ticks(failure:error,step:i64) {
    inject clock
    when modified(step)&&valid(step) {
        if valid(failure)&&!expected_failure(failure,clock.evaluation_time,modified(failure)) {record(-999)}
        if valid(failure) {
            if modified(failure) {record(step*100+51)} else {record(step*100+50)}
        } else {record(step*100+40)}
    }
}
fn observe_selection(book:map<str,i64>,other:i64,selector:bool,step:i64) {
    for value in values(book) {
        let seen=capture(selected,value,other,selector)
        consume_value(seen.value)
        error_ticks(seen.error,step)
    }
}
test captured_ref_retargets {
    eval(observe_selection,
        book:[delta<map<str,i64>>(upsert:["A":-1]),delta<map<str,i64>>(upsert:["A":8]),delta<map<str,i64>>(upsert:["A":9])],
        other:[10,20,30],selector:[true,false,true],step:[0,1,2])
}
"#;
    run(
        &source,
        "captured-keyed-ref",
        &["keyed::captured_ref_retargets"],
        "[51,150,250,20,9]",
    )
}

#[test]
fn capture_forwarding_member_retains_actual_external_normal_endpoint()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE
        .split("test keyed_capture_lifetimes")
        .next()
        .ok_or("fixture declarations")?
        .to_string()
        + r#"
fn identity(value:i64)->i64 => value
fn observed_identity(book:map<str,i64>) {
    for value in values(book) {
        let seen=capture(identity,value)
        consume_value(seen.value)
    }
}
test captured_identity {
    eval(observed_identity,book:[delta<map<str,i64>>(upsert:["A":7]),delta<map<str,i64>>(upsert:["A":8])])
}
"#;
    run(
        &source,
        "captured-keyed-identity",
        &["keyed::captured_identity"],
        "[7,8]",
    )
}

#[test]
fn capture_no_input_generator_preserves_zero_and_future_yields()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE
        .split("test keyed_capture_lifetimes")
        .next()
        .ok_or("fixture declarations")?
        .to_string()
        + r"
fn autonomous()->i64 {
    yield 0us:7
    yield 2us:8
}
fn observe_autonomous() {
    let seen=capture(autonomous)
    consume_value(seen.value)
}
test captured_autonomous {
    eval(observe_autonomous)
}
";
    run_graph(
        &source,
        "captured-autonomous-generator",
        &["keyed::captured_autonomous"],
        "[7,8]",
        0,
    )
}

#[test]
fn capture_whole_generator_graph_preserves_future_yield_after_consumer_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE
        .split("test keyed_capture_lifetimes")
        .next()
        .ok_or("fixture declarations")?
        .to_string()
        + r"
fn autonomous_prices()->i64 {
    yield 0us:7
    yield 1us:-1
    yield 1us:8
}
fn autonomous_pipeline()->i64 => required(autonomous_prices())
fn observe_autonomous_failure(failure:error) {
    inject clock
    when modified(failure)&&valid(failure) {
        if !expected_failure(failure,clock.evaluation_time,true) {record(-999)}
        record(51)
    }
}
fn observe_autonomous_value(value:i64,failure:error) {
    inject clock
    when modified(value)&&valid(value) {
        record(value)
        if valid(failure) {
            if !expected_failure(failure,clock.evaluation_time,false)||modified(failure) {record(-999)}
            let origin=failure.context
            if origin!=null {
                if origin.evaluation_time>=clock.evaluation_time {record(-999)}
            } else {record(-999)}
            record(250)
        } else {record(40)}
    }
}
fn observe_autonomous_pipeline() {
    let seen=capture(autonomous_pipeline)
    observe_autonomous_failure(seen.error)
    observe_autonomous_value(seen.value,seen.error)
}
test captured_autonomous_pipeline {
    eval(observe_autonomous_pipeline)
}
";
    run_graph(
        &source,
        "captured-autonomous-pipeline",
        &["keyed::captured_autonomous_pipeline"],
        "[7,8,40,51,250]",
        0,
    )
}

fn run(
    source: &str,
    name: &str,
    tests: &[&str],
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    run_graph(source, name, tests, expected, 1)
}

fn run_graph(
    source: &str,
    name: &str,
    tests: &[&str],
    expected: &str,
    keyed: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let suite = hgl_program::compile_module_suite(&sources(source))?;
    assert_eq!(
        suite
            .tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>(),
        tests
    );
    assert_eq!(suite.plans.len(), tests.len());
    assert!(suite.plans.iter().all(|plan| plan.keyed.len() == keyed));
    if keyed == 0 {
        for plan in &suite.plans {
            assert_eq!(
                plan.input_length, 0,
                "autonomous controls must have no driving sequence"
            );
            assert_eq!(
                plan.nodes
                    .iter()
                    .filter(|node| node.generator.is_some())
                    .count(),
                1
            );
            assert_eq!(plan.captures.len(), 1);
            assert!(plan.nodes[plan.captures[0].owner].inputs.is_empty());
        }
    }
    let project = regression_cache::open(&root, name)?;
    let directory = &project.path;
    let code = hgl_program::emit_tests(&suite).replace("fn main()", "fn generated_main()");
    generated_cache::write(
        directory.join("src/main.rs"),
        code + &RUNTIME
            .replace("EXPECTED", expected)
            .replace("ALLOW_SIBLING", if keyed == 0 { "false" } else { "true" }),
    )?;
    let mut manifest = format!(
        "[package]\nname={name:?}\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n"
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
    ] {
        let path = root.join("crates").join(name);
        writeln!(manifest, "{name}={{path={:?}}}", path.to_string_lossy())?;
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
    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
        "{} executed tests, {} evaluations, 0 failures",
        tests.len(),
        tests.len()
    )));
    Ok(())
}

const RUNTIME: &str = r#"
struct Provider;
static RECORDS:std::sync::Mutex<Vec<i64>>=std::sync::Mutex::new(Vec::new());
mod native {
 pub fn record_i64(value:i64)->i64 {assert_ne!(value,-999,"capture failed with incorrect code, message or evaluate provenance");super::RECORDS.lock().unwrap().push(value);0}
}
fn main() {
 generated_main();
 let mut records=RECORDS.lock().unwrap();
 records.sort_unstable();
 for expected in EXPECTED {
  let at=records.iter().position(|&value|value==expected).unwrap_or_else(||panic!("missing observation {expected}, remaining records {records:?}"));records.remove(at);
 }
 // The unhandled-error control may run its successful sibling before failure.
 assert!(records.is_empty()||(ALLOW_SIBLING&&records.as_slice()==[7]),"unexpected keyed observations: {records:?}");
}
"#;

#[test]
fn dependent_bodies_and_deferred_scalar_binding_are_diagnosed() {
    for (body, diagnostic) in [
        ("result=value", "cannot assign enclosing bindings"),
        (
            "consume_value(key)",
            "key expression binding is unsupported",
        ),
        (
            "consume_value(offset)",
            "scalar captures require an agreed binding contract",
        ),
        ("return value", "loop results and exits are unsupported"),
    ] {
        let source = "module keyed\nfn consume_value(value:i64) {when {}}\n".to_string()
            + &format!(
                "\nfn target(book:map<str,i64>,const offset:i64) {{var result=offset\nfor key,value in items(book) {{{body}}}}}\nfn source()->map<str,i64> {{yield 0us:delta<map<str,i64>>(upsert:[\"A\":1])}}\nfn main() => target(source(),1)\n"
            );
        let error = hgl_program::compile(&sources(&source), "main").unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
}

#[test]
fn member_and_shared_temporal_captures_keep_original_endpoint_types() -> Result<(), String> {
    let source = r#"module keyed_shapes
fn consume(value:i64,offset:i64) {when {}}
fn observe(book:map<str,i64>,offset:i64) {
    for value in values(book) {
        let adjusted=value+offset
        consume(adjusted,offset)
    }
}
test captures {
    eval(observe,book:[delta<map<str,i64>>(upsert:["A":1])],offset:[10])
}
"#;
    let suite = hgl_program::compile_module_suite(&sources(source))?;
    assert_eq!(suite.tests.len(), 1);
    assert_eq!(suite.plans.len(), 1);
    let plan = &suite.plans[0];
    assert_eq!(plan.keyed.len(), 1);
    let keyed = &plan.keyed[0];
    assert_eq!(plan.nodes[keyed.member].result, hgl_source::Ty::I64);
    let owner = &plan.nodes[keyed.owner];
    assert_eq!(owner.result, hgl_source::Ty::Void);
    assert_eq!(owner.inputs.len(), 2);
    assert!(matches!(owner.inputs[0].2, hgl_source::Ty::Map(..)));
    assert_eq!(owner.inputs[1].2, hgl_source::Ty::I64);
    let offset = owner.inputs[1].1;
    assert!(!keyed.nodes.contains(&offset));
    assert!(keyed.nodes.iter().any(|&id| {
        plan.nodes[id]
            .inputs
            .iter()
            .any(|(_, source, _)| *source == keyed.member)
    }));
    assert!(keyed.nodes.iter().any(|&id| {
        plan.nodes[id]
            .inputs
            .iter()
            .any(|(_, source, _)| *source == offset)
    }));
    Ok(())
}
