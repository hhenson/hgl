//! Finite abstract atomic shared semantics and actual evaluation allocation checks.
use hgl_program::compile_tests;
fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("abstract.hgl".into(), source.into()),
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
fn shared_abstract_cases_typecheck() {
    let result = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/abstract_atomic_values.hgl"
    )));
    assert!(result.is_ok(), "{result:?}");
}
#[test]
fn shared_abstract_cases_execute_without_tick_allocations() -> Result<(), Box<dyn std::error::Error>>
{
    run_shared(
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/abstract_atomic_values.hgl"),
        true,
    )
}
fn run_shared(source: &str, measure: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-abstract-{}",
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
    eprintln!("abstract measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"abstract first/repeated publication and recording must allocate nothing");
    Ok(cycles)
}
"#);
    }
    code.push_str("struct Provider;\n");
    fs::write(dir.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"abstract-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
fn family_ordinary_and_generic_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source=r#"module hgraph.std part family_smoke
abstract struct Event { label:str }
struct First:Event { items:list<i64> }
struct Second:Event { items:list<i64> }
abstract struct Sub:Event {}
struct Nested:Sub {items:list<i64>}
abstract struct Generic<T> { value:T }
struct Child<T>:Generic<T> { }
test {
 fn snapshot(v:atomic<Event>)->atomic<Event> => pass_through(v)
 fn constructed(tick:i64)->atomic<Event> { when { return Second(label:"new",items:[7,8]) } }
 test family_smoke {
  let a:Event=First(label:"same",items:[1])
  let b:Event=Second(label:"same",items:[1])
  assert a!=b
  assert eval(constructed,[1,2,3])==[Second(label:"new",items:[7,8]),Second(label:"new",items:[7,8]),Second(label:"new",items:[7,8])]
  let generic:Generic<str>=Child<str>(value:"yes")
  let inferred:Generic<str>=Child(value:"yes")
  let empty:Generic<list<i64>>=Child(value:[])
  var child=Nested(label:"old",items:[1])
  let narrow:Sub=child
  let ancestor:Event=narrow
  push(child.items,2)
  assert eval(snapshot,[ancestor,_,ancestor])==[Nested(label:"old",items:[1]),_,Nested(label:"old",items:[1])]
  assert eval(snapshot,[a,b,_,a]) == [First(label:"same",items:[1]),Second(label:"same",items:[1]),_,First(label:"same",items:[1])]
 }
}"#.replace(">=","> =");
    let result = compile_tests(&sources(&source));
    assert!(result.is_ok(), "{result:?}");
    run_shared(&source, true)
}

#[test]
fn invalid_family_construction_and_widening_are_rejected() {
    for statement in [
        "let x = Event(label:\"no\")",
        "let x:Event = Unrelated(label:\"no\")",
        "let x:Generic<str> = Child<i64>(value:1)",
        "let a:Event = Member(label:\"same\")\nlet b:Other = a",
    ] {
        let source = format!(
            "module hgraph.std part invalid_family\nabstract struct Event {{label:str}}\nabstract struct Other {{label:str}}\nstruct Member:Event {{}}\nstruct Unrelated {{label:str}}\nabstract struct Generic<T> {{value:T}}\nstruct Child<T>:Generic<T> {{}}\ntest {{test invalid {{{statement}\nassert true}}}}"
        );
        assert!(compile_tests(&sources(&source)).is_err(), "{statement}");
    }
}

#[test]
fn concrete_inheritance_bases_are_rejected_through_generic_and_imported_names() {
    for (base, child, construction) in [
        (
            "struct Base {value:i64}",
            "struct Child:base::Base {extra:i64}",
            "Child(value:1,extra:2)",
        ),
        (
            "struct Base<T> {value:T}",
            "struct Child<T>:base::Base<T> {extra:i64}",
            "Child<i64>(value:1,extra:2)",
        ),
        (
            "struct Base<T> {value:T}",
            "abstract struct Child<T>:base::Base<T> {extra:i64}\nstruct Member:Child<i64>{}",
            "Member(value:1,extra:2)",
        ),
    ] {
        let imported = format!("module ancestor\nexport {base}");
        let root = format!(
            "module consumer\nuse ancestor as base\n{child}\nexport fn main() {{let value={construction}}}"
        );
        let error = hgl_program::compile(
            &[("root.hgl".into(), root), ("base.hgl".into(), imported)],
            "main",
        )
        .expect_err("concrete bases must never enter a family");
        assert!(
            error.contains("inheritance requires an abstract base"),
            "{error}"
        );
    }
}
