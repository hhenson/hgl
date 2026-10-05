//! The complete generated replay, target and recording path must not allocate in ticks.
use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
const SOURCE: &str = r#"module prepared_execution
native const fn configuration_marker(value:i64)->i64 throws
native const fn configuration_marker(value:i64)->i64 throws {}
struct ConfigRow {time:datetime
value:i64}
const fn configuration_rows()->list<ConfigRow> {let value=configuration_marker(7)
return [ConfigRow(time:@1970-01-01T00:00:00.000001Z,value:value)]}
fn configured_events(const rows:list<ConfigRow>)->i64 {var index=0
while index<len(rows) {yield rows[index].time:rows[index].value
index+=1}}
fn configured_context(tick:i64) {when {}}
fn once_configuration(tick:i64)->i64 {configured_context(tick)
configured_events(configuration_rows())}
test once_configuration {assert eval(once_configuration,tick:[1]) == [7]}
fn legacy_members(value:i64)->set<i64> {inject out
when {if value>0 {upsert(out,value)} else {discard(out,-value)}}}
fn legacy_bools(value:bool)->set<bool> {inject out
when {upsert(out,value)}}
test legacy_members {assert eval(legacy_members,value:[1,2,-1,1]) == [delta<set<i64>>(added:[1]),delta<set<i64>>(added:[2]),delta<set<i64>>(removed:[1]),delta<set<i64>>(added:[1])]}
test legacy_bools {assert eval(legacy_bools,value:[true,false,true]) == [delta<set<bool>>(added:[true]),delta<set<bool>>(added:[false]),_]}
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
    let mut sources = vec![(
        "prepared_execution.hgl".into(),
        format!("{SOURCE}{}", scaling_source()),
    )];
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
        .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(")
        .replace(
            "store.prepare_collection_inputs();",
            "crate::verify_pool(&mut store,&built);store.prepare_collection_inputs();",
        );
    code.push_str(r#"
struct Provider;
mod native {pub fn configuration_marker_i64(value:i64)->hgl_types::NodeResult<i64> {assert_eq!(super::CONFIGURATION_CALLS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0,"configuration initializer executed more than once");Ok(value)}}
static CONFIGURATION_CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn verify_pool(store:&mut hgl_store::Store,built:&hgl_describe::BuiltGraph) {
 fn count(store:&hgl_store::Store,id:hgl_store::OutputId)->usize {let output=store.bindings().output(id);1+output.members.prepared.iter().map(|(_,child)|count(store,*child)).sum::<usize>()+output.fixed.iter().map(|child|count(store,*child)).sum::<usize>()}
 let Some(root)=built.outputs.iter().flatten().copied().find(|id|store.bindings().output(*id).members.prepared.len()>=8) else{return;};
 let n=store.bindings().output(root).members.prepared.len();let mut depth=0;let mut node=root;
 while let Some((_,child))=store.bindings().output(node).members.prepared.first() {depth+=1;node=*child;}
 let descendants=built.outputs.iter().flatten().map(|id|count(store,*id)).sum::<usize>();
 assert_eq!(descendants,2*(1+n*depth),"finite topology must preserve per-parent reachability");
 let (scalars,lists)=store.global_state().values().slot_counts();let rows=6*n+1;
 assert!(scalars<=rows*(depth+3)+1,"record scalar pool grew beyond finite recipe bounds: {scalars}");
 assert!(lists<=rows*3*depth+2,"record list pool grew beyond finite recipe bounds: {lists}");
 println!("prepared scaling N={n} depth={depth}: temporal endpoints={descendants}, record scalar slots={scalars}, record list slots={lists}");
}
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
    manifest(root, &dir)?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            .args(profile)
            .current_dir(&dir)
            .output()?;
        for line in String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.starts_with("prepared scaling"))
        {
            println!("{line}");
        }
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

fn manifest(
    root: &std::path::Path,
    dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
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

fn scaling_source() -> String {
    fn ty(depth: usize) -> String {
        if depth == 0 {
            "str".into()
        } else {
            format!("map<str,{}>", ty(depth - 1))
        }
    }
    fn value(depth: usize, index: usize, payload: &str) -> String {
        if depth == 0 {
            format!("{payload:?}")
        } else {
            format!(
                "delta<{}>(upsert:[\"level{depth}-{index}\":{}])",
                ty(depth),
                value(depth - 1, index, payload)
            )
        }
    }
    let mut source = String::new();
    for depth in [2, 3] {
        for n in [8, 32] {
            let mut rows = Vec::new();
            for index in 0..n {
                rows.push(value(depth, index, "first"));
                rows.push(format!(
                    "delta<{}>(remove:[\"level{depth}-{index}\"])",
                    ty(depth)
                ));
                rows.push(value(depth, index, "longer independent retained payload"));
            }
            let rows = rows.join(",");
            write!(source,"\nfn scale_{depth}_{n}(value:{})->{} {{when {{return delta_value(value)}}}}\ntest scale_{depth}_{n} {{assert eval(scale_{depth}_{n},value:[{rows}]) == [{rows}]}}\n",ty(depth),ty(depth)).unwrap_or_else(|_|unreachable!("String formatting"));
        }
    }
    source
}
