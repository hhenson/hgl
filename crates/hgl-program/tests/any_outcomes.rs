//! Every admitted outcome box retains exact selected data and diagnostic ownership.
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

#[test]
fn result_and_error_boxes_execute_through_ordinary_and_temporal_owners()
-> Result<(), Box<dyn std::error::Error>> {
    execute(SOURCE, "any-outcomes", STRICT_RUNTIME, "", (1, 21))
}
#[test]
fn boxed_outcome_codecs_preserve_context_and_reject_corrupt_cold_data()
-> Result<(), Box<dyn std::error::Error>> {
    execute(
        SOURCE,
        "any-outcome-codecs",
        STRICT_RUNTIME,
        &codec_controls(),
        (1, 21),
    )
}
#[test]
fn captured_diagnostics_can_be_boxed_without_losing_the_original_context()
-> Result<(), Box<dyn std::error::Error>> {
    execute(CAPTURE, "any-captured-outcome", GENERIC_RUNTIME, "", (1, 1))
}
const CAPTURE: &str = r#"module captured_any
fn required(value:i64)->i64 {when {
if value<0 {let failure:result<i64,error> = err(error(message:"bad",code:"value.missing_key"))
return failure?}
return value}}
fn observe_error(value:error)->any {inject clock
when {
let context=value.context
if context!=null {if context.phase=="evaluate" && context.evaluation_time==clock.evaluation_time {return any(delta_value(value))}}
return any("missing or wrong context")}}
fn observed(value:i64)->any {let seen=capture(required,value)
observe_error(seen.error)}
test retain_context {
let expected=any(error(message:"bad",code:"value.missing_key"))
assert eval(observed,value:[-1,1,-2])==[expected,_,expected]
}
"#;

#[test]
fn exact_shared_boxed_outcome_cases_execute() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_std/hgl/hgraph/tests/result_errors.hgl"),
    )?;
    execute(&source, "shared-any-outcomes", "", "", (6, 58))
}

fn codec_controls() -> String {
    use hgl_source::Ty;
    let result = Ty::Result(Box::new(Ty::I64), Box::new(Ty::Error));
    let nested = Ty::Result(Box::new(result.clone()), Box::new(Ty::Str));
    let boxed = Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any));
    let box_any = hgl_rust::any_values::construct(&boxed, "value");
    let boxed_name = layout_name(&boxed);
    let box_error = hgl_rust::any_values::construct(&Ty::Error, "value");
    let box_result = hgl_rust::any_values::construct(&result, "value");
    let box_nested = hgl_rust::any_values::construct(&nested, "value");
    let error_name = layout_name(&Ty::Error);
    let result_name = layout_name(&result);
    let nested_name = layout_name(&nested);
    let support = hgl_rust::cold_transport::support();
    let pack = hgl_rust::cold_transport::encode(&Ty::Any, "value");
    let unpack = hgl_rust::cold_transport::decode(&Ty::Any, "bytes", "registry");
    format!(
        r#"
mod probe {{use super::*;{support}
fn box_error(value:hgl_types::ErrorValue)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({box_error})}}
fn box_result(value:Result<i64,hgl_types::ErrorValue>)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({box_result})}}
fn box_nested(value:Result<Result<i64,hgl_types::ErrorValue>,String>)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({box_nested})}}
fn box_any(value:Result<hgl_types::AnyValue,hgl_types::AnyValue>)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({box_any})}}
fn pack(value:hgl_types::AnyValue,registry:&hgl_describe::Registry)->hgl_types::NodeResult<Vec<u8>>{{Ok({pack})}}
fn unpack(bytes:&[u8],registry:&hgl_describe::Registry)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({unpack})}}
fn frame(value:&[u8])->Vec<u8>{{let mut bytes=(value.len() as u64).to_le_bytes().to_vec();bytes.extend(value);bytes}}
fn check_bad(plan:&'static hgl_types::AnyPlan,bytes:Vec<u8>){{let data=hgl_types::AnyValue::from_encoded(plan,bytes);let validate=plan.validate.expect("cold validator");assert!(validate(data.data()).is_err());}}
pub fn run()->hgl_types::NodeResult{{
let registry=hgl_describe::Registry::new();
let local=hgl_types::ErrorValue{{message:"bad".into(),code:Some("value.missing_key".into()),context:None}};
let original=hgl_types::ErrorContext{{node:"original.instance".into(),phase:"evaluate".into(),evaluation_time:hgl_types::EngineTime::from_micros(42)}};
let mut diagnostic=local.clone();diagnostic.context=Some(original.clone());
let source=box_error(diagnostic.clone())?;diagnostic.message="changed".into();diagnostic.context.as_mut().unwrap().node="changed.instance".into();
let saved=source.retain()?;drop(source);let decoded=any_read_{error_name}(saved.data())?;assert_eq!(decoded.message,"bad");assert_eq!(decoded.context,Some(original.clone()));
let same=box_error(local.clone())?;assert!(saved.equal(&same)?);assert_eq!(saved.compare(&same).unwrap_err().execution_code(),Some("value.capability"));assert_eq!(saved.key_hash().unwrap_err().execution_code(),Some("value.capability"));
let mut changed=local.clone();changed.code=None;assert!(!saved.equal(&box_error(changed)?)?);
let mut destination=hgl_types::AnyValue::default();destination.reserve(saved.data().len())?;
let(result,allocations)=hgl_alloc_count::count_in(||->hgl_types::NodeResult{{for _ in 0..32{{assert!(saved.equal(&same)?);destination.copy_prepared(&saved);}}Ok(())}});result?;assert_eq!(allocations,0,"borrowed diagnostic equality and reserved owning copy allocated");
assert_eq!(any_read_{error_name}(destination.data())?.context,Some(original.clone()));
let mut full=local.clone();full.context=Some(original.clone());
for value in [Ok(7),Err(local.clone()),Err(full)]{{let value=box_result(value)?;let retained=value.retain()?;drop(value);let decoded=any_read_{result_name}(retained.data())?;let expected_context=match &decoded{{Err(error)=>error.context.clone(),Ok(_)=>None}};
let nested=box_nested(Ok(decoded))?;let bytes=pack(nested.retain()?,&registry)?;
for registry in [&registry,&registry.clone()]{{let transported=unpack(&bytes,registry)?;assert!(transported.equal(&nested)?);let decoded=any_read_{nested_name}(transported.data())?.unwrap();assert_eq!(match decoded{{Err(error)=>error.context,Ok(_)=>None}},expected_context);}}
let captured=retained.capture::<hgl_semantics::ir::Value>()?;let rebuilt=decode_any(&hgl_semantics::any_values::retain(captured)).map_err(hgl_types::NodeError::new)?;assert!(rebuilt.equal(&retained)?);
assert_eq!(any_read_{result_name}(rebuilt.data())?.err().and_then(|error|error.context),expected_context);
}}
for value in [Err("nested error".to_owned()),Ok(Ok(7))]{{let value=box_nested(value)?;let bytes=pack(value.retain()?,&registry)?;assert!(unpack(&bytes,&registry)?.equal(&value)?);}}
for value in [Ok(saved.retain()?),Err(saved.retain()?)]{{let boxed=box_any(value)?;let bytes=pack(boxed.retain()?,&registry)?;let transported=unpack(&bytes,&registry)?;assert!(transported.equal(&boxed)?);let selected=any_read_{boxed_name}(transported.data())?;let child=match selected{{Ok(child)|Err(child)=>child}};assert_eq!(any_read_{error_name}(child.data())?.context,Some(original.clone()));}}
let good=box_result(Ok(7))?;let plan=good.plan().unwrap();let encoded=good.data().to_vec();
for bytes in [Vec::new(),vec![2],vec![0],vec![0,0,0],{{let mut bytes=encoded.clone();bytes[0]=2;bytes}},{{let mut bytes=encoded.clone();bytes.push(0);bytes}},{{let mut bytes=encoded.clone();bytes[1..9].copy_from_slice(&7u64.to_le_bytes());bytes}}]{{check_bad(plan,bytes);}}
let error_plan=saved.plan().unwrap();for bytes in [Vec::new(),saved.data()[..saved.data().len()-1].to_vec(),[frame(b"bad"),frame(&[2]),frame(&[0])].concat(),[frame(&[255]),frame(&[0]),frame(&[0])].concat(),[frame(b"bad"),frame(&[0]),frame(&[1])].concat()]{{check_bad(error_plan,bytes);}}
Ok(())
}}
}}
fn main(){{authored_main();probe::run().unwrap();}}
"#
    )
}

const SOURCE: &str = r#"module any_outcomes
struct Holder {outcome:result<i64,error>
cause:error
boxed:any}
const fn box_result(value:result<i64,error>)->any => any(value)
fn relay(value:any)->any {when {return value}}
fn equal(left:any,right:any)->bool {when {return left==right}}
fn boxed_text(value:result<str,str>)->any {when {return any(delta_value(value))}}
fn boxed_any(value:result<any,any>)->any {when {return any(delta_value(value))}}
fn boxed(value:result<i64,error>)->any {when {return any(delta_value(value))}}
fn boxed_error(value:error)->any {when {return any(delta_value(value))}}
fn boxed_holder(value:atomic<Holder>)->any {when {return any(delta_value(value))}}
fn produce(const success:result<i64,error>,const failure:result<i64,error>)->any {
yield 0us:any(success)
yield 1us:any(failure)
yield 1us:any(error(message:"bad",code:null))
}
test boxes {
let cause=error(message:"bad",code:"value.missing_key")
let success:result<i64,error> = ok(7)
let failure:result<i64,error> = err(cause)
assert any(cause)==any(error(message:"bad",code:"value.missing_key"))
assert any(cause)!=any(error(message:"other",code:"value.missing_key"))
assert any(cause)!=any(error(message:"bad",code:null))
assert any(success)!=any(failure)
assert any(success)==box_result(success)
assert any(failure)==box_result(failure)
assert any(success)!=any(7)
let text_ok:result<str,str> = ok("yes")
let text_err:result<str,str> = err("no")
assert any(text_ok)!=any(text_err)
let other_ok:result<i64,str> = ok(7)
assert any(success)!=any(other_ok)
let nested:result<result<i64,error>,str> = ok(failure)
assert any(nested)!=any(success)
let any_ok:result<any,any> = ok(any(success))
let any_err:result<any,any> = err(any(cause))
assert any(any_ok)!=any(any_err)
assert eval(boxed_any,value:[any_ok,_,any_err])==[any(any_ok),_,any(any_err)]
assert eval(boxed_text,value:[text_ok,text_err])==[any(text_ok),any(text_err)]
assert eval(equal,left:[any(success),any(failure),any(cause)],right:[any(success),any(success),any(error(message:"bad",code:"value.missing_key"))])==[true,false,true]
let results:list<result<i64,error>> = [success,failure]
assert any(results)==any(results)
assert any((success,cause))==any((success,cause))
assert eval(relay,value:[any(success),_,any(failure),any(cause)])==[any(success),_,any(failure),any(cause)]
assert eval(boxed,value:[success,_,failure])==[any(success),_,any(failure)]
assert eval(boxed_error,value:[cause,_,error(message:"bad",code:null)])==[any(cause),_,any(error(message:"bad",code:null))]
let holder=Holder(outcome:failure,cause:cause,boxed:any(success))
assert eval(boxed_holder,value:[holder,_,holder])==[any(holder),_,any(holder)]
assert eval(produce,success:success,failure:failure)==[any(success),any(failure),any(error(message:"bad",code:null))]
}
"#;

fn execute(
    source: &str,
    name: &str,
    runtime: &str,
    controls: &str,
    (tests, evaluations): (usize, usize),
) -> Result<(), Box<dyn std::error::Error>> {
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
    sources.insert(0, ("any_outcomes.hgl".into(), source.into()));
    hgl_program::compile_tests(&sources)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), tests);
    if name == "shared-any-outcomes" {
        assert_eq!(suite.plans.len(), 13);
    }
    let code = decorate(hgl_program::emit_tests(&suite), runtime, controls);
    let project = regression_cache::open(&root, name)?;
    generated_cache::write(project.path.join("src/main.rs"), code)?;
    let mut manifest = format!(
        "[package]\nname={name:?}\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n"
    );
    for dependency in [
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
            "{dependency}={{path={:?}}}",
            root.join("crates").join(dependency).to_string_lossy()
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
    if runtime == GENERIC_RUNTIME {
        std::fs::write(
            root.join("target/any-captured-allocation-report.txt"),
            &output.stdout,
        )?;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains(&format!(
            "{tests} executed tests, {evaluations} evaluations, 0 failures"
        )),
        "{text}"
    );
    Ok(())
}

fn layout_name(ty: &hgl_source::Ty) -> String {
    let mut name = String::new();
    for byte in ty.source_name().bytes() {
        write!(name, "{byte:03}").unwrap_or_else(|_| unreachable!("String formatting"));
    }
    name
}
fn decorate(mut code: String, runtime: &str, controls: &str) -> String {
    if !runtime.is_empty() {
        code = code.replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    }
    if runtime == GENERIC_RUNTIME {
        code = code.replace(
            "child.evaluate_captured(ctx)",
            "crate::measured_capture(child,ctx)",
        );
    }
    code.push_str(runtime);
    if !controls.is_empty() {
        code = code.replace("fn main() {", "fn authored_main() {");
        code.push_str(controls);
    }

    code.push_str("\nstruct Provider;\nmod native {pub use hgl_stdlib::std_native::*;}\n");
    code
}

const STRICT_RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
 let(start,allocations)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));start.map_err(hgl_kernel::EngineError::Node)?;assert_eq!(allocations,0,"bounded outcome boxing startup allocated");
 let mut cycles=0;let mut now=config.start_time;let mut outcome=Ok(());
 while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
 let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));if result.is_ok(){assert_eq!(count,0,"bounded outcome boxing tick allocated");}outcome=result;cycles+=1;}
 let(stop,allocations)=hgl_alloc_count::count_in(||graph.stop(store,now));assert_eq!(allocations,0,"bounded outcome boxing cleanup allocated");hgl_types::finish(outcome,stop).map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;

const GENERIC_RUNTIME: &str = r#"
#[global_allocator]static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
static CAPTURED:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
fn measured_capture<T>(child:&mut hgl_kernel::nested::PreparedChild<T>,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult<Option<hgl_types::ErrorValue>>{let result=child.evaluate_captured(ctx);if matches!(result,Ok(Some(_))){CAPTURED.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}result}
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
 let(start,startup)=hgl_alloc_count::count_in(||graph.start(store,config.start_time));start.map_err(hgl_kernel::EngineError::Node)?;
 let mut cycles=0;let mut now=config.start_time;let mut outcome=Ok(());let mut ordinary=0;let mut captured=0;let mut escaping=0;
 while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;let before=CAPTURED.load(std::sync::atomic::Ordering::Relaxed);
 let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));if result.is_err(){escaping+=count;}else if CAPTURED.load(std::sync::atomic::Ordering::Relaxed)>before{captured+=count;}else{ordinary+=count;}outcome=result;cycles+=1;}
 let(stop,cleanup)=hgl_alloc_count::count_in(||graph.stop(store,now));hgl_types::finish(outcome,stop).map_err(hgl_kernel::EngineError::Node)?;
 println!("generic captured Any: {cycles} cycles; startup {startup}; no-new-failure ticks {ordinary}; captured-failure ticks {captured}; escaping-failure ticks {escaping}; cleanup {cleanup}");Ok(cycles)
}
"#;

#[test]
fn sparse_capture_comparison_preserves_horizon_presence_and_exact_payload_type()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module capture_comparison
fn relay(value:any)->any {when {return value}}
test equality {
let result=eval(relay,value:[any(7),_,any("x"),_])
let alias=result
assert alias==[any(7),_,any("x"),_]
assert [any(7),_,any("x"),_]==alias
assert alias!=[any(7),_,any("x")]
assert alias!=[any(7),any("x"),_,_]
let other=eval(relay,value:[any(7),_,any("x"),_])
assert alias==other
}
"#;
    execute(source, "sparse-any-comparison", STRICT_RUNTIME, "", (1, 7))?;
    let invalid = source.replace("assert alias==other", "assert alias==[7,_,7,_]");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    sources.insert(0, ("wrong.hgl".into(), invalid));
    let error = hgl_program::compile_tests(&sources)
        .expect_err("sparse capture payloads require exact identity");
    assert!(
        error.contains("captured sequence payload type mismatch"),
        "{error}"
    );
    Ok(())
}

#[test]
fn inactive_result_payloads_cannot_bypass_whole_result_equality_capability()
-> Result<(), Box<dyn std::error::Error>> {
    const DECLARATIONS: &str = r"module inactive_result
use hgraph.std::{TextOnly}
fn success(value:result<i64,TextOnly>)->result<i64,TextOnly> {when {return value}}
fn failure(value:result<TextOnly,i64>)->result<TextOnly,i64> {when {return value}}
";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let library = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    for (function, result, constructor) in [
        ("success", "result<i64,TextOnly>", "ok"),
        ("failure", "result<TextOnly,i64>", "err"),
    ] {
        for assertion in [
            "assert captured==[expected]",
            "assert [expected]==captured",
            "assert captured!=[expected]",
            "assert expected==expected",
        ] {
            let source = format!(
                "{DECLARATIONS}test capability {{let expected:{result} = {constructor}(7)\nlet captured=eval({function},value:[expected])\n{assertion}}}"
            );
            let mut sources = library.clone();
            sources.insert(0, ("inactive.hgl".into(), source));
            let error = hgl_program::compile_tests(&sources)
                .expect_err("inactive E still requires equality");
            assert!(
                error.contains("result equality requires equality of both payload types"),
                "{error}"
            );
        }
    }
    let source = format!(
        r#"{DECLARATIONS}test interpreter_capability {{
let expected:result<i64,TextOnly> = ok(7)
assert raises("value.capability") {{assert eval(success,value:[expected])==[expected]}}
let expected_failure:result<TextOnly,i64> = err(7)
assert raises("value.capability") {{assert eval(failure,value:[expected_failure])==[expected_failure]}}
}}
"#
    );
    execute(
        &source,
        "inactive-result-equality",
        STRICT_RUNTIME,
        "",
        (1, 2),
    )
}
