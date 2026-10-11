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
    let suite = hgl_program::compile_module_suite(&sources(body)?)?;
    assert!(
        !suite.tests.is_empty(),
        "native fixture must execute authored tests"
    );
    let generated = hgl_program::emit_tests(&suite);
    let start = "store.prepare_collection_inputs();hgl_kernel::run_simulation(";
    let end = ").map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>";
    let generated = generated.replace(start,"store.prepare_collection_inputs();let (run,allocations)=hgl_alloc_count::count_in(||hgl_kernel::run_simulation(").replace(end,"));if run.is_ok(){assert_eq!(allocations,0,\"native scalar evaluation allocated\");}run.map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>");
    let generated = format!(
        "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;\n{generated}"
    );
    execute(&generated)
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
    fn make(value:str)->Token {when {return token(value)}}
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
        for (_, text) in &mut input {
            *text = text.replace(
                "fn token(text: str) -> Token",
                "fn token(text: str) -> Token throws",
            );
        }
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
