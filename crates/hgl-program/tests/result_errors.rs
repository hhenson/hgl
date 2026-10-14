//! Nullable map reads keep absence separate from values and coded failures.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn specified_result_examples_execute() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_spec/language/examples/result-errors.hgl"),
    )?;
    execute(&source, RUNTIME, "result-errors")
}
#[test]
fn prepared_result_and_error_data_allocate_nothing() -> Result<(), Box<dyn std::error::Error>> {
    execute(PREPARED, STRICT_RUNTIME, "prepared-result-data")
}

#[test]
fn specified_result_rejections_keep_their_codes() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_spec/language/examples/reject/result-errors.hgl"),
    )?;
    let library = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    let controls = [
        (
            "unchecked",
            "let value:result<i64,error> =ok(1)\nunchecked(value)",
            "result.required_variant",
        ),
        (
            "wrong_tag",
            "let value:result<i64,error> =ok(1)\nwrong_tag(value)",
            "result.required_variant",
        ),
        (
            "incompatible",
            "let value:result<i64,error> =ok(1)\nincompatible(value)",
            "result.propagation_type",
        ),
        (
            "wrong_handler_error",
            "let value:result<i64,str> =ok(1)\neval(wrong_handler_error,value:[value])",
            "result.propagation_type",
        ),
        (
            "nullable_operand",
            "let book=map<str,i64>(items:[\"X\":1])\nnullable_operand(book)",
            "result.propagation_type",
        ),
        (
            "no_expected_type",
            "no_expected_type()",
            "result.context_type",
        ),
        (
            "graph_propagation",
            "let value:result<i64,error> =ok(1)\neval(graph_propagation,value:[value])",
            "result.propagation_phase",
        ),
    ];
    for (name, invocation, code) in controls {
        let marker = format!("fn {name}(");
        let header = source
            .find(&marker)
            .ok_or("missing rejection declaration")?;
        let begin = source[..header].rfind('\n').map_or(0, |i| i + 1);
        let remaining = &source[header..];
        let end = remaining
            .find("\nconst fn ")
            .into_iter()
            .chain(remaining.find("\nfn "))
            .chain(remaining.find("\ntest "))
            .min()
            .map_or(source.len(), |i| header + i);
        let declaration = &source[begin..end];
        let checked =
            format!("module rejects\n{declaration}\ntest instantiate {{ {invocation} }}\n");
        let mut sources = library.clone();
        sources.insert(0, ("reject.hgl".into(), checked));
        let diagnostics = hgl_program::module_diagnostics(&sources);
        assert_eq!(diagnostics.len(), 1, "{name}: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].issue.code,
            Some(code),
            "{name}: {diagnostics:?}"
        );
    }
    Ok(())
}

#[test]
fn image_backed_native_result_data_keeps_the_private_operational_channel()
-> Result<(), Box<dyn std::error::Error>> {
    execute(NATIVE_DATA, RUNTIME, "native-result-image-data")
}

#[test]
fn specified_capture_examples_execute() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_spec/language/examples/error-capture.hgl"),
    )?;
    execute(&source, CAPTURE_RUNTIME, "capture-errors")
}

#[test]
fn native_result_data_is_distinct_from_private_operational_failure()
-> Result<(), Box<dyn std::error::Error>> {
    execute(NATIVE_DATA, RUNTIME, "native-result-data")
}
const NATIVE_DATA: &str = r#"module native_result
native const fn checked(value:i64)->result<i64,error>
native const fn checked(value:i64)->result<i64,error> {}
native const fn mixed(value:i64)->result<i64,error>
native const fn mixed(value:i64)->result<i64,error> { rust_abi_node_result }
fn data(value:i64)->result<i64,error>{when {return checked(value)}}
fn mixed_data(value:i64)->result<i64,error>{when {return mixed(value)}}
fn propagated(value:i64)->i64 {when {return checked(value)?}}
test native_outcomes {
 let success:result<i64,error> =ok(7)
 let failure:result<i64,error> =err(error(code:"value.missing_key",message:"native result data"))
 assert eval(data,value:[7,-1])==[success,failure]
 assert eval(mixed_data,value:[7,-1])==[success,failure]
 assert raises("value.missing_key") {eval(propagated,value:[-1])}
 assert raises("value.capability") {eval(mixed_data,value:[-2])}
}
"#;

#[test]
fn capture_preserves_helper_native_and_operator_selection() -> Result<(), Box<dyn std::error::Error>>
{
    execute(LIFTED_CAPTURE, STRICT_RUNTIME, "capture-call-selection")
}
const LIFTED_CAPTURE: &str = r"module lifted_capture
use hgraph.native as arithmetic
use hgraph.operators as operators
const fn plus(lhs:i64,rhs:i64)->i64=>lhs+rhs
fn helper(lhs:i64,rhs:i64)->i64 {let seen=capture(plus,lhs,rhs)
seen.value}
fn native_lift(lhs:i64,rhs:i64)->i64 {let seen=capture(arithmetic::add,lhs,rhs)
seen.value}
fn operator_lift(lhs:i64,rhs:i64)->i64 {let seen=capture(operators::add_,lhs,rhs)
seen.value}
test calls {
 assert eval(helper,lhs:[1,2],rhs:[2,3])==[3,5]
 assert eval(native_lift,lhs:[1,2],rhs:[2,3])==[3,5]
 assert eval(operator_lift,lhs:[1,2],rhs:[2,3])==[3,5]
}
";
#[test]
fn source_declared_ref_capture_reports_the_target_limit_before_shared_emission()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    sources.insert(0,("ref_capture.hgl".into(),"module ref_capture\nfn route(value:ref<i64>)->ref<i64>{when {return value}}\nfn observed(value:i64) {let seen=capture(route,value)}\ntest attempt {eval(observed,value:[1])}\n".into()));
    let error =
        hgl_program::compile_tests(&sources).expect_err("declared REF target proof is excluded");
    assert!(
        error
            .contains("Rust capture target cannot prepare a source-declared REF normal connection"),
        "{error}"
    );
    Ok(())
}
#[test]
fn specified_capture_rejections_keep_their_codes() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_spec/language/examples/reject/error-capture.hgl"),
    )?;
    let library = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    let helpers = "fn identity(value:i64)->i64{when {return value}}\nfn sink(value:i64){when {}}\nconst fn scalar_only(value:i64)->i64=>value+1\n";
    for (name, invocation, code) in [
        ("value_capture", "value_capture(1)", "capture.phase"),
        (
            "handler_capture",
            "eval(handler_capture,value:[1])",
            "capture.phase",
        ),
        (
            "outputless_value",
            "eval(outputless_value,value:[1])",
            "capture.output_field",
        ),
        ("scalar_target", "scalar_target()", "capture.phase"),
    ] {
        let marker = format!("fn {name}(");
        let header = source.find(&marker).ok_or("rejection declaration")?;
        let begin = source[..header].rfind('\n').map_or(0, |i| i + 1);
        let rest = &source[header..];
        let end = rest
            .find("\nconst fn ")
            .into_iter()
            .chain(rest.find("\nfn "))
            .chain(rest.find("\ntest "))
            .min()
            .map_or(source.len(), |i| header + i);
        let checked = format!(
            "module reject_capture\n{helpers}{}\ntest attempt {{{invocation}}}\n",
            &source[begin..end]
        );
        let mut sources = library.clone();
        sources.insert(0, ("reject.hgl".into(), checked));
        let diagnostics = hgl_program::module_diagnostics(&sources);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.issue.code == Some(code)),
            "{name}: {diagnostics:?}"
        );
        assert!(
            hgl_program::compile_tests(&sources).is_err(),
            "admitted {name}"
        );
    }
    Ok(())
}

#[test]
fn unused_capture_children_retire_after_an_earlier_escaping_failure()
-> Result<(), Box<dyn std::error::Error>> {
    execute(UNUSED_CAPTURE, UNUSED_RUNTIME, "capture-unused-child")?;
    let source=UNUSED_CAPTURE.replace("fn fail(value:i64)->i64 {when {return at(map<str,i64>(items:[]),\"missing\")}}","fn fail(value:i64)->i64 {start {at(map<str,i64>(items:[]),\"missing\")}when {return value}}");
    execute(&source, UNUSED_RUNTIME, "capture-unused-start-child")
}
const UNUSED_CAPTURE: &str = r#"module unused_capture
native const fn probe(value:i64)
native const fn probe(value:i64) {}
fn observed(value:i64)->i64 {start {probe(10)}stop {probe(11)}when {return value}}
fn fail(value:i64)->i64 {when {return at(map<str,i64>(items:[]),"missing")}}
fn sink(value:i64){when {}}
fn root(value:i64){sink(fail(value))
let seen=capture(observed,value)
sink(seen.value)}
test never_activated {assert raises("value.missing_key") {eval(root,value:[1])}}
"#;
const UNUSED_RUNTIME: &str = r#"
static PROBES:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
 let start=graph.start(store,config.start_time);
 let outcome=match start{Ok(())=>graph.evaluate(store,config.start_time),Err(error)=>Err(error)};assert!(outcome.is_err());
 let scopes=store.bindings().storage_counts()[2];assert!(scopes>1);
 let cleanup=graph.stop(store,config.start_time);
 for index in 1..scopes{assert_eq!(store.bindings().child_owner(hgl_store::ScopeId{index,generation:0}),None,"unused prepared scope survived cleanup");}
 assert_eq!(PROBES.load(std::sync::atomic::Ordering::Relaxed),0,"unstarted capture child fired lifecycle hooks");
 hgl_types::finish(outcome,cleanup).map_err(hgl_kernel::EngineError::Node)?;Ok(1)
}
"#;

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
    if name == "native-result-image-data" {
        sources.push(native_result_image(&root)?);
    }
    sources.insert(0, ("any_lookup.hgl".into(), source.into()));
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), 1, "{name}: exact root test inventory");
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    if runtime == CAPTURE_RUNTIME {
        code = code.replace(
            "child.evaluate_captured(ctx)",
            "crate::measured_capture(child,ctx)",
        );
    }
    code.push_str(NATIVE_PROVIDER);
    if runtime == UNUSED_RUNTIME {
        code.push_str("mod unused_probe {pub fn probe_i64(_:i64){crate::PROBES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}}\n");
        code = code.replace(
            "mod native {",
            "mod native {pub use crate::unused_probe::*;",
        );
    }
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
    verify_counts(name, &output.stdout);
    report_allocations(&root, name, &output.stdout)?;
    Ok(())
}

const NATIVE_PROVIDER: &str = r#"
struct Provider;
mod native {
 pub use hgl_stdlib::std_native::*;
 pub fn checked_i64(value:i64)->Result<i64,hgl_types::ErrorValue>{if value<0{Err(hgl_types::ErrorValue{message:"native result data".into(),code:Some("value.missing_key".into()),context:None})}else{Ok(value)}}
 pub fn mixed_i64(value:i64)->hgl_types::NodeResult<Result<i64,hgl_types::ErrorValue>>{if value==-2{Err(hgl_types::NodeError::coded("native operational failure","value.capability"))}else{Ok(checked_i64(value))}}
}
"#;

fn verify_counts(name: &str, bytes: &[u8]) {
    let evaluations = match name {
        "result-errors" | "capture-errors" => 16,
        "native-result-data" | "native-result-image-data" => 4,
        "prepared-result-data" | "capture-call-selection" => 3,
        "capture-unused-child" | "capture-unused-start-child" => 1,
        _ => unreachable!("registered source inventory for {name}"),
    };
    let output = String::from_utf8_lossy(bytes);
    assert!(
        output.contains(&format!(
            "1 executed tests, {evaluations} evaluations, 0 failures"
        )),
        "{name}: {output}"
    );
}

fn native_result_image(
    root: &std::path::Path,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let project = regression_cache::open(root, "native-result-provider")?;
    let provider = project.path.join("provider.rs");
    generated_cache::write(
        &provider,
        r#"
pub const PROVIDER:&str="hgl.native_result_data";pub const FINGERPRINT:&str="result-data-v1";
pub fn checked_i64(value:i64)->Result<i64,hgl_types::ErrorValue>{if value<0{Err(hgl_types::ErrorValue{message:"native result data".into(),code:Some("value.missing_key".into()),context:None})}else{Ok(value)}}
pub fn mixed_i64(value:i64)->hgl_types::NodeResult<Result<i64,hgl_types::ErrorValue>>{if value==-2{Err(hgl_types::NodeError::coded("native operational failure","value.capability"))}else{Ok(checked_i64(value))}}
"#,
    )?;
    let descriptor = format!(
        "native_descriptor 1 target \"hgl-rust-1\"\nmodule native_result\nprovider \"hgl.native_result_data\" fingerprint \"result-data-v1\" image {:?}\nvalue checked(value:i64)->result<i64,error> symbol \"checked_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\nvalue mixed(value:i64)->result<i64,error> throws symbol \"mixed_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\n",
        provider.to_string_lossy()
    );
    Ok((
        project
            .path
            .join("result.hgl-native")
            .to_string_lossy()
            .into_owned(),
        descriptor,
    ))
}

fn report_allocations(
    root: &std::path::Path,
    name: &str,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if name == "capture-errors" {
        let report = String::from_utf8_lossy(bytes);
        assert!(report.contains("capture ticks:"), "{report}");
        std::fs::write(
            root.join("target/capture-allocation-report.txt"),
            report.as_bytes(),
        )?;
    }
    if name.starts_with("result") {
        let report = String::from_utf8_lossy(bytes);
        assert!(report.contains("result ticks:"), "{report}");
        std::fs::write(
            root.join("target/result-data-allocation-report.txt"),
            report.as_bytes(),
        )?;
    }
    Ok(())
}

const RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let mut cycles=0;let mut now=config.start_time;let mut successful=0;let mut failed=0;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){successful+=count;}else{failed+=count;}outcome=result;cycles+=1;
    }
    let stop=graph.stop(store,now);outcome=hgl_types::finish(outcome,stop);
    println!("result ticks: {cycles} cycles, {successful} successful-path allocations, {failed} failure-path allocations");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;

const CAPTURE_RUNTIME: &str = r#"
#[global_allocator]static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
static CAPTURED:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
fn measured_capture<T>(child:&mut hgl_kernel::nested::PreparedChild<T>,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult<Option<hgl_types::ErrorValue>>{
 let result=child.evaluate_captured(ctx);if matches!(result,Ok(Some(_))){CAPTURED.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}result
}
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
 let(start,start_allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));start.map_err(hgl_kernel::EngineError::Node)?;
 let mut cycles=0;let mut now=config.start_time;let mut ordinary=0;let mut captured=0;let mut escaping=0;let mut outcome=Ok(());
 while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
 let before=CAPTURED.load(std::sync::atomic::Ordering::Relaxed);let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
 if result.is_err(){escaping+=count;}else if CAPTURED.load(std::sync::atomic::Ordering::Relaxed)>before{captured+=count;}else{ordinary+=count;}
 outcome=result;cycles+=1;
 }
 let(stop,stop_allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));outcome=hgl_types::finish(outcome,stop);
 println!("capture ticks: {cycles} cycles, startup {start_allocations}, successful ticks without captured failure {ordinary}, ticks with captured failure {captured}, escaping failure {escaping}, cleanup {stop_allocations}");
 outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;

const PREPARED: &str = r#"module prepared_results
fn selected(step:i64)->result<i64,error>{when {if step==0 {return ok(0)}return err(error(message:"failure",code:"domain.code"))}}
fn errors(step:i64)->error {when {return error(message:"local",code:null)}}
fn relay(value:result<i64,error>)->result<i64,error> {when {let outcome=value
if is_ok(outcome){return ok(outcome.value)}
return err(outcome.error)}}
test data {
let success:result<i64,error> =ok(0)
let failure:result<i64,error> =err(error(message:"failure",code:"domain.code"))
assert eval(selected,step:[0,1,1,0])==[success,failure,failure,success]
assert eval(errors,step:[0,1])==[error(message:"local"),error(message:"local")]
assert eval(relay,value:[success,failure,failure,success])==[success,failure,failure,success]
}
"#;
const STRICT_RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
let(start,count)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));start.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(count,0);
let mut cycles=0;let mut now=config.start_time;while !graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));result.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(count,0,"successful Result/Error data allocated");cycles+=1;}
let(stop,count)=hgl_alloc_count::count_in(||graph.stop(store,now));stop.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(count,0);Ok(cycles)
}
"#;
