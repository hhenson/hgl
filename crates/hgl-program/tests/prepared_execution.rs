//! The complete generated replay, target and recording path must not allocate in ticks.
use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
const SOURCE: &str = r#"module prepared_execution
fn text(value:str)->str {when {return delta_value(value)}}
fn zone(value:timezone)->timezone {when {return delta_value(value)}}
fn clock(value:zoned_time)->zoned_time {when {return delta_value(value)}}
fn fixed(value:list<str,2>)->list<str,2> {when {return delta_value(value)}}
fn keyed(value:map<str,i64>)->map<str,i64> {when {return delta_value(value)}}
fn nested(value:map<timezone,set<zoned_time>>)->map<timezone,set<zoned_time>> {when {return delta_value(value)}}
fn whole(value:atomic<list<str>>)->atomic<list<str>> {when {return delta_value(value)}}
struct Pair {label:str
values:list<i64>}
struct Bundle {label:str
count:i64}
fn bundle(value:Bundle)->Bundle {when {return delta_value(value)}}
fn keyed_whole(value:map<str,atomic<list<str>>>)->map<str,atomic<list<str>>> {when {return delta_value(value)}}
fn atomic_pair(value:atomic<Pair>)->atomic<Pair> {when {return delta_value(value)}}
test text {assert eval(text,value:["a","a",_,"a longer retained value"]) == ["a","a",_,"a longer retained value"]}
test zone {assert eval(zone,value:[@[US/Eastern],@[America/New_York],_,@[US/Eastern]]) == [@[US/Eastern],@[America/New_York],_,@[US/Eastern]]}
test clock {assert eval(clock,value:[@09:30[US/Eastern],@09:30[America/New_York],_,@09:30[US/Eastern]]) == [@09:30[US/Eastern],@09:30[America/New_York],_,@09:30[US/Eastern]]}
test fixed {assert eval(fixed,value:[delta<list<str,2>>(items:[0:"first",1:"second"]),delta<list<str,2>>(items:[1:"longer retained value"]),_,delta<list<str,2>>(items:[0:"first"])]) == [delta<list<str,2>>(items:[0:"first",1:"second"]),delta<list<str,2>>(items:[1:"longer retained value"]),_,delta<list<str,2>>(items:[0:"first"])]}
test keyed {assert eval(keyed,value:[delta<map<str,i64>>(upsert:["key":1]),delta<map<str,i64>>(remove:["key"]),delta<map<str,i64>>(upsert:["key":2])]) == [delta<map<str,i64>>(upsert:["key":1]),delta<map<str,i64>>(remove:["key"]),delta<map<str,i64>>(upsert:["key":2])]}
test nested {assert eval(nested,value:[delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[US/Eastern]])]),delta<map<timezone,set<zoned_time>>>(remove:[@[UTC]]),delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[America/New_York]])])]) == [delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[US/Eastern]])]),delta<map<timezone,set<zoned_time>>>(remove:[@[UTC]]),delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[America/New_York]])])]}
test whole {let first:list<str> = ["first"]
assert eval(whole,value:[first,[],_,["longer retained value"],first]) == [first,[],_,["longer retained value"],first]
assert eval(whole,value:[first]) == [first]}
test bundle {assert eval(bundle,value:[delta<Bundle>(label:"first",count:1),delta<Bundle>(count:2),_,delta<Bundle>(label:"longer retained value")]) == [delta<Bundle>(label:"first",count:1),delta<Bundle>(count:2),_,delta<Bundle>(label:"longer retained value")]}
test keyed_whole {assert eval(keyed_whole,value:[delta<map<str,atomic<list<str>>>>(upsert:["key":["first"]]),delta<map<str,atomic<list<str>>>>(upsert:["key":[]]),delta<map<str,atomic<list<str>>>>(remove:["key"]),delta<map<str,atomic<list<str>>>>(upsert:["key":["longer retained value"]])]) == [delta<map<str,atomic<list<str>>>>(upsert:["key":["first"]]),delta<map<str,atomic<list<str>>>>(upsert:["key":[]]),delta<map<str,atomic<list<str>>>>(remove:["key"]),delta<map<str,atomic<list<str>>>>(upsert:["key":["longer retained value"]])]}
test atomic_pair {let first:Pair=Pair(label:"first",values:[1,2])
assert eval(atomic_pair,value:[first,Pair(label:"longer retained value",values:[]),_,first]) == [first,Pair(label:"longer retained value",values:[]),_,first]}
"#;
#[test]
fn finite_owning_publications_use_prepared_storage() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("crate parent")?
        .parent()
        .ok_or("workspace parent")?;
    let mut sources = vec![("prepared_execution.hgl".into(), SOURCE.into())];
    for file in ["replay_record.hgl", "impl/replay_record.hgl"] {
        sources.push((
            file.into(),
            fs::read_to_string(root.join("external/hgraph_std/hgl/hgraph").join(file))?,
        ));
    }
    let suite = hgl_program::compile_tests(&sources)?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-prepared-execution-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    let mut code = hgl_program::emit_tests(&suite)
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(");
    code.push_str(r#"
struct Provider;
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph, store:&mut hgl_store::Store, config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError> {
 graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
 let mut cycles=0; let mut total=0; let mut now=config.start_time;
 while !graph.stop_requested() {let next=graph.next_scheduled_time();if next>=config.end_time {break;}now=next;
 let (result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));result.map_err(hgl_kernel::EngineError::Node)?;total+=count;cycles+=1;}
 graph.stop(store,now).map_err(hgl_kernel::EngineError::Node)?;
 assert_eq!(total,0,"complete generated publication and recording allocated across {cycles} cycles");Ok(cycles)
}
"#);
    fs::write(dir.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"prepared-execution\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    ] {
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name)
        )?;
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
