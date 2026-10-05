//! Complete ordinary collection semantics and actual prepared evaluation allocation checks.
use hgl_program::compile_tests;
fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("atomic_collections.hgl".into(), source.into()),
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
fn shared_atomic_collections_cases_typecheck() {
    let result = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/atomic_set_map_values.hgl"
    )));
    assert!(result.is_ok(), "{result:?}");
}
#[test]
fn shared_atomic_collections_cases_execute_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/atomic_set_map_values.hgl"),
        true,
    )
}
fn run_shared(source: &str, measure: bool) -> Result<(), Box<dyn std::error::Error>> {
    run_program(source, measure, "", None, &[])
}
fn run_program(
    source: &str,
    measure: bool,
    provider: &str,
    failure: Option<&str>,
    marks: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, process::Command, time::SystemTime};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-atomic_collections-{}",
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
    eprintln!("atomic_collections measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"atomic_collections first/repeated publication and recording must allocate nothing");
    Ok(cycles)
}
"#);
    }
    code.push_str("struct Provider;\n");
    code.push_str(provider);
    fs::write(dir.join("src/main.rs"), code)?;
    manifest(&root, &dir)?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            .args(profile)
            .current_dir(&dir)
            .output()?;
        assert!(
            output.status.success() == failure.is_none(),
            "{}\n{}\n{}",
            dir.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(message) = failure {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(message),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let actual = stdout
            .lines()
            .filter(|line| line.starts_with("MARK"))
            .collect::<Vec<_>>();
        assert_eq!(actual, marks);
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn collection_constructor_rejects_invalid_forms_and_known_duplicates() {
    for expr in [
        "set(items: [1])",
        "set<i64>()",
        "set<i64>(other: [])",
        "set<i64>(items: [1, 1])",
        "set<f64>(items: [0.0, -0.0])",
        "map<str,i64>(items: [\"same\": 1, \"same\": 2])",
        "map<str,i64>(items: [1: 2])",
        "map<str,i64>(items: [\"key\": true])",
        "map<str,i64>(items: [1,2])",
        "set<list<i64>>(items: [])",
    ] {
        let source = format!("module invalid\ntest reject {{let value={expr}\nassert true}}");
        assert!(compile_tests(&sources(&source)).is_err(), "{expr}");
    }
}
#[test]
fn nested_collections_compare_unordered() -> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"module hgraph.std part ordinary_collections
    const fn nested(key:str)->map<str,set<i64>> { return map<str,set<i64>>(items:[key:set<i64>(items:[1,2])]) }
    test { test unordered {
        let first=nested("row")
        let other=map<str,set<i64>>(items:["row":set<i64>(items:[2,1])])
        assert first==other
        assert first!=map<str,set<i64>>(items:[])
    }}"#,
        true,
    )
}

#[test]
fn hook_constructor_checks_duplicate_before_value_and_retains_written_order()
-> Result<(), Box<dyn std::error::Error>> {
    let provider = r#"mod native {pub fn mark_i64(v:i64)->hgl_types::NodeResult<i64> {println!("MARK{v}");Ok(v)}}"#;
    for (second, failure, marks) in [
        ("tick+1", None, vec!["MARK1", "MARK10", "MARK2", "MARK20"]),
        (
            "tick",
            Some("duplicate ordinary collection"),
            vec!["MARK1", "MARK10", "MARK1"],
        ),
    ] {
        let source = format!(
            r"module hot_collections
native const fn mark(value:i64)->i64 throws
native const fn mark(value:i64)->i64 throws {{}}
fn build(tick:i64)->atomic<map<i64,i64>> {{when {{return map<i64,i64>(items:[mark(tick):mark(10),mark({second}):mark(20)])}}}}
test order {{assert eval(build,[1])==[map<i64,i64>(items:[2:20,1:10])]}}
"
        );
        run_program(&source, false, provider, failure, &marks)?;
    }
    Ok(())
}

fn manifest(
    root: &std::path::Path,
    dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs};
    let mut manifest = String::from(
        "[package]\nname=\"atomic_collections-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    Ok(())
}

#[test]
fn shared_complete_containers_retain_composite_keys() -> Result<(), Box<dyn std::error::Error>> {
    let shared =
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/composite_key_values.hgl");
    let header = shared
        .split("    test ")
        .next()
        .ok_or("missing declarations")?;
    let (_, case) = shared
        .split_once("    test complete_containers_own_keys_before_source_bindings_change")
        .ok_or("missing ordinary container case")?;
    run_shared(
        &format!(
            "{header}    test complete_containers_own_keys_before_source_bindings_change{case}"
        ),
        true,
    )
}
#[test]
fn ordinary_composite_duplicates_and_wrong_nominals_are_rejected() {
    for expression in [
        "set<Key>(items:[Key(n:1),Key(n:1,present:null)])",
        "map<Key,i64>(items:[Key(n:1):1,Key(n:1):2])",
        "set<tuple<f64,str>>(items:[(0.0,\"a\"),(-0.0,\"a\")])",
        "set<Key>(items:[Other(n:1)])",
        "set<tuple<list<i64>>>(items:[])",
    ] {
        let source = format!(
            "module wrong\nstruct Key {{n:i64\npresent:i64=null}}\nstruct Other {{n:i64}}\ntest bad {{let value={expression}\nassert true}}"
        );
        assert!(compile_tests(&sources(&source)).is_err(), "{expression}");
    }
}

#[test]
fn complete_optional_keys_preserve_presence_and_runtime_nan_stops_value()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r"module hgraph.std part optional_ordinary_keys
struct Key {number:i64
flag:bool=null}
test { fn pass(v:atomic<set<Key>>)->atomic<set<Key>> =>pass_through(v)
test presence {
 let unset=Key(number:1)
 let present=Key(number:1,flag:false)
 let both=set<Key>(items:[unset,present])
 assert eval(pass,[both])==[set<Key>(items:[present,unset])]
 assert both!=set<Key>(items:[present])
}}
",
        true,
    )?;
    let source = r"module nan_key
struct Key {part:f64=null}
native const fn mark(value:f64)->f64 throws
native const fn mark(value:f64)->f64 throws {}
native const fn value(item:i64)->i64 throws
native const fn value(item:i64)->i64 throws {}
fn observe(tick:i64) {when {let result=map<Key,i64>(items:[Key(part:mark(0.0)):value(tick)])}}
test reject {eval(observe,[1])}
";
    let provider = r#"mod native {
pub fn mark_f64(_:f64)->hgl_types::NodeResult<f64> {println!("MARKnan");Ok(f64::NAN)}
pub fn value_i64(v:i64)->hgl_types::NodeResult<i64> {println!("MARKvalue");Ok(v)}
}"#;
    run_program(
        source,
        false,
        provider,
        Some("NaN collection key"),
        &["MARKnan"],
    )
}

#[test]
fn native_collection_equality_preserves_optional_family_and_recursive_descendants()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(r#"module nested_equality
struct Node {tags:set<i64>
next:atomic<Node> =null}
struct Optional {tags:set<i64>=null}
abstract struct Event {label:str}
struct Member:Event {tags:set<i64>}
fn compare(tick:i64)->bool {when {
 let left=map<str,Node>(items:["row":Node(tags:set<i64>(items:[1,2]),next:Node(tags:set<i64>(items:[3,4])))])
 let right=map<str,Node>(items:["row":Node(tags:set<i64>(items:[2,1]),next:Node(tags:set<i64>(items:[4,3])))])
 let a=map<str,Optional>(items:["row":Optional(tags:set<i64>(items:[1,2]))])
 let b=map<str,Optional>(items:["row":Optional(tags:set<i64>(items:[2,1]))])
 let missing=map<str,Optional>(items:["row":Optional()])
 let first:Event=Member(label:"same",tags:set<i64>(items:[1,2]))
 let second:Event=Member(label:"same",tags:set<i64>(items:[2,1]))
 return left==right && a==b && a!=missing && first==second
}}
test equality {assert eval(compare,[1])==[true]}
"#.replace(">=","> =").as_str(),false)
}

#[test]
fn shared_sparse_composite_keys_execute_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/composite_key_values.hgl"),
        true,
    )
}
#[test]
fn composite_domains_are_shared_across_opposite_plan_and_key_orders()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"module hgraph.std part domain_order
struct A {number:i64
label:str}
struct Z {nested:tuple<str,f64>
text:str=null}
test {
 fn pass<T>(value:T)->T {when {return delta_value(value)}}
 test opposite_domains {
  let z=Z(nested:("z",-0.0))
  let a=A(number:7,label:"a")
  assert eval(pass,[delta<map<Z,i64>>(upsert:[z:1,Z(nested:("z",0.0),text:""):2])])==[delta<map<Z,i64>>(upsert:[Z(nested:("z",0.0),text:""):2,z:1])]
  assert eval(pass,[delta<set<A>>(added:[a,A(number:8,label:"b")])])==[delta<set<A>>(added:[A(number:8,label:"b"),a])]
  assert eval(pass,[delta<set<A>>(added:[A(number:8,label:"b"),a])])==[delta<set<A>>(added:[a,A(number:8,label:"b")])]
  assert eval(pass,[delta<map<Z,i64>>(upsert:[Z(nested:("z",0.0),text:""):2,z:1]),delta<map<Z,i64>>(remove:[z])])==[delta<map<Z,i64>>(upsert:[z:1,Z(nested:("z",0.0),text:""):2]),delta<map<Z,i64>>(remove:[Z(nested:("z",0.0))])]
 }
}
"#,
        true,
    )
}
#[test]
fn composite_scalar_leaves_keep_enum_and_provider_identity()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"module hgraph.std part composite_scalars
enum E {first=-7,second=11}
struct Key {tag:E
zone:timezone
nested:tuple<date,time,duration>
label:str=null}
test {
 fn pass<T>(value:T)->T {when {return delta_value(value)}}
 test leaves {
  let alias=Key(tag:E::first,zone:@[US/Eastern],nested:(@2026-01-01,@12:00,1s),label:"retained")
  let canonical=Key(tag:E::first,zone:@[America/New_York],nested:(@2026-01-01,@12:00,1s),label:"retained")
  assert eval(pass,[delta<set<Key>>(added:[alias,canonical]),delta<set<Key>>(removed:[alias]),delta<set<Key>>(added:[alias])])==[delta<set<Key>>(added:[canonical,alias]),delta<set<Key>>(removed:[alias]),delta<set<Key>>(added:[alias])]
 }
}
"#,
        true,
    )
}
#[test]
fn inferred_inherited_arguments_restore_source_atomic_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(
        r#"module hgraph.std part inferred_atomic_identity
struct Child {number:i64}
struct Book {snapshot:atomic<Child>}
struct Box<T> {value:T}
struct Derived<T>:Box<T> {label:str}
test {
 const fn data()->Derived<Book> {
  let inferred=Derived(value:Book(snapshot:Child(number:7)),label:"source")
  let exact:Derived<Book> = inferred
  return exact
 }
 fn pass(value:atomic<Derived<Book>>)->atomic<Derived<Book>> {when {return delta_value(value)}}
 test canonical_identity {
  assert eval(pass,[data(),_,data()])==[data(),_,data()]
 }
}
"#,
        true,
    )
}
