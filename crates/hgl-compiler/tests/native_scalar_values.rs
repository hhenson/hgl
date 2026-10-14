//! Descriptor-defined scalar values retain exact nominal identity and owning provider data.
use std::{fmt::Write as _, fs, path::PathBuf, process::Command, time::SystemTime};

fn root() -> Result<PathBuf, std::io::Error> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
}
fn sources(body: &str) -> Result<Vec<(String, String)>, String> {
    let root = root().map_err(|e| e.to_string())?;
    let mut sources = hgl_program::library_files::sources(
        &[
            root.join("external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl"),
            root.join("native/stdlib/atomic_values.hgl-native"),
            root.join("external/hgraph_std/hgl/hgraph/replay_record.hgl"),
            root.join("external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
        ],
        &[],
    )?;
    sources.insert(0, (
        "native-cases.hgl".into(),
        format!("module native_cases\nuse hgraph.std::{{Token,TokenOther,TextOnly,token,token_text,token_other,token_other_text,text_only,text_only_text}}\n{body}"),
    ));
    Ok(sources)
}
fn run(body: &str) -> Result<(), Box<dyn std::error::Error>> {
    run_sources(&sources(body)?)
}
fn run_sources(input: &[(String, String)]) -> Result<(), Box<dyn std::error::Error>> {
    run_sources_with_counts(input, None)
}
fn run_sources_with_counts(
    input: &[(String, String)],
    calls: Option<[usize; 3]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let suite = hgl_program::compile_module_suite(input)?;
    assert!(
        !suite.tests.is_empty(),
        "native fixture must execute authored tests"
    );
    let generated = hgl_program::emit_tests(&suite);
    let start = "store.prepare_collection_inputs();hgl_kernel::run_simulation(";
    let end = ").map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>";
    let generated = generated.replace(start,"store.prepare_collection_inputs();let (run,allocations)=hgl_alloc_count::count_in(||hgl_kernel::run_simulation(").replace(end,"));if run.is_ok(){assert_eq!(allocations,0,\"native scalar evaluation allocated\");}run.map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>");
    let mut generated = format!(
        "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;\n{generated}"
    );
    if let Some(calls) = calls {
        generated = generated.replace("store.prepare_collection_inputs();", "{let choices=store.global_state().bind::<hgl_store::List<i64>>(\"choices\").map_err(|e|e.message)?;store.global_state().set(choices,&vec![1,2]).map_err(|e|e.message)?;}store.prepare_collection_inputs();");
        generated = generated.replace("fn main() {", "fn authored_main() {");
        writeln!(
            generated,
            "static NATIVE_CALLS:[std::sync::atomic::AtomicUsize;3]=[const {{std::sync::atomic::AtomicUsize::new(0)}};3];fn main(){{authored_main();for (counter,expected) in NATIVE_CALLS.iter().zip({calls:?}) {{assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed),expected,\"native helper executed more than once per authored invocation\");}}}}"
        )?;
    }
    execute(&generated)
}
#[test]
fn native_owning_helpers_prepare_primitive_inputs_configuration_literals_and_nested_calls()
-> Result<(), Box<dyn std::error::Error>> {
    let input = primitive_sources(
        r#"
use hgraph.std::{primitive_token,primitive_text,primitive_size}
fn make(value:i64,flag:bool,const fixed:i64)->Token {when {return primitive_token(value,flag,fixed)}}
fn text(value:i64,flag:bool)->str {when {return primitive_text(value,flag,1)}}
fn nested(value:str,flag:bool)->Token {when {return primitive_token(primitive_size(value),flag,1)}}
fn configured(value:i64,const flag:bool)->Token {when {return primitive_token(value,flag,1)}}
fn locals(value:i64,flag:bool)->Token {when {let number=value
var choice=flag
return primitive_token(number,choice,1)}}
fn globals(value:i64,flag:bool)->Token {inject global_state
when {set(global_state,"number",value)
return primitive_token(get(global_state,"number"),flag,1)}}
fn literal(value:i64)->str {when {return primitive_text(value,true,1)}}
fn expression(value:i64)->Token {when {return primitive_token(value+1,(value>=0)&&!(value>1),1)}}
fn modulo(value:i64)->Token {when {return primitive_token((value*2-1)%5,true,1)}}
fn queried(value:i64)->Token {when {return primitive_token(value,valid(value),1)}}
fn indexed(value:i64)->Token {inject global_state
when {let choices:list<i64> =get(global_state,"choices")
return primitive_token(choices[value],true,1)}}
test primitive_helpers {
assert eval(make,value:[1,_,2],flag:[true,_,false],fixed:1)==[token("yes"),_,token("no")]
assert eval(text,value:[1,_,2],flag:[true,_,false])==["yes",_,"no"]
assert eval(nested,value:["x",_,"xx"],flag:[true,_,false])==[token("yes"),_,token("no")]
assert eval(configured,value:[1,_,2],flag:true)==[token("yes"),_,token("no")]
assert eval(locals,value:[1,_,2],flag:[true,_,false])==[token("yes"),_,token("no")]
assert eval(globals,value:[1,_,2],flag:[true,_,false])==[token("yes"),_,token("no")]
assert eval(literal,value:[1,_,2])==["yes",_,"no"]
assert eval(expression,value:[0,_,1,9223372036854775807])==[token("yes"),_,token("no"),token("no")]
assert eval(modulo,value:[1,_,-2])==[token("yes"),_,token("no")]
assert eval(queried,value:[1,_,2])==[token("yes"),_,token("no")]
assert eval(indexed,value:[0,_,1])==[token("yes"),_,token("no")]
}
"#,
    )?;
    run_sources_with_counts(&input, Some([19, 23, 2]))
}
fn primitive_sources(body: &str) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut input = sources(body)?;
    for (name, text) in &mut input {
        if name.ends_with("atomic_values_fixture.hgl") {
            text.push_str("\nexport native const fn primitive_token(value:i64,flag:bool,fixed:i64)->Token\nexport native const fn primitive_text(value:i64,flag:bool,fixed:i64)->str\nexport native const fn primitive_size(value:str)->i64\n");
        } else if name.ends_with(".hgl-native") {
            *text = text.replace(
                "\"atomic_values.rs\"",
                "\"../../crates/hgl-compiler/tests/fixtures/native_primitive_provider.rs\"",
            );
            for (name, result, bound) in [
                ("primitive_token", "Token", " bound \"primitive_width\""),
                ("primitive_text", "str", " bound \"primitive_width\""),
            ] {
                writeln!(
                    text,
                    "value {name}(value:i64,flag:bool,fixed:i64)->{result} symbol \"{name}\"{bound} ownership owned phases {{setup,cold,evaluation}} effects {{}}"
                )?;
            }
            text.push_str("value primitive_size(value:str)->i64 symbol \"primitive_size\" ownership owned phases {setup,cold,evaluation} effects {}\n");
        }
    }
    Ok(input)
}
#[test]
fn unsupported_runtime_native_transport_rejects_planning_but_cold_calls_execute()
-> Result<(), Box<dyn std::error::Error>> {
    let input = primitive_sources(
        r#"
use hgraph.std::{primitive_token}
const fn increment(value:i64)->i64 {return value+1}
fn unsupported(value:i64)->Token {when {return primitive_token(increment(value),true,1)}}
fn source()->i64 {inject alarm
start {schedule(alarm,0us)}
when {return 0}}
export fn main() {unsupported(source())}
test unsupported_runtime {assert eval(unsupported,value:[0])==[token("yes")]}
"#,
    )?;
    hgl_program::check_sources(&input)?;
    let program = hgl_program::compile(&input, "main")?;
    let target = hgl_program::emit_rust(&program);
    assert!(target.starts_with("compile_error!("), "{target}");
    assert!(
        target.contains("Rust native target does not support prepared owning result transport")
    );
    assert!(!target.contains("native_primitive_provider.rs") && !target.contains("native_image"));
    let error = hgl_program::compile_module_suite(&input)
        .err()
        .ok_or("unmapped runtime transport admitted")?;
    assert!(
        error.contains("Rust native target does not support prepared owning result transport"),
        "{error}"
    );
    let input = primitive_sources(
        r#"
use hgraph.std::{primitive_token}
const fn increment(value:i64)->i64 {return value+1}
test supported_cold {assert primitive_token(increment(0),true,1)==token("yes")}
"#,
    )?;
    run_sources_with_counts(&input, Some([1, 1, 0]))
}

#[test]
fn unsupported_owning_builtin_provider_results_preserve_source_and_cold_support()
-> Result<(), Box<dyn std::error::Error>> {
    let declarations = [
        ("owned_bytes", "bytes"),
        ("owned_zone", "timezone"),
        ("owned_time", "zoned_time"),
        ("owned_datetime", "zoned_datetime"),
    ];
    let mut interface =
        "module builtin_results\nexport native const fn fixed(value:i64)->i64\n".to_owned();
    let mut descriptor = "native_descriptor 1 target \"hgl-rust-1\"\nmodule builtin_results\nprovider \"hgl.test.builtin_results\" fingerprint \"builtin-results-v1\" image \"native_builtin_results.rs\"\nvalue fixed(value:i64)->i64 symbol \"fixed\" ownership owned phases {setup,cold,evaluation} effects {}\n".to_owned();
    for (helper, result) in declarations {
        writeln!(
            interface,
            "export native const fn {helper}(value:i64)->{result}"
        )?;
        writeln!(
            descriptor,
            "value {helper}(value:i64)->{result} symbol {helper:?} bound \"builtin_width\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}"
        )?;
    }
    let fixture = root()?.join("crates/hgl-compiler/tests/fixtures");
    let input = |body: &str| -> Result<Vec<(String, String)>, String> {
        let mut input = sources(body)?;
        input.push(("builtin-results.hgl".into(), interface.clone()));
        input.push((
            fixture
                .join("builtin-results.hgl-native")
                .to_string_lossy()
                .into_owned(),
            descriptor.clone(),
        ));
        Ok(input)
    };
    for (helper, result) in declarations {
        let input = input(&format!(
            "use builtin_results::{{{helper}}}\nfn unsupported(value:i64)->{result} {{when {{return {helper}(value)}}}}\nfn source()->i64 {{yield 0us:1}}\nexport fn main() {{unsupported(source())}}\ntest runtime {{eval(unsupported,[1])}}"
        ))?;
        hgl_program::check_sources(&input)?;
        let program = hgl_program::compile(&input, "main")?;
        let target = hgl_program::emit_rust(&program);
        assert!(target.starts_with("compile_error!("), "{helper}: {target}");
        assert!(
            target.contains("Rust native target does not support prepared owning result transport")
        );
        assert!(!target.contains("native_builtin_results.rs") && !target.contains("native_image"));
        let error = hgl_program::compile_module_suite(&input)
            .err()
            .ok_or("unsupported owning builtin target admitted")?;
        assert!(
            error.contains("Rust native target does not support prepared owning result transport"),
            "{helper}: {error}"
        );
    }
    let input = input(
        r"
use builtin_results::{owned_bytes,owned_zone,owned_time,owned_datetime,fixed}
fn copy(value:i64)->i64 {when {return fixed(value)}}
test cold_results {
assert owned_bytes(1)==bytes([1])
assert owned_zone(1)==owned_zone(1)
assert owned_zone(1)!=owned_zone(2)
assert owned_time(1)==owned_time(1)
assert owned_time(1)!=owned_time(2)
assert owned_datetime(1)==owned_datetime(1)
assert owned_datetime(1)!=owned_datetime(2)
assert eval(copy,[1,_,2])==[2,_,3]
}
",
    )?;
    run_sources(&input)
}

#[test]
fn inferred_native_values_cannot_shadow_visible_test_nominal_identities() -> Result<(), String> {
    let mut input = sources(
        r#"
const fn configured()->Token {return token("a")}
test {
struct Clash {value:i64}
test incompatible_identity {let opaque=configured()
let structural=Clash(value:1)
assert any(opaque)!=any(structural)}
}
"#,
    )?;
    let (_, descriptor) = input
        .iter_mut()
        .find(|(name, _)| name.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    *descriptor = descriptor.replace("hgraph.std::Token\"", "native_cases::Clash\"");
    let error = hgl_program::compile_module_suite(&input)
        .err()
        .ok_or("inferred native identity collision admitted")?;
    assert!(
        error.contains(
            "native canonical identity conflicts with non-native HGL type native_cases::Clash"
        ),
        "{error}"
    );
    Ok(())
}
#[test]
fn modules_share_one_provider_image_and_retain_all_scalar_mappings_and_aliases()
-> Result<(), Box<dyn std::error::Error>> {
    let mut input = sources("")?;
    input.retain(|(name, _)| {
        !name.ends_with("atomic_values_fixture.hgl")
            && !name.ends_with(".hgl-native")
            && name != "native-cases.hgl"
    });
    input.insert(
        0,
        (
            "shared-native-cases.hgl".into(),
            r#"
module shared_native_cases
use first::{Token,make}
use second::{TokenOther,other}
use aliases::{Alias,alias}
fn token_pass(value:str)->Token {when {return make(value)}}
fn other_pass(value:str)->TokenOther {when {return other(value)}}
fn alias_pass(value:Token)->Alias {when {return value}}
test shared_image {
assert eval(token_pass,value:["a",_,""])==[make("a"),_,make("")]
assert eval(other_pass,value:["b",_,""])==[other("b"),_,other("")]
assert eval(alias_pass,value:[make("a"),_,make("")])==[alias("a"),_,alias("")]
assert alias("same")==make("same")
}
"#
            .into(),
        ),
    );
    let image = root()?.join("native/stdlib/atomic_values.rs");
    for (module, ty, helper, marker, canonical, symbol) in [
        (
            "first",
            "Token",
            "make",
            "Token",
            "hgraph.std::Token",
            "token_str",
        ),
        (
            "second",
            "TokenOther",
            "other",
            "TokenOther",
            "hgraph.std::TokenOther",
            "token_other_str",
        ),
        (
            "aliases",
            "Alias",
            "alias",
            "Token",
            "hgraph.std::Token",
            "token_str",
        ),
    ] {
        input.push((format!("{module}.hgl"), format!("module {module}\nexport native type {ty}\nexport native const fn {helper}(text:str)->{ty}\n")));
        input.push((format!("{module}.hgl-native"), format!("native_descriptor 1 target \"hgl-rust-1\"\nmodule {module}\nprovider \"hgl.native_atomic_fixture\" fingerprint \"native-atomic-values-v1\" image {:?}\nscalar {ty} canonical \"{canonical}\" capabilities {{copy,text,equality,hash,order,serialization}} marker \"{marker}\"\nvalue {helper}(text:str)->{ty} symbol \"{symbol}\" bound \"identity_width\" ownership owned phases {{setup,cold,evaluation}} effects {{allocation}}\n", image.to_string_lossy())));
    }
    run_sources(&input)
}
fn execute(generated: &str) -> Result<(), Box<dyn std::error::Error>> {
    static GENERATED: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = GENERATED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = root()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-native-scalar-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let mut manifest = "[package]\nname=\"hgl-native-scalar\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-alloc-count",
    ] {
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name).to_string_lossy()
        )?;
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    fs::write(directory.join("src/main.rs"), generated)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--offline", "--quiet"])
        .arg("--target-dir")
        .arg(root.join("target/source-tests"))
        .current_dir(&directory);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{}\n{}\nsource: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        directory.display()
    );
    fs::remove_dir_all(directory)?;
    Ok(())
}
#[test]
fn ordinary_native_setup_owns_replacements_and_uses_provider_comparison()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    test setup {
        var current:Token=token("before")
        let retained=current
        let text=token_text(current)
        current=token("after")
        assert token_text(retained)=="before"
        assert text=="before"
        assert token_text(current)=="after"
        assert token("same")==token("same")
        assert token("left")<token("right")
        assert text_only_text(text_only("opaque"))=="opaque"
    }
    "#)
}

#[test]
fn scalar_native_publications_preserve_equal_ticks_silence_and_empty_values()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    fn pass(value:Token)->Token {when {return value}}
    fn atomic_pass(value:atomic<Token>)->Token {when {return value}}
    fn delta_pass(value:Token)->delta<Token> {when {return delta_value(value)}}
    test scalar {
        assert eval(pass,value:[token(""),token("same"),token("same"),_,token("last")])==[token(""),token("same"),token("same"),_,token("last")]
        assert eval(atomic_pass,value:[token("a"),_,token("b")])==[token("a"),_,token("b")]
        assert eval(delta_pass,value:[token("a"),token("a")])==[token("a"),token("a")]
        assert eval(pass,value:[])==[]
        assert eval(pass,value:[_,_])==[_,_]
    }
    "#)
}

#[test]
fn native_evaluation_helper_results_retain_owning_destinations()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    struct TokenField {value:Token}
    fn make(value:str)->Token {when {return token(value)}}
    fn field(value:Token)->str {inject global_state
when {set(global_state,"field",TokenField(value:value))
let saved:TokenField=get(global_state,"field")
return token_text(saved.value)}}
    fn observe(value:Token)->str {when {return token_text(value)}}
    fn observe_alias(value:Token)->str {when {let alias=value
return token_text(alias)}}
    fn copy_alias(value:Token)->Token {when {let alias=value
return token(token_text(alias))}}
    fn copy_global(value:Token)->Token {inject global_state
when {set(global_state,"source",value)
set(global_state,"source",token(token_text(get(global_state,"source"))))
return token(token_text(get(global_state,"source")))}}
 fn lifecycle(value:Token,const initial:Token)->Token {inject global_state
start {let retained=initial
set(global_state,"lifecycle",retained)}
when {return token(token_text(value))}
stop {let retained:Token=get(global_state,"lifecycle")
set(global_state,"lifecycle",retained)}}
 fn compare(value:Token)->bool {inject global_state
when {
set(global_state,"compared",value)
var local:Token=get(global_state,"compared")
let retained=local
local=token("replacement")
let stored:Token=get(global_state,"compared")
return retained==value && stored==value && local==token("replacement") && value<=value && value>=value && value<token("z") && token("z")>value && value!=token("z")}}
 test helpers {
assert eval(field,value:[token("a"),_,token("")])==["a",_,""]
assert eval(lifecycle,value:[token("a"),_,token("")],initial:token("start"))==[token("a"),_,token("")]
        assert eval(compare,value:[token("a"),_,token("b"),token("")])==[true,_,true,true]
        assert eval(make,value:["","same","same",_,"last"])==[token(""),token("same"),token("same"),_,token("last")]
        assert eval(observe,value:[token("a"),_,token("b")])==["a",_,"b"]
        assert eval(observe_alias,value:[token("a"),_,token("b")])==["a",_,"b"]
        assert eval(copy_alias,value:[token("a"),_,token("b"),token("")])==[token("a"),_,token("b"),token("")]
        assert eval(copy_global,value:[token("a"),_,token("b"),token("")])==[token("a"),_,token("b"),token("")]
 }
 "#)
}

#[test]
fn native_scalar_negative_contract_matrix_rejects_before_execution() -> Result<(), String> {
    for body in [
        "test invalid {let value:Token=token_other(\"same\")}",
        "test invalid {let value=Token(\"same\")}",
        "test invalid {let value=token(\"same\").text}",
        "test invalid {assert text_only(\"same\")==text_only(\"same\")}",
        "test invalid {assert token(\"same\")==token_other(\"same\")}",
        "test invalid {assert token(\"same\")+token(\"same\")==token(\"samesame\")}",
        "test invalid {let value:Token<i64>=token(\"same\")}",
        "test invalid {let immutable=token(\"same\")\nimmutable=token(\"later\")}",
        "fn bounded(value:list<i64,(token_text(token(\"checking\")))>)->i64 {when {return 1}}\ntest invalid {assert eval(bounded,value:[[1]])==[1]}",
    ] {
        assert!(
            hgl_program::compile_module_suite(&sources(body)?).is_err(),
            "admitted invalid native contract: {body}"
        );
    }
    Ok(())
}

#[test]
fn evaluation_native_alias_cannot_become_a_cold_key_recipe() -> Result<(), String> {
    let body = "fn invalid(value:i64)->set<Token> {when {let key=token(\"runtime\")\nreturn delta<set<Token>>(added:[key])}}\ntest invalid_key {assert eval(invalid,value:[1])==[delta<set<Token>>(added:[token(\"runtime\")])]}";
    let error = hgl_program::compile_module_suite(&sources(body)?)
        .err()
        .ok_or("evaluation-local native recipe was admitted")?;
    assert!(
        error.contains("delta position requires a constant"),
        "{error}"
    );
    Ok(())
}

#[test]
fn owning_evaluation_requires_explicit_pure_target_bound() -> Result<(), String> {
    let mut input = sources(
        "fn make(value:str)->Token {when {return token(value)}}\ntest bound {assert eval(make,value:[\"a\"])==[token(\"a\")]}",
    )?;
    let (_, descriptor) = input
        .iter_mut()
        .find(|(name, _)| name.ends_with(".hgl-native"))
        .ok_or("descriptor missing")?;
    *descriptor = descriptor.replace(" bound \"identity_width\"", "");
    let error = hgl_program::compile_module_suite(&input)
        .err()
        .ok_or("missing native bound admitted")?;
    assert!(error.contains("pure target bound"), "{error}");
    Ok(())
}

#[test]
fn native_aggregate_and_local_owners_retain_independently() -> Result<(), Box<dyn std::error::Error>>
{
    run(r#"
    struct Pair {
 first:Token
 second:Token
 }
    fn local(value:Token)->Token {when {var current=value
let retained=current
current=token("replacement")
return retained}}
    fn constructor(value:Token)->Token {inject global_state
when {set(global_state,"saved",Pair(first:value,second:value))
var retained:Pair=Pair(first:token("initial"),second:token("initial"))
if true {let observed:Pair=get(global_state,"saved")
retained=Pair(first:observed.first,second:observed.second)}
set(global_state,"saved",Pair(first:token("replacement"),second:token("replacement")))
return retained.first}}
    fn list_pass(value:atomic<list<Token>>)->atomic<list<Token>> {when {return delta_value(value)}}
    fn pair_pass(value:atomic<Pair>)->atomic<Pair> {when {return delta_value(value)}}
    fn tuple_pass(value:atomic<tuple<Token,str>>)->atomic<tuple<Token,str>> {when {return delta_value(value)}}
    test owners {
assert eval(constructor,value:[token("a"),_,token("")])==[token("a"),_,token("")]
        assert eval(local,value:[token("a"),token("b")])==[token("a"),token("b")]
        assert eval(list_pass,value:[[token("a"),token("b")],[],[token("")]])==[[token("a"),token("b")],[],[token("")]]
        assert eval(pair_pass,value:[Pair(first:token("a"),second:token("b"))])==[Pair(first:token("a"),second:token("b"))]
        assert eval(tuple_pass,value:[(token("a"),"b")])==[(token("a"),"b")]
    }
    "#)
}

#[test]
fn native_boxes_and_cold_collection_keys_preserve_provider_values()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    fn boxed(value:Token)->any {when {return any(value)}}
    fn nested(value:Token)->any {when {return any((value,value))}}
    test boxes_and_keys {
        var original=any(token("retained"))
        let alias=original
        original=any(token("replacement"))
        assert alias==any(token("retained"))
        assert any(token("same"))!=any(token_other("same"))
        assert any(text_only("same"))!=any(token("same"))
        assert eval(boxed,value:[token(""),token("same"),_,token("last")])==[any(token("")),any(token("same")),_,any(token("last"))]
        assert eval(nested,value:[token("a"),token("b")])==[any((token("a"),token("a"))),any((token("b"),token("b")))]
        let first=set<Token>(items:[token("a"),token("b")])
        assert first==set<Token>(items:[token("b"),token("a")])
        let mapping=map<Token,i64>(items:[token("a"):1,token("b"):2])
        assert mapping==map<Token,i64>(items:[token("b"):2,token("a"):1])
    }
    "#)
}

#[test]
fn native_sparse_windows_globals_and_timed_values_keep_complete_owners()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    use hgraph.std::{TimedValue}
    struct Quote {bid:Token
ask:Token}
    fn pass<T>(value:T)->T {when{return delta_value(value)}}
    fn list_pass(value:list<Token,2>)->list<Token,2> =>pass(value)
    fn growing_pass(value:list<Token>)->list<Token> =>pass(value)
    fn tuple_pass(value:tuple<Token,i64>)->tuple<Token,i64> =>pass(value)
    fn quote_pass(value:Quote)->Quote =>pass(value)
    fn map_pass(value:map<i64,Token>)->map<i64,Token> =>pass(value)
    fn window(value:rolling<Token,2>)->rolling<Token,2> =>pass(value)
    fn text_pass(value:TextOnly)->str {when{return text_only_text(value)}}
    fn globals(value:Token)->Token {
        inject global_state
        when {
            set(global_state,"retained",value)
            var local:Token=get(global_state,"retained")
            let old=local
            local=token("replacement")
            let unchanged:Token=get(global_state,"retained")
            return unchanged
        }
    }
    test shapes {
        assert eval(list_pass,value:[delta<list<Token,2>>(items:[1:token("a")]),_,delta<list<Token,2>>(items:[0:token("b")])])==[delta<list<Token,2>>(items:[1:token("a")]),_,delta<list<Token,2>>(items:[0:token("b")])]
        assert eval(growing_pass,value:[delta<list<Token>>(items:[0:token("a")]),delta<list<Token>>(items:[1:token("b")])])==[delta<list<Token>>(items:[0:token("a")]),delta<list<Token>>(items:[1:token("b")])]
        assert eval(tuple_pass,value:[delta<tuple<Token,i64>>(items:[0:token("a")]),delta<tuple<Token,i64>>(items:[1:2])])==[delta<tuple<Token,i64>>(items:[0:token("a")]),delta<tuple<Token,i64>>(items:[1:2])]
        assert eval(quote_pass,value:[delta<Quote>(bid:token("a")),delta<Quote>(ask:token("b"))])==[delta<Quote>(bid:token("a")),delta<Quote>(ask:token("b"))]
        assert eval(map_pass,value:[delta<map<i64,Token>>(upsert:[1:token("a")]),delta<map<i64,Token>>(remove:[1]),delta<map<i64,Token>>(upsert:[1:token("b")])])==[delta<map<i64,Token>>(upsert:[1:token("a")]),delta<map<i64,Token>>(remove:[1]),delta<map<i64,Token>>(upsert:[1:token("b")])]
        assert eval(window,value:[token("a"),token("a"),_,token("b")])==[token("a"),token("a"),_,token("b")]
        assert eval(text_pass,value:[text_only("a"),_,text_only("b")])==["a",_,"b"]
        assert eval(globals,value:[token("a"),token("b")])==[token("a"),token("b")]
        assert eval(globals,value:[token("")])==[token("")]
        let sample:TimedValue<Token> =TimedValue(time:@1970-01-01T00:00:00.000001Z,value:token("timed"))
        assert sample.time==@1970-01-01T00:00:00.000001Z
        assert sample.value==token("timed")
    }
    "#)
}

#[test]
fn native_sparse_key_lifecycle_retains_cold_aliases() -> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    fn pass<T>(value:T)->T {when{return delta_value(value)}}
    fn sets(value:set<Token>)->set<Token> =>pass(value)
    fn maps(value:map<Token,i64>)->map<Token,i64> =>pass(value)
    fn boxes(value:set<any>)->set<any> =>pass(value)
    test keys {
        let key=token("same")
        let box=any(key)
        assert eval(boxes,value:[delta<set<any>>(added:[box]),delta<set<any>>(removed:[box])])==[delta<set<any>>(added:[box]),delta<set<any>>(removed:[any(key)])]
        assert eval(sets,value:[delta<set<Token>>(added:[key]),delta<set<Token>>(removed:[key]),delta<set<Token>>(added:[key])])==[delta<set<Token>>(added:[key]),delta<set<Token>>(removed:[key]),delta<set<Token>>(added:[key])]
        assert eval(maps,value:[delta<map<Token,i64>>(upsert:[key:1]),delta<map<Token,i64>>(remove:[key]),delta<map<Token,i64>>(upsert:[key:2])])==[delta<map<Token,i64>>(upsert:[key:1]),delta<map<Token,i64>>(remove:[key]),delta<map<Token,i64>>(upsert:[key:2])]
    }
    "#)
}

#[test]
fn mutable_constructor_owners_preserve_ordinary_and_boxed_aliases()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
struct Text { value:str }
struct Fixed { value:i64 }
fn text_alias(value:str)->bool {when {
var current=Text(value:value)
let retained=current
let boxed=any(current)
current=Text(value:"replacement")
return retained.value==value && current.value=="replacement" && boxed==any(Text(value:value))}}
fn scalar_alias(value:i64)->bool {when {
var current=Fixed(value:value)
let retained=current
current=Fixed(value:99)
return retained.value==value && current.value==99}}
test mutable_owners {
assert eval(text_alias,value:["a",_,""])==[true,_,true]
assert eval(scalar_alias,value:[1,_,2])==[true,_,true]
}
"#)
}

#[test]
fn standalone_native_results_and_aliases_use_independent_ordinary_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"
fn source()->str {inject alarm
start {schedule(alarm,0us)}
when {return "retained"}}
fn make(value:str)->Token {when {return token(value)}}
fn consume(value:atomic<Token>) {inject global_state
when {
var current=delta_value(value)
let original=current
current=token("replacement")
set(global_state,"saved",original)
let saved:Token=get(global_state,"saved")
set(global_state,"text",token_text(original))
set(global_state,"saved",token(token_text(saved)))
set(global_state,"replacement",token_text(current))
set(global_state,"empty",token_text(token("")))
let matching=token("retained")
set(global_state,"equal",original==matching && original<=matching && original>=matching && original!=current && original>current && current<original)
}}
export fn main() {consume(make(source()))}
"#;
    let mut generated = String::new();
    for (name, body) in [
        ("success", body.to_owned()),
        (
            "failure",
            body.replace("token(token_text(saved))", "token(\"failure\")"),
        ),
    ] {
        let mut input = sources(&body)?;
        let (_, descriptor) = input
            .iter_mut()
            .find(|(name, _)| name.ends_with(".hgl-native"))
            .ok_or("descriptor missing")?;
        *descriptor = descriptor
            .replace("->Token symbol", "->Token throws symbol")
            .replace(
                "\"atomic_values.rs\"",
                "\"../../crates/hgl-compiler/tests/fixtures/native_generic_provider.rs\"",
            );
        let code = hgl_program::emit_rust(&hgl_program::compile(&input, "main")?);
        writeln!(generated, "mod {name} {{{code}}}")?;
    }
    generated.push_str(r#"
static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
fn main() {
 for failing in [false,false,true] {
 let mut registry=hgl_describe::Registry::new();
 let description=if failing {failure::register(&mut registry).unwrap();failure::main(&registry).unwrap()}
 else {success::register(&mut registry).unwrap();success::main(&registry).unwrap()};
 let mut store=hgl_store::Store::new();store.global_state().provision();
 let mut built=hgl_describe::instantiate_complete(&description,&registry,&mut store).unwrap();
 let run=hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{
 start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)});
 if failing {let hgl_kernel::EngineError::Node(error)=run.unwrap_err() else {panic!("provider error lost")};
 assert_eq!(error.code,Some("native.generic.failure"));}
 else {run.unwrap();
 for (key,want) in [("replacement","replacement"),("empty","")] {
 let handle=store.global_state().bind::<String>(key).unwrap();assert_eq!(store.global_state().get(handle).unwrap(),want);}
 let equal=store.global_state().bind::<bool>("equal").unwrap();assert!(store.global_state().get(equal).unwrap());}
 let text=store.global_state().bind::<String>("text").unwrap();assert_eq!(store.global_state().get(text).unwrap(),"retained");
 }
 assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),13);
}
"#);
    execute(&generated)
}

#[test]
fn standalone_cold_native_recipes_execute_once_and_descriptions_retain_aliases()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"
fn consume(const first:Token,const second:Token,const text:str,const numbers:list<i64>) {inject global_state,alarm
start {schedule(alarm,0us)}
when {set(global_state,"first",token_text(first))
set(global_state,"second",token_text(second))
set(global_state,"text",text)
set(global_state,"number",numbers[0])}}
const fn wrap(text:str)->Token {return token(text)}
export fn main() {
var current=wrap("retained")
let saved=current
if true {current=token("replacement")} else {current=token("failure")}
let unused=token("")
consume(first:saved,second:current,text:token_text(saved),numbers:[7])
}
"#;
    let mut generated = String::new();
    for (name, body) in [
        ("success", body.to_owned()),
        ("failure", body.replace("replacement", "failure")),
        ("ordinary_failure", body.replace("let unused=token(\"\")", "let values:list<i64> =[]\nlet broken=values[0]\nlet alias=broken\nlet unused=token(\"\")")),
    ] {
        let mut input = sources(&body)?;
        let (_, descriptor) = input
            .iter_mut()
            .find(|(name, _)| name.ends_with(".hgl-native"))
            .ok_or("descriptor missing")?;
        *descriptor = descriptor
            .replace("->Token symbol", "->Token throws symbol")
            .replace(
                "\"atomic_values.rs\"",
                "\"../../crates/hgl-compiler/tests/fixtures/native_generic_provider.rs\"",
            );
        let plan = hgl_program::compile(&input, "main")?;
        writeln!(
            generated,
            "mod {name} {{{}}}",
            hgl_program::emit_rust(&plan)
        )?;
    }
    generated.push_str(r#"
static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
fn main(){
let mut registry=hgl_describe::Registry::new();success::register(&mut registry).unwrap();
assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),0);
for invocation in 1..=2 {
let graph=success::main(&registry).unwrap();
assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),3*invocation);
for description in [&graph,&graph.clone()] {
let mut store=hgl_store::Store::new();store.global_state().provision();
let mut built=hgl_describe::instantiate_complete(description,&registry,&mut store).unwrap();
hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}).unwrap();
for (key,want) in [("first","retained"),("second","replacement"),("text","retained")] {
let key=store.global_state().bind::<String>(key).unwrap();assert_eq!(store.global_state().get(key).unwrap(),want);}
}
assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),3*invocation);
let mut corrupt=graph.clone();
let hgl_types::ScalarValue::Bytes(bytes)=&mut corrupt.nodes[0].scalars[0].1 else {panic!("native carrier")};bytes[0]^=1;
let mut store=hgl_store::Store::new();store.global_state().provision();
assert!(format!("{:?}",hgl_describe::instantiate_complete(&corrupt,&registry,&mut store).err()).contains("carrier identity mismatch"));
}
let mut registry=hgl_describe::Registry::new();failure::register(&mut registry).unwrap();
assert!(format!("{:?}",failure::main(&registry).unwrap_err()).contains("native.generic.failure"));
assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),8);
let mut registry=hgl_describe::Registry::new();ordinary_failure::register(&mut registry).unwrap();
assert!(format!("{:?}",ordinary_failure::main(&registry).unwrap_err()).contains("ordinary list index out of bounds"));
assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),10);
}
"#);
    execute(&generated)
}

#[test]
fn standalone_cold_native_keeps_phase_and_topology_checks() -> Result<(), String> {
    let body = "export fn main(){if token(\"a\")==token(\"b\") {let ignored=1}}";
    let error = hgl_program::compile(&sources(body)?, "main")
        .err()
        .ok_or("provider-dependent topology admitted")?;
    assert!(
        error.contains("provider-dependent wiring topology"),
        "{error}"
    );
    for body in [
        "export fn main(){let ignored=token(\"cold\")}",
        "const fn wrap(text:str)->Token{return token(text)}\nexport fn main(){let ignored=wrap(\"cold\")}",
        "export fn main(){if true {let ignored=token(\"cold\")}}",
        "const fn wrap(text:str)->Token{return token(text)}\nexport fn main(){if true {let ignored=wrap(\"cold\")}}",
        "const fn wrap(text:str)->Token{return token(text)}\nfn source()->i64 {inject alarm\nstart{schedule(alarm,0us)}\nwhen{return 1}}\nexport fn main(){source()\nlet ignored=wrap(\"cold\")}",
    ] {
        let mut input = sources(body)?;
        for (name, text) in &mut input {
            if name.ends_with(".hgl-native") {
                *text = text.replace(
                    "phases {setup,cold,evaluation}",
                    "phases {setup,evaluation}",
                );
            }
        }
        let error = hgl_program::compile(&input, "main")
            .err()
            .ok_or("evaluation-only native admitted in cold phase")?;
        assert!(
            error.contains("native helper is unavailable in cold"),
            "{error}"
        );
        for (name, text) in &mut input {
            if name.ends_with(".hgl-native") {
                *text = text.replace("phases {setup,evaluation}", "phases {cold}");
            }
        }
        hgl_program::compile(&input, "main")?;
    }
    Ok(())
}

#[test]
fn native_wiring_context_preserves_actual_node_invocation_phases() -> Result<(), String> {
    let prefix = "const fn wrap(value:str)->Token{return token(value)}\nfn source()->str {inject alarm\nstart {schedule(alarm,0us)}\nwhen{return \"x\"}}\n";
    for body in [
        "fn consume(value:str)->Token {when{return wrap(value)}}\nexport fn main(){consume(source())}",
        "export fn main(){wrap(source())}",
    ] {
        let mut input = sources(&format!("{prefix}{body}"))?;
        for (name, text) in &mut input {
            if name.ends_with(".hgl-native") {
                *text = text.replace("phases {setup,cold,evaluation}", "phases {evaluation}");
            }
        }
        hgl_program::compile(&input, "main")?;
        for (name, text) in &mut input {
            if name.ends_with(".hgl-native") {
                *text = text.replace("phases {evaluation}", "phases {cold}");
            }
        }
        let error = hgl_program::compile(&input, "main")
            .err()
            .ok_or("cold-only native admitted in evaluation")?;
        assert!(
            error.contains("native helper is unavailable in evaluation"),
            "{error}"
        );
    }
    Ok(())
}

#[test]
fn standalone_cold_aggregate_and_any_descriptions_preserve_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let mut generated = String::new();
    let mut checks = String::new();
    for (name, ty, initial, replacement, calls, failure_calls) in cold_aggregate_cases() {
        for failure in [false, true] {
            let name = format!("{name}_{}", if failure { "failure" } else { "success" });
            let replacement = if failure {
                replacement.replace("replacement", "failure")
            } else {
                replacement.into()
            };
            let body = format!(
                r#"
struct Pair {{first:Token
second:Token}}
struct State {{items:list<Pair>}}
fn consume(const first:{ty},const second:{ty}) {{inject global_state,alarm
start {{schedule(alarm,0us)}}
when {{set(global_state,"first",any(first))
set(global_state,"second",any(second))}}}}
export fn main() {{var current:{ty} ={initial}
let retained=current
current={replacement}
consume(first:retained,second:current)}}
"#
            );
            let body = if name.starts_with("recursive_") {
                body.replace(&format!("current={replacement}"), &format!("current.items[0].first={replacement}\npush(current.items,Pair(first:token(\"tail\"),second:token(\"\")))"))
            } else {
                body
            };
            let mut input = sources(&body)?;
            let (_, descriptor) = input
                .iter_mut()
                .find(|(name, _)| name.ends_with(".hgl-native"))
                .ok_or("descriptor missing")?;
            *descriptor = descriptor
                .replace("->Token symbol", "->Token throws symbol")
                .replace(
                    "\"atomic_values.rs\"",
                    "\"../../crates/hgl-compiler/tests/fixtures/native_generic_provider.rs\"",
                )
                .replace(",serialization", "");
            let plan = hgl_program::compile(&input, "main")?;
            let marker = "hgl_store::Any";
            let capture = "value.capture::<hgl_semantics::ir::Value>().map_err(|e|e.message)?";
            writeln!(
                generated,
                "mod {name} {{{} pub fn inspect(store:&mut hgl_store::Store)->Result<(),String>{{for (key,want) in [(\"first\",b\"retained\".as_slice()),(\"second\",b\"replacement\".as_slice())] {{let key=store.global_state().bind::<{marker}>(key).map_err(|e|e.message)?;let value=store.global_state().get(key).map_err(|e|e.message)?;let captured={capture};let mut found=false;hgl_semantics::ir_visit::value(&captured,&mut |v|{{if let hgl_semantics::ir::Kind::Opaque(value)=&v.kind {{found|=value.data()==want;}}true}});assert!(found,\"retained native payload changed\");}}Ok(())}}}}",
                hgl_program::emit_rust(&plan)
            )?;
            if failure {
                writeln!(
                    checks,
                    "{{let mut registry=hgl_describe::Registry::new();{name}::register(&mut registry).unwrap();let before=CALLS.load(std::sync::atomic::Ordering::Relaxed);assert!(format!(\"{{:?}}\",{name}::main(&registry).unwrap_err()).contains(\"native.generic.failure\"));assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed)-before,{failure_calls});}}"
                )?;
            } else {
                writeln!(
                    checks,
                    "{{let mut registry=hgl_describe::Registry::new();{name}::register(&mut registry).unwrap();let before=CALLS.load(std::sync::atomic::Ordering::Relaxed);let graph={name}::main(&registry).unwrap();assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed)-before,{calls});for registry in [&registry,&registry.clone()] {{for graph in [&graph,&graph.clone()] {{let mut store=hgl_store::Store::new();store.global_state().provision();let mut built=hgl_describe::instantiate_complete(graph,registry,&mut store).unwrap();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}}).unwrap();{name}::inspect(&mut store).unwrap();}}}}assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed)-before,{calls});}}"
                )?;
            }
        }
    }
    writeln!(
        generated,
        "static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);fn main(){{{checks}}}"
    )?;
    execute(&generated)
}

type ColdAggregateCase = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    usize,
    usize,
);
fn cold_aggregate_cases() -> [ColdAggregateCase; 6] {
    [
        (
            "recursive",
            "State",
            "State(items:[Pair(first:token(\"retained\"),second:token(\"\"))])",
            "token(\"replacement\")",
            5,
            3,
        ),
        (
            "pair",
            "Pair",
            "Pair(first:token(\"retained\"),second:token(\"\"))",
            "Pair(first:token(\"replacement\"),second:token(\"later\"))",
            4,
            3,
        ),
        (
            "list",
            "list<Token>",
            "[token(\"retained\"),token(\"\") ]",
            "[token(\"replacement\") ]",
            3,
            3,
        ),
        (
            "tuple",
            "tuple<Token,str>",
            "(token(\"retained\"),\"first\")",
            "(token(\"replacement\"),\"second\")",
            2,
            2,
        ),
        (
            "boxed",
            "any",
            "any(token(\"retained\"))",
            "any(token(\"replacement\"))",
            2,
            2,
        ),
        (
            "boxed_pair",
            "any",
            "any(Pair(first:token(\"retained\"),second:token(\"\")))",
            "any(Pair(first:token(\"replacement\"),second:token(\"later\")))",
            4,
            3,
        ),
    ]
}

#[test]
fn malformed_cold_aggregate_carriers_return_build_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let body = r#"
fn consume(const value:tuple<i64,bool,str,any,list<i64>>) {inject global_state,alarm
start {schedule(alarm,0us)}
when {set(global_state,"retained",any(value))}}
export fn main(){consume(value:(42,true,"text",any(token("owner")),[1,2]))}
"#;
    let image = std::env::temp_dir().join(format!(
        "hgl-cold-validation-provider-{}.rs",
        std::process::id()
    ));
    let provider = fs::read_to_string(root()?.join("native/stdlib/atomic_values.rs"))?.replace(
        "destination.write(data);",
        "crate::COPIES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);destination.write(data);",
    );
    fs::write(&image, provider)?;
    let mut input = sources(body)?;
    for (name, text) in &mut input {
        if name.ends_with(".hgl-native") {
            *text = text.replace(
                "\"atomic_values.rs\"",
                &format!("{:?}", image.display().to_string()),
            );
        }
    }
    let plan = hgl_program::compile(&input, "main")?;
    let generated = format!(
        r#"
{}
static COPIES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static UNCHECKED:hgl_types::AnyPlan=hgl_types::AnyPlan{{execution:&hgl_store::any_values::operations::EXECUTION,capture:None,validate:None,identity:std::any::TypeId::of::<i64>,operation:hgl_types::AnyOperation::Integer,copy:|_,_|panic!("unchecked cold copy"),capabilities:[true;3]}};
fn frame(bytes:&[u8])->Vec<u8>{{let mut out=(bytes.len() as u64).to_le_bytes().to_vec();out.extend(bytes);out}}
fn main(){{
let mut registry=hgl_describe::Registry::new();register(&mut registry).unwrap();
let graph=main_graph(&registry).unwrap();
let copies=COPIES.load(std::sync::atomic::Ordering::Relaxed);
let (node,scalar)=graph.nodes.iter().enumerate().find_map(|(node,n)|n.scalars.iter().position(|(key,_)|key.starts_with("\0cold:")).map(|scalar|(node,scalar))).unwrap();
let hgl_types::ScalarValue::Bytes(original)=&graph.nodes[node].scalars[scalar].1 else {{panic!("carrier")}};
let start=original.iter().position(|b|*b<32).unwrap();
let count=u64::from_le_bytes(original[start..start+8].try_into().unwrap()) as usize;
let payload=start+8+8*count;
let mut fields=Vec::new();let mut rest=&original[payload..];
while !rest.is_empty(){{let width=u64::from_le_bytes(rest[..8].try_into().unwrap()) as usize;fields.push(rest[8..8+width].to_vec());rest=&rest[8+width..];}}
let mut bad=vec![Vec::new(),vec![0;7],u64::MAX.to_le_bytes().to_vec()];
for (index,value) in [(0,vec![0;7]),(1,Vec::new()),(1,vec![2]),(2,vec![255]),(3,u64::MAX.to_le_bytes().to_vec()),(3,vec![0;9]),(3,[1u64.to_le_bytes().as_slice(),&[255]].concat()),(4,vec![0;7]),(4,u64::MAX.to_le_bytes().to_vec())] {{let mut changed=fields.clone();changed[index]=value;bad.push(changed.iter().flat_map(|v|frame(v)).collect());}}
bad.push(fields[..4].iter().flat_map(|v|frame(v)).collect());
let mut extra=original[payload..].to_vec();extra.extend(frame(&[]));bad.push(extra);
let mut carriers:Vec<Vec<u8>>=bad.into_iter().map(|bytes|{{let mut carrier=original[..payload].to_vec();carrier.extend(bytes);carrier}}).collect();
for (offset,value) in [(start,u64::MAX),(start+8,0),(start+8,u64::MAX)] {{let mut carrier=original.clone();carrier[offset..offset+8].copy_from_slice(&value.to_le_bytes());carriers.push(carrier);}}
let mut missing=original.clone();missing[start+8..start+16].copy_from_slice(&registry.retain_descriptor(&UNCHECKED).unwrap().to_le_bytes());carriers.push(missing);
assert_eq!(carriers.len(),18);
for carrier in carriers {{let mut corrupt=graph.clone();corrupt.nodes[node].scalars[scalar].1=hgl_types::ScalarValue::Bytes(carrier);
let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let mut store=hgl_store::Store::new();store.global_state().provision();hgl_describe::instantiate_complete(&corrupt,&registry,&mut store)}}));
assert!(outcome.is_ok(),"malformed cold payload panicked");assert!(matches!(outcome.unwrap(),Err(hgl_describe::BuildError::InvalidNodeType{{..}})),"malformed cold payload was admitted");assert_eq!(COPIES.load(std::sync::atomic::Ordering::Relaxed),copies,"malformed carrier copied an owner");}}
for description in [&graph,&graph.clone()] {{let mut store=hgl_store::Store::new();store.global_state().provision();let mut built=hgl_describe::instantiate_complete(description,&registry,&mut store).unwrap();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}}).unwrap();let key=store.global_state().bind::<hgl_store::Any>("retained").unwrap();let retained=store.global_state().get(key).unwrap().capture::<hgl_semantics::ir::Value>().unwrap();let mut found=false;hgl_semantics::ir_visit::value(&retained,&mut |v|{{if let hgl_semantics::ir::Kind::Opaque(value)=&v.kind {{found|=value.data()==b"owner";}}true}});assert!(found);}}
assert!(COPIES.load(std::sync::atomic::Ordering::Relaxed)>copies,"valid cold roundtrip did not retain owners");
}}
"#,
        hgl_program::emit_rust(&plan).replace("pub fn main(", "pub fn main_graph(")
    );
    let result = execute(&generated);
    fs::remove_file(image)?;
    result
}

#[test]
fn hgl_gen_helper_executes_with_a_compatible_rust_target_name()
-> Result<(), Box<dyn std::error::Error>> {
    let mut input = sources(
        r#"
use hgraph.std::{gen}
test {
 fn echo(value:Token)->Token {when {return value}}
 test keyword_name {assert eval(echo,[gen("owner")]) == [gen("owner")]}
}
"#,
    )?;
    for (name, text) in &mut input {
        if name.ends_with("atomic_values_fixture.hgl") {
            text.push_str("\nnative const fn gen(text:str)->Token\n");
        } else if name.ends_with(".hgl-native") {
            let helper = text
                .lines()
                .find(|line| line.starts_with("value token("))
                .ok_or("token descriptor missing")?
                .replace("value token(", "value gen(");
            text.push_str(&helper);
            text.push('\n');
        }
    }
    run_sources(&input)
}

#[test]
fn complete_native_tuple_results_publish_from_reserved_slots()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
test {
 fn complete(value:Token,text:bytes)->tuple<Token,bytes,i64> {when {
  let retained=value
  let held_text=text
  return (retained,held_text,1)
 }}
 test complete_native_tuple {
  let owner=token("owner")
  let empty=token("")
  assert eval(complete,value:[owner,empty,_,owner],text:[bytes([0,255]),bytes(),_,bytes([0,255])]) == [(owner,bytes([0,255]),1),(empty,bytes(),1),_,(owner,bytes([0,255]),1)]
 }
}
"#)
}

#[test]
fn complete_tuple_capacity_failure_preserves_every_output_child()
-> Result<(), Box<dyn std::error::Error>> {
    let input = sources(
        r"test {
fn pair(value:bytes,other:bytes)->tuple<bytes,bytes> {when {
let first=value
let second=other
return (first,second)}}
test capacity {eval(pair,value:[bytes([1,2])],other:[bytes([3,4])])}
}",
    )?;
    let suite = hgl_program::compile_module_suite(&input)?;
    let generated = hgl_program::emit_tests(&suite)
        .replace("fn main() {", "fn authored_main() {")
        .replace("hgl_kernel::run_simulation(", "crate::capacity_failure_simulation(")
        .replace("store.prepare_collection_inputs();", "store.prepare_collection_inputs();let node=graph.nodes.iter().position(|node|node.implementation.contains(\"::pair#\")).unwrap();crate::shrink_destination(&mut store,built.outputs[node].unwrap());");
    let generated = format!(
        r#"{generated}
static ROOT:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static FAILURES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
fn shrink_destination(store:&mut hgl_store::Store,root:hgl_store::OutputId){{
 ROOT.store(root.0 as usize,std::sync::atomic::Ordering::Relaxed);
 let storage=store.prepared();let children=storage.bindings.output(root).fixed.clone();
 let first=storage.bindings.output(children[0]).slot as usize;
 let second=storage.bindings.output(children[1]).slot as usize;
 let columns=<Vec<u8> as hgl_store::columns::Column>::column_mut(storage.columns);
 let mut retained=Vec::with_capacity(2);retained.push(9);columns[first]=retained;columns[second]=Vec::new();
}}
fn capacity_failure_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{{
 let outcome=hgl_kernel::run_simulation(graph,store,config);
 assert!(matches!(&outcome,Err(hgl_kernel::EngineError::Node(error)) if error.message.contains("prepared scalar capacity exceeded")));
 let root=hgl_store::OutputId(ROOT.load(std::sync::atomic::Ordering::Relaxed) as u32);
 let storage=store.prepared();let children=&storage.bindings.output(root).fixed;
 let first=storage.bindings.output(children[0]);let second=storage.bindings.output(children[1]);
 let columns=<Vec<u8> as hgl_store::columns::Column>::column(storage.columns);
 assert_eq!(columns[first.slot as usize].as_slice(),&[9]);assert!(columns[second.slot as usize].is_empty());
 assert_eq!(first.modified_at,hgl_types::EngineTime::NEVER);assert_eq!(second.modified_at,hgl_types::EngineTime::NEVER);
 FAILURES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);outcome
}}
fn main(){{let error=test0::test().unwrap_err();assert!(error.contains("prepared scalar capacity exceeded"));assert_eq!(FAILURES.load(std::sync::atomic::Ordering::Relaxed),1);}}
"#
    );
    execute(&generated)
}

#[test]
fn temporal_cold_aggregate_carriers_validate_before_retaining_native_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"
use hgraph.std::{cold_zone,cold_time,cold_datetime}
fn consume(const value:tuple<any,timezone,zoned_time,zoned_datetime,time>) {inject global_state,alarm
start {schedule(alarm,0us)}
when {set(global_state,"retained",any(value))}}
export fn main(){consume(value:(any(token("owner")),cold_zone(1),cold_time(1),cold_datetime(1),@09:30))}
"#;
    let image = std::env::temp_dir().join(format!(
        "hgl-temporal-cold-provider-{}.rs",
        std::process::id()
    ));
    fs::write(&image,fs::read_to_string(root()?.join("native/stdlib/atomic_values.rs"))?.replace("destination.write(data);","crate::COPIES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);destination.write(data);"))?;
    let mut input = sources(body)?;
    for (name, text) in &mut input {
        if name.ends_with(".hgl-native") {
            *text = text.replace(
                "\"atomic_values.rs\"",
                &format!("{:?}", image.display().to_string()),
            );
        }
    }
    let mut provider = fs::read_to_string(&image)?;
    provider.push_str(r#"
pub fn cold_zone(_:i64)->hgl_types::ZoneId{hgl_types::ZoneId::from_validated_name("US/Eastern".into())}
pub fn cold_time(_:i64)->hgl_types::ZonedTime{hgl_types::ZonedTime::from_validated_parts(hgl_types::Time(34_200_000_000),cold_zone(1))}
pub fn cold_datetime(_:i64)->hgl_types::ZonedDateTime{hgl_types::ZonedDateTime::from_validated_parts(hgl_types::calendar::datetime("2026-07-01T16:00Z").unwrap(),cold_zone(1),-14_400)}
"#);
    fs::write(&image, provider)?;
    for (name, text) in &mut input {
        if name.ends_with("atomic_values_fixture.hgl") {
            text.push_str("\nexport native const fn cold_zone(value:i64)->timezone\nexport native const fn cold_time(value:i64)->zoned_time\nexport native const fn cold_datetime(value:i64)->zoned_datetime\n");
        }
        if name.ends_with(".hgl-native") {
            for (helper, result) in [
                ("cold_zone", "timezone"),
                ("cold_time", "zoned_time"),
                ("cold_datetime", "zoned_datetime"),
            ] {
                writeln!(
                    text,
                    "value {helper}(value:i64)->{result} symbol {helper:?} ownership owned phases {{cold}} effects {{allocation}}"
                )?;
            }
        }
    }
    let plan = hgl_program::compile(&input, "main")?;
    let generated = format!(
        r#"
{}
static COPIES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
fn frame(bytes:&[u8])->Vec<u8>{{let mut out=(bytes.len() as u64).to_le_bytes().to_vec();out.extend(bytes);out}}
fn main(){{
 let mut registry=hgl_describe::Registry::new();register(&mut registry).unwrap();let graph=main_graph(&registry).unwrap();
 let copies=COPIES.load(std::sync::atomic::Ordering::Relaxed);
 let(node,scalar)=graph.nodes.iter().enumerate().find_map(|(node,n)|n.scalars.iter().position(|(key,_)|key.starts_with("\0cold:")).map(|scalar|(node,scalar))).unwrap();
 let hgl_types::ScalarValue::Bytes(original)=&graph.nodes[node].scalars[scalar].1 else{{panic!("carrier")}};
 let start=original.iter().position(|b|*b<32).unwrap();let count=u64::from_le_bytes(original[start..start+8].try_into().unwrap()) as usize;let payload=start+8+8*count;
 let mut fields=Vec::new();let mut rest=&original[payload..];while !rest.is_empty(){{let width=u64::from_le_bytes(rest[..8].try_into().unwrap()) as usize;fields.push(rest[8..8+width].to_vec());rest=&rest[8+width..];}}
 let mut bad=vec![(1,b"Missing/Zone".to_vec()),(1,b"us/eastern".to_vec()),(1,Vec::new())];
 for time in [-1i64,86_400_000_000,i64::MAX]{{let mut zoned=time.to_le_bytes().to_vec();zoned.extend(b"US/Eastern");bad.push((2,zoned));bad.push((4,time.to_le_bytes().to_vec()));}}
 let mut wrong_name=fields[2][..8].to_vec();wrong_name.extend(b"Missing/Zone");bad.push((2,wrong_name));
 for offset in [0i64,-18_000,i64::MAX]{{let mut zoned=fields[3].clone();zoned[8..16].copy_from_slice(&offset.to_le_bytes());bad.push((3,zoned));}}
 let mut outside=fields[3].clone();outside[..8].copy_from_slice(&i64::MAX.to_le_bytes());bad.push((3,outside));
 let mut wrong_zone=fields[3][..16].to_vec();wrong_zone.extend(b"us/eastern");bad.push((3,wrong_zone));
 assert_eq!(bad.len(),15);
 for(index,value)in bad{{let mut changed=fields.clone();changed[index]=value;let mut carrier=original[..payload].to_vec();carrier.extend(changed.iter().flat_map(|v|frame(v)));let mut corrupt=graph.clone();corrupt.nodes[node].scalars[scalar].1=hgl_types::ScalarValue::Bytes(carrier);
 let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let mut store=hgl_store::Store::new();store.global_state().provision();hgl_describe::instantiate_complete(&corrupt,&registry,&mut store)}}));
 assert!(outcome.is_ok(),"malformed temporal carrier panicked");assert!(matches!(outcome.unwrap(),Err(hgl_describe::BuildError::InvalidNodeType{{..}})),"malformed temporal carrier admitted");assert_eq!(COPIES.load(std::sync::atomic::Ordering::Relaxed),copies,"temporal validation ran after native ownership effects");}}
 for description in [&graph,&graph.clone()]{{let mut store=hgl_store::Store::new();store.global_state().provision();let mut built=hgl_describe::instantiate_complete(description,&registry,&mut store).unwrap();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}}).unwrap();let key=store.global_state().bind::<hgl_store::Any>("retained").unwrap();let value=store.global_state().get(key).unwrap().capture::<hgl_semantics::ir::Value>().unwrap();let mut found=[false;4];hgl_semantics::ir_visit::value(&value,&mut|v|{{match &v.kind{{hgl_semantics::ir::Kind::Opaque(v)=>found[0]|=v.data()==b"owner",hgl_semantics::ir::Kind::Literal(hgl_source::Literal::TimeZone(v))=>found[1]|=v.as_str()=="US/Eastern",hgl_semantics::ir::Kind::Literal(hgl_source::Literal::ZonedTime(v))=>found[2]|=v.zone().as_str()=="US/Eastern"&&v.time().0==34_200_000_000,hgl_semantics::ir::Kind::Literal(hgl_source::Literal::ZonedDateTime(v))=>found[3]|=v.zone().as_str()=="US/Eastern"&&v.offset_seconds()==-14_400,_=>()}}true}});assert_eq!(found,[true;4]);}}
 assert!(COPIES.load(std::sync::atomic::Ordering::Relaxed)>copies);
}}
"#,
        hgl_program::emit_rust(&plan).replace("pub fn main(", "pub fn main_graph(")
    );
    let result = execute(&generated);
    fs::remove_file(image)?;
    result
}

#[test]
fn partial_native_order_agrees_in_cold_setup_and_generated_ticks()
-> Result<(), Box<dyn std::error::Error>> {
    let mut input = sources(
        r#"
fn less(value:Token,const other:Token)->bool {when {return value<other}}
fn le(value:Token,const other:Token)->bool {when {return value<=other}}
fn greater(value:Token,const other:Token)->bool {when {return value>other}}
fn ge(value:Token,const other:Token)->bool {when {return value>=other}}
test partial_order {
let a=token("a")
let b=token("b")
assert !(a<b)
assert !(a<=b)
assert !(a>b)
assert !(a>=b)
assert !(b<a)
assert !(b<=a)
assert !(b>a)
assert !(b>=a)
assert a==a
assert a!=b
assert a<=a
assert a>=a
assert !(any(a)<any(b))
assert !(any(a)<=any(b))
assert !(any(a)>any(b))
assert !(any(a)>=any(b))
assert eval(less,value:[a,_,b],other:b)==[false,_,false]
assert eval(le,value:[a,_,b],other:b)==[false,_,true]
assert eval(greater,value:[a,_,b],other:b)==[false,_,false]
assert eval(ge,value:[a,_,b],other:b)==[false,_,true]
}
"#,
    )?;
    let image = std::env::temp_dir().join(format!(
        "hgl-partial-order-provider-{}.rs",
        std::process::id()
    ));
    fs::write(
        &image,
        fs::read_to_string(root()?.join("native/stdlib/atomic_values.rs"))?.replace(
            "Ok(Some(text(lhs).cmp(text(rhs))))",
            "Ok((text(lhs)==text(rhs)).then_some(Ordering::Equal))",
        ),
    )?;
    for (name, text) in &mut input {
        if name.ends_with(".hgl-native") {
            *text = text.replace(
                "\"atomic_values.rs\"",
                &format!("{:?}", image.display().to_string()),
            );
        }
    }
    let result = run_sources(&input);
    fs::remove_file(image)?;
    result
}

#[test]
fn empty_native_aggregate_cold_carriers_include_type_only_provider_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("hgl-type-only-carrier-{}", std::process::id()));
    fs::create_dir_all(&directory)?;
    let image = directory.join("provider.rs");
    let alternate = directory.join("alternate.rs");
    let carrier = directory.join("carrier.bin");
    let compatible_carrier = directory.join("compatible.bin");
    let provider=fs::read_to_string(root()?.join("native/stdlib/atomic_values.rs"))?.replace("if data.len() > capacity {","crate::PREFLIGHTS.fetch_add(1,std::sync::atomic::Ordering::Relaxed);if data.len() > capacity {").replace("destination.write(data);","crate::COPIES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);destination.write(data);");
    fs::write(&image, &provider)?;
    fs::write(
        &alternate,
        provider.replace("native-atomic-values-v1", "native-atomic-values-alternate"),
    )?;
    let plan = type_only_carrier_plan(&image, "native-atomic-values-v1")?;
    let other = type_only_carrier_plan(&alternate, "native-atomic-values-alternate")?;
    let emitted = hgl_program::emit_rust(&plan);
    assert!(emitted.contains("cold_configurations"));
    assert!(emitted.contains("NativeDomain") && emitted.contains("native-atomic-values-v1"));
    let counters = "static COPIES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);static PREFLIGHTS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);";
    for (program, path) in [(&other, &carrier), (&plan, &compatible_carrier)] {
        execute(&format!(
            r"
{} {counters}
fn main(){{let mut registry=hgl_describe::Registry::new();register(&mut registry).unwrap();let graph=main_graph(&registry).unwrap();let bytes=graph.nodes.iter().flat_map(|n|&n.scalars).find_map(|(_,v)|match v{{hgl_types::ScalarValue::Bytes(bytes)=>Some(bytes),_=>None}}).unwrap();std::fs::write({:?},bytes).unwrap();}}
",
            hgl_program::emit_rust(program).replace("pub fn main(", "pub fn main_graph("),
            path.to_string_lossy()
        ))?;
    }
    execute(&format!(
        r#"
{} {counters}
fn main(){{
 let mut registry=hgl_describe::Registry::new();register(&mut registry).unwrap();let graph=main_graph(&registry).unwrap();
 let(node,scalar)=graph.nodes.iter().enumerate().find_map(|(node,n)|n.scalars.iter().position(|(_,v)|matches!(v,hgl_types::ScalarValue::Bytes(_))).map(|scalar|(node,scalar))).unwrap();
 let mut incompatible=graph.clone();incompatible.nodes[node].scalars[scalar].1=hgl_types::ScalarValue::Bytes(std::fs::read({:?}).unwrap());
 let copies=COPIES.load(std::sync::atomic::Ordering::Relaxed);let preflights=PREFLIGHTS.load(std::sync::atomic::Ordering::Relaxed);
 let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let mut store=hgl_store::Store::new();store.global_state().provision();hgl_describe::instantiate_complete(&incompatible,&registry,&mut store)}}));
 assert!(result.is_ok(),"incompatible type-only carrier panicked");assert!(matches!(result.unwrap(),Err(hgl_describe::BuildError::InvalidNodeType{{what,..}}) if what=="cold carrier identity mismatch"),"incompatible type-only provider carrier was admitted");
 assert_eq!(COPIES.load(std::sync::atomic::Ordering::Relaxed),copies);assert_eq!(PREFLIGHTS.load(std::sync::atomic::Ordering::Relaxed),preflights);
 let mut compatible=graph.clone();compatible.nodes[node].scalars[scalar].1=hgl_types::ScalarValue::Bytes(std::fs::read({:?}).unwrap());
 for description in [&graph,&graph.clone(),&compatible]{{let mut store=hgl_store::Store::new();store.global_state().provision();let mut built=hgl_describe::instantiate_complete(description,&registry,&mut store).unwrap();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig{{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}}).unwrap();for(key,want)in [("count",0),("number",2)]{{let handle=store.global_state().bind::<i64>(key).unwrap();assert_eq!(store.global_state().get(handle).unwrap(),want);}}}}
 assert_eq!(COPIES.load(std::sync::atomic::Ordering::Relaxed),copies);assert_eq!(PREFLIGHTS.load(std::sync::atomic::Ordering::Relaxed),preflights);
}}
"#,
        hgl_program::emit_rust(&plan).replace("pub fn main(", "pub fn main_graph("),
        carrier.to_string_lossy(),
        compatible_carrier.to_string_lossy()
    ))?;
    fs::remove_dir_all(directory)?;
    Ok(())
}

fn type_only_carrier_plan(
    image: &std::path::Path,
    fingerprint: &str,
) -> Result<hgl_program::Program, Box<dyn std::error::Error>> {
    let mut input = sources(
        r#"
use carrier_seed::{seed}
fn consume(const keys:list<Token>,const number:i64) {inject global_state,alarm
start {schedule(alarm,0us)}
when {set(global_state,"count",len(keys))
set(global_state,"number",number)}}
export fn main(){let deferred=seed(1)
consume(keys:[],number:deferred)}
"#,
    )?;
    for (name, text) in &mut input {
        if name.ends_with(".hgl-native") {
            *text = text
                .replace(
                    "\"atomic_values.rs\"",
                    &format!("{:?}", image.to_string_lossy()),
                )
                .replace("native-atomic-values-v1", fingerprint);
        }
    }
    input.push((
        "carrier_seed.hgl".into(),
        "module carrier_seed\nexport native const fn seed(value:i64)->i64".into(),
    ));
    input.push((root()?.join("crates/hgl-compiler/tests/fixtures/carrier-seed.hgl-native").to_string_lossy().into_owned(),"native_descriptor 1 target \"hgl-rust-1\"\nmodule carrier_seed\nprovider \"hgl.test.builtin_results\" fingerprint \"builtin-results-v1\" image \"native_builtin_results.rs\"\nvalue seed(value:i64)->i64 symbol \"fixed\" ownership owned phases {cold} effects {}\n".into()));
    Ok(hgl_program::compile(&input, "main")?)
}

#[test]
fn native_dictionary_lookups_use_borrowed_key_hashes_and_preserve_unknown_absence()
-> Result<(), Box<dyn std::error::Error>> {
    run(r#"
    fn lookup(book:atomic<map<Token,i64>>,key:Token)->i64 {when {
        let value=get(book,key)
        if value==null {return 9}
        return value
    }}
    test native_lookup {
        let book=map<Token,i64>(items:[token("a"):7,token("b"):0])
        assert eval(lookup,book:[book,book,book,book],key:[token("b"),token("absent"),token("a"),token("b")])==[0,9,7,0]
        let value=get(book,token("absent"))
        assert value==null
        assert get(book,token("b"),default:8)==0
    }
    "#)
}
