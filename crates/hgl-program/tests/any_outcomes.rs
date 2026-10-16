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
fn captured_diagnostic_any_keys_retain_longer_data_through_map_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module diagnostic_keys
fn required(value:i64)->i64 {when {
if value==1 {let short:result<i64,error> = err(error(message:"x",code:"value.missing_key"))
return short?}
let longer:result<i64,error> = err(error(message:"a substantially longer diagnostic message retained as the map key",code:"value.missing_key"))
return longer?}}
fn boxed(failure:error)->atomic<map<any,i64>> {when {
return map<any,i64>(items:[any(failure.message):7])}}
fn relay(value:atomic<map<any,i64>>)->atomic<map<any,i64>> {when {return delta_value(value)}}
fn retained(value:atomic<map<any,i64>>)->atomic<map<any,i64>> {when {
let copy=delta_value(value)
return copy}}
fn boxed_set(failure:error)->atomic<set<any>> {when {return set<any>(items:[any(failure.message)])}}
fn relay_set(value:atomic<set<any>>)->atomic<set<any>> {when {return delta_value(value)}}
fn retained_set(value:atomic<set<any>>)->atomic<set<any>> {when {let copy=delta_value(value)
return copy}}
fn observed(value:i64)->atomic<map<any,i64>> {let seen=capture(required,value)
retained(relay(boxed(seen.error)))}
fn observed_set(value:i64)->atomic<set<any>> {let seen=capture(required,value)
retained_set(relay_set(boxed_set(seen.error)))}
test retained_keys {
let short=map<any,i64>(items:[any("x"):7])
let longer=map<any,i64>(items:[any("a substantially longer diagnostic message retained as the map key"):7])
assert eval(observed,value:[1,_,2,1])==[short,_,longer,short]
let short_set=set<any>(items:[any("x")])
let longer_set=set<any>(items:[any("a substantially longer diagnostic message retained as the map key")])
assert eval(observed_set,value:[1,_,2,1])==[short_set,_,longer_set,short_set]
}
"#;
    execute(source, "any-captured-map-key", GENERIC_RUNTIME, "", (1, 2))
}

#[test]
fn diagnostic_text_projections_survive_two_relays_and_retained_outputs()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module diagnostic_text
native const fn failure(value:i64)->result<i64,error>
native const fn failure(value:i64)->result<i64,error> {}
native const fn error_data(value:i64)->error
native const fn error_data(value:i64)->error {}
native const fn normal(value:i64)->result<str,error>
native const fn normal(value:i64)->result<str,error> {}
fn required(value:i64)->i64 {when {return failure(value)?}}
fn message(failure:error)->str {when {return failure.message}}
fn code(failure:error)->str {when {let code=failure.code
if code!=null {return code}return "absent"}}
fn context_phase(failure:error)->str {when {let context=failure.context
if context!=null {return context.phase}return "absent"}}
fn context_node(failure:error)->str {when {let context=failure.context
if context!=null {return context.node}return "absent"}}
fn native_error(value:i64)->error {when {return error_data(value)}}
fn verify_node(failure:error,name:str)->bool {when {let context=failure.context
if context!=null {return name==context.node}return name=="absent"}}
fn identity(value:str)->str {when {return value}}
fn relay(value:str)->str {when {return value}}
fn retained(value:str)->str {when {let copy=value
return copy}}
fn observed(value:i64)->str {let seen=capture(required,value)
retained(relay(message(seen.error)))}
fn observed_code(value:i64)->str {let seen=capture(required,value)
retained(relay(code(seen.error)))}
fn observed_phase(value:i64)->str {let seen=capture(required,value)
retained(relay(context_phase(seen.error)))}
fn absent_phase(value:i64)->str => retained(relay(context_phase(native_error(value))))
fn observed_node(value:i64)->bool {let seen=capture(required,value)
verify_node(seen.error,retained(relay(context_node(seen.error))))}
fn observed_normal(value:str)->str {let seen=capture(identity,value)
retained(relay(seen.value))}
test retained_text {
assert eval(observed,value:[1,_,2,1])==["x",_,"a much longer diagnostic message","x"]
assert eval(observed_code,value:[1,_,2,0,1])==["a",_,"a much longer diagnostic code","absent","a"]
assert eval(observed_phase,value:[1,_,2,1])==["evaluate",_,"evaluate","evaluate"]
assert eval(absent_phase,value:[1,_,2,1])==["absent",_,"absent","absent"]
assert eval(observed_node,value:[1,_,2,1])==[true,_,true,true]
assert eval(observed_normal,value:["x",_,"a much longer ordinary string","x"])==["x",_,"a much longer ordinary string","x"]
}
"#;
    execute(
        source,
        "captured-diagnostic-text",
        GENERIC_RUNTIME,
        "",
        (1, 6),
    )
}

#[test]
fn finite_result_strings_publish_selected_slots_without_allocating()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module finite_result_strings
struct Holder {text:str}
fn success(value:result<str,i64>)->str {when {let outcome=value
if is_ok(outcome) {return outcome.value}return ""}}
fn failure(value:result<i64,str>)->str {when {let outcome=value
if is_err(outcome) {return outcome.error}return ""}}
fn both(value:result<str,str>)->str {when {let outcome=value
if is_ok(outcome) {return outcome.value}return outcome.error}}
fn nested(value:result<Holder,error>)->str {when {let outcome=value
if is_ok(outcome) {return outcome.value.text}return outcome.error.message}}
fn deep(value:result<result<str,i64>,error>)->str {when {let outcome=value
if is_ok(outcome) {let inner=outcome.value
if is_ok(inner) {return inner.value}return "inactive"}return outcome.error.message}}
fn metadata(value:error)->str {when {let cause=value
if cause.context!=null {return "context"}
if cause.code!=null {return cause.message}return "absent"}}
test selected_strings {
let short:result<str,i64> = ok("x")
let longer:result<str,i64> = ok("a longer selected string")
let empty:result<str,i64> = ok("")
let inactive:result<str,i64> = err(7)
assert eval(success,value:[short,_,longer,inactive,empty,longer,short])==["x",_,"a longer selected string","","","a longer selected string","x"]
let short_error:result<i64,str> = err("x")
let longer_error:result<i64,str> = err("a longer selected string")
let empty_error:result<i64,str> = err("")
let inactive_error:result<i64,str> = ok(7)
assert eval(failure,value:[short_error,_,longer_error,inactive_error,empty_error,longer_error,short_error])==["x",_,"a longer selected string","","","a longer selected string","x"]
let ok:result<str,str> = ok("x")
let err:result<str,str> = err("a longer selected string")
let blank:result<str,str> = err("")
assert eval(both,value:[ok,_,err,err,blank,ok])==["x",_,"a longer selected string","a longer selected string","","x"]
let nested_ok:result<Holder,error> = ok(Holder(text:"x"))
let nested_err:result<Holder,error> = err(error(message:"a longer selected string"))
let nested_empty:result<Holder,error> = ok(Holder(text:""))
assert eval(nested,value:[nested_ok,_,nested_err,nested_empty,nested_err,nested_ok])==["x",_,"a longer selected string","","a longer selected string","x"]
let deep_ok:result<result<str,i64>,error> = ok(short)
let deep_long:result<result<str,i64>,error> = ok(longer)
let deep_err:result<result<str,i64>,error> = ok(inactive)
let deep_empty:result<result<str,i64>,error> = err(error(message:""))
assert eval(deep,value:[deep_ok,_,deep_long,deep_err,deep_empty,deep_ok])==["x",_,"a longer selected string","inactive","","x"]
let absent=error(message:"unselected",code:null)
let present=error(message:"x",code:"")
let long=error(message:"a longer selected string",code:"present")
let empty_cause=error(message:"",code:"")
assert eval(metadata,value:[present,_,long,absent,empty_cause,long,present])==["x",_,"a longer selected string","absent","","a longer selected string","x"]
}
"#;
    execute(
        source,
        "finite-result-string-slots",
        STRICT_RUNTIME,
        "",
        (1, 6),
    )
}

#[test]
fn finite_result_index_projections_evaluate_the_index_once()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module finite_result_index
native const fn pick(value:i64)->i64
native const fn pick(value:i64)->i64 {}
fn selected(value:result<list<str>,i64>,offset:i64)->str {when {let outcome=value
if is_ok(outcome) {return outcome.value[pick(offset)]}return "absent"}}
test selected_index {
let first:result<list<str>,i64> = ok(["x","tail"])
let longer:result<list<str>,i64> = ok(["other","a longer selected string"])
let empty:result<list<str>,i64> = ok(["",""])
let inactive:result<list<str>,i64> = err(7)
assert eval(selected,value:[first,_,longer,inactive,empty,first],offset:[0,_,1,99,0,1])==["x",_,"a longer selected string","absent","","tail"]
}
"#;
    let controls = r"
static PROJECTION_CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
fn main(){authored_main();assert_eq!(PROJECTION_CALLS.load(std::sync::atomic::Ordering::Relaxed),4);}
";
    execute(
        source,
        "finite-result-string-index",
        STRICT_RUNTIME,
        controls,
        (1, 1),
    )
}

#[test]
fn capture_normal_string_ports_keep_their_finite_producer_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module finite_capture_normal
fn text(value:str)->str {when {return delta_value(value)}}
fn observed(value:str)->str {let seen=capture(text,value)
text(text(seen.value))}
test normal_strings {
assert eval(observed,value:["x",_,"a longer normal string","","a longer normal string","x"])==["x",_,"a longer normal string","","a longer normal string","x"]
}
"#;
    execute(source, "finite-capture-normal", STRICT_RUNTIME, "", (1, 1))
}

#[test]
fn capture_normal_ports_keep_unbounded_native_and_diagnostic_lineage()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module diagnostic_text
native const fn failure(value:i64)->result<i64,error>
native const fn failure(value:i64)->result<i64,error> {}
native const fn error_data(value:i64)->error
native const fn error_data(value:i64)->error {}
native const fn normal(value:i64)->result<str,error>
native const fn normal(value:i64)->result<str,error> {}
fn required(value:i64)->i64 {when {return failure(value)?}}
fn message(value:error)->str {when {return value.message}}
fn text(value:str)->str {when {return delta_value(value)}}
fn from_diagnostics(value:i64)->str {let failure=capture(required,value)
let normal=capture(message,failure.error)
text(text(normal.value))}
fn native_text(value:i64)->str {when {let outcome=normal(value)
if is_ok(outcome) {return outcome.value}return "absent"}}
fn from_native(value:i64)->str {let normal=capture(native_text,value)
text(text(normal.value))}
test unbounded_normals {
assert eval(from_diagnostics,value:[1,_,2,1])==["x",_,"a much longer diagnostic message","x"]
assert eval(from_native,value:[1,_,2,1])==["x",_,"a much longer diagnostic message","x"]
}
"#;
    execute(
        source,
        "capture-normal-provenance",
        GENERIC_RUNTIME,
        "",
        (1, 2),
    )
}

#[test]
fn finite_strings_outside_diagnostic_flow_keep_prepared_transport()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module finite_text
fn relay(value:str)->str {when {return delta_value(value)}}
fn retained(value:str)->str {when {return delta_value(value)}}
fn observed(value:str)->str => retained(relay(value))
test finite {assert eval(observed,value:["x",_,"a much longer ordinary string","x"])==["x",_,"a much longer ordinary string","x"]}
"#;
    execute(source, "finite-unmarked-text", STRICT_RUNTIME, "", (1, 1))
}

#[test]
fn diagnostic_carrier_classification_covers_all_collection_descendants() {
    use hgl_source::type_shape::TypeSpelling as _;
    for (spelling, expected) in [
        ("str", true),
        ("bytes", true),
        ("timezone", true),
        ("zoned_time", true),
        ("zoned_datetime", true),
        ("map<str,i64>", true),
        ("list<str>", true),
        ("map<any,i64>", true),
        ("map<i64,any>", true),
        ("set<any>", true),
        ("rolling<any,2,1>", true),
        ("delta<map<any,i64>>", true),
        ("delta<set<any>>", true),
        ("map<i64,i64>", false),
        ("set<i64>", false),
        ("rolling<i64,2,1>", false),
        ("delta<map<i64,i64>>", false),
    ] {
        let ty = hgl_source::Ty::parse(spelling).expect("admitted exact carrier type");
        assert_eq!(
            hgl_semantics::constructor_owners::failure_data_type(&ty),
            expected,
            "{spelling}"
        );
    }
    assert!(hgl_semantics::constructor_owners::failure_data_type(
        &hgl_source::Ty::Nullable(Box::new(hgl_source::Ty::Str))
    ));
}

#[test]
fn exact_shared_boxed_outcome_cases_execute() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(
        root.join("external/hgraph_std/hgl/hgraph/tests/result_errors.hgl"),
    )?;
    execute(&source, "shared-any-outcomes", "", "", (6, 58))
}

fn diagnostic_text_provider(
    root: &std::path::Path,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let image = root.join("target/diagnostic-text-provider.rs");
    generated_cache::write(
        &image,
        r#"
pub const PROVIDER:&str="hgl.diagnostic_text";pub const FINGERPRINT:&str="diagnostic-text-v1";
pub fn failure_i64(value:i64)->Result<i64,hgl_types::ErrorValue>{Err(error_data_i64(value))}
pub fn normal_i64(value:i64)->Result<String,hgl_types::ErrorValue>{Ok(error_data_i64(value).message)}
pub fn error_data_i64(value:i64)->hgl_types::ErrorValue{hgl_types::ErrorValue{
message:if value==2 {"a much longer diagnostic message"}else{"x"}.into(),
code:if value==0 {None}else{Some(if value==2 {"a much longer diagnostic code"}else{"a"}.into())},context:None}}
"#,
    )?;
    Ok((
        "diagnostic-text.hgl-native".into(),
        format!(
            "native_descriptor 1 target \"hgl-rust-1\"\nmodule diagnostic_text\nprovider \"hgl.diagnostic_text\" fingerprint \"diagnostic-text-v1\" image {:?}\nvalue failure(value:i64)->result<i64,error> symbol \"failure_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\nvalue error_data(value:i64)->error symbol \"error_data_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\nvalue normal(value:i64)->result<str,error> symbol \"normal_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\n",
            image.to_string_lossy()
        ),
    ))
}

fn projection_index_provider(
    root: &std::path::Path,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let image = root.join("target/projection-index-provider.rs");
    generated_cache::write(
        &image,
        r#"
pub const PROVIDER:&str="hgl.projection_index";pub const FINGERPRINT:&str="projection-index-v1";
pub fn pick_i64(value:i64)->i64{crate::PROJECTION_CALLS.fetch_add(1,std::sync::atomic::Ordering::Relaxed);value}
"#,
    )?;
    Ok((
        "projection-index.hgl-native".into(),
        format!(
            "native_descriptor 1 target \"hgl-rust-1\"\nmodule finite_result_index\nprovider \"hgl.projection_index\" fingerprint \"projection-index-v1\" image {:?}\nvalue pick(value:i64)->i64 symbol \"pick_i64\" ownership owned phases {{setup,cold,evaluation}} effects {{}}\n",
            image.to_string_lossy()
        ),
    ))
}

fn add_test_provider(
    sources: &mut Vec<(String, String)>,
    root: &std::path::Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(
        name,
        "captured-diagnostic-text" | "capture-normal-provenance"
    ) {
        sources.push(diagnostic_text_provider(root)?);
    } else if name == "finite-result-string-index" {
        sources.push(projection_index_provider(root)?);
    }
    Ok(())
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
    add_test_provider(&mut sources, &root, name)?;
    hgl_program::compile_tests(&sources)?;
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert_eq!(suite.tests.len(), tests);
    assert_plan_profile(name, &suite.plans);
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
        report_allocations(&root, name, &output.stdout)?;
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

fn assert_plan_profile(name: &str, plans: &[hgl_semantics::ir::Plan]) {
    if name == "shared-any-outcomes" {
        assert_eq!(plans.len(), 13);
    } else if name == "captured-diagnostic-text" {
        assert_eq!(plans.len(), 6);
    } else if name == "finite-unmarked-text" {
        assert_eq!(plans.len(), 1);
        assert!(hgl_rust::execution_proof::prepared(&plans[0]));
        assert!(hgl_semantics::constructor_owners::unbounded_failure_data(&plans[0]).is_empty());
    } else if name == "finite-capture-normal" {
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        let unknown = hgl_semantics::constructor_owners::unbounded_failure_data(plan);
        assert_eq!(unknown.len(), 1);
        assert!(unknown.contains(&plan.captures[0].owner));
        assert!(
            plan.output
                .as_ref()
                .is_some_and(|(output, _)| !unknown.contains(output))
        );
    } else if name == "capture-normal-provenance" {
        assert_eq!(plans.len(), 2);
        for plan in plans {
            let unknown = hgl_semantics::constructor_owners::unbounded_failure_data(plan);
            assert!(
                plan.output
                    .as_ref()
                    .is_some_and(|(output, _)| unknown.contains(output))
            );
        }
    }
}

fn report_allocations(root: &std::path::Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let report = if name == "any-captured-map-key" {
        "target/any-captured-map-key-allocation-report.txt"
    } else if name == "captured-diagnostic-text" {
        "target/captured-diagnostic-text-allocation-report.txt"
    } else if name == "capture-normal-provenance" {
        "target/capture-normal-provenance-allocation-report.txt"
    } else {
        "target/any-captured-allocation-report.txt"
    };
    std::fs::write(root.join(report), bytes)
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

#[test]
fn unseeded_result_publications_prepare_both_payload_slots_from_cold_child_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r#"module unseeded_result
use hgraph.std::{TextOnly}
fn make(value:i64)->result<i64,TextOnly> {when {return ok(value)}}
fn reject(value:i64)->result<TextOnly,i64> {when {return err(value)}}
fn success(value:result<i64,TextOnly>)->i64 {when {let outcome=value
if is_ok(outcome) {return outcome.value}return 0}}
fn failure(value:result<TextOnly,i64>)->i64 {when {let outcome=value
if is_err(outcome) {return outcome.error}return 0}}
fn made(value:i64)->i64 => success(make(value))
fn rejected(value:i64)->i64 => failure(reject(value))
fn make_text(value:str)->result<str,i64> {when {return ok(value)}}
fn reject_text(value:str)->result<i64,str> {when {return err(value)}}
fn alternating(value:i64)->result<i64,i64> {when {if value>0 {return ok(value)}return err(value)}}
fn nested(value:str)->result<result<str,i64>,i64> {when {return ok(ok(value))}}
test unseeded_slots {
assert eval(made,value:[7,_,9])==[7,_,9]
assert eval(rejected,value:[7,_,9])==[7,_,9]
let first:result<str,i64> = ok("first")
let longer:result<str,i64> = ok("longer replacement")
let empty:result<str,i64> = ok("")
assert eval(make_text,value:["first",_,"longer replacement",""])==[first,_,longer,empty]
let first_error:result<i64,str> = err("first")
let longer_error:result<i64,str> = err("longer replacement")
let empty_error:result<i64,str> = err("")
assert eval(reject_text,value:["first",_,"longer replacement",""])==[first_error,_,longer_error,empty_error]
let success:result<i64,i64> = ok(7)
let failure:result<i64,i64> = err(-1)
assert eval(alternating,value:[7,-1,7,-1])==[success,failure,success,failure]
let nested_first:result<result<str,i64>,i64> = ok(first)
let nested_longer:result<result<str,i64>,i64> = ok(longer)
let nested_empty:result<result<str,i64>,i64> = ok(empty)
assert eval(nested,value:["first",_,"longer replacement",""])==[nested_first,_,nested_longer,nested_empty]
}
"#;
    execute(
        source,
        "unseeded-result-capacity",
        STRICT_RUNTIME,
        "",
        (1, 6),
    )
}
