//! The complete generated replay, target and recording path must not allocate in ticks.
use std::{fmt::Write as _, fs, process::Command, time::SystemTime};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const SOURCE: &str = r#"module prepared_execution
fn constructed_map(value:i64)->map<str,i64> {when {return delta<map<str,i64>>(upsert:["retained":value])}}
test constructed_map {assert eval(constructed_map,value:[1,2]) == [delta<map<str,i64>>(upsert:["retained":1]),delta<map<str,i64>>(upsert:["retained":2])]}
fn retained_key(value:i64)->map<str,i64> {when {let key="retained"
let alias=key
return delta<map<str,i64>>(upsert:[alias:value])}}
test retained_key {assert eval(retained_key,value:[1,2]) == [delta<map<str,i64>>(upsert:["retained":1]),delta<map<str,i64>>(upsert:["retained":2])]}
fn nested_constructor(value:str)->map<str,map<str,str>> {when {let outer="outer"
let inner="inner"
return delta<map<str,map<str,str>>>(upsert:[outer:delta<map<str,str>>(upsert:[inner:value])])}}
test nested_constructor {assert eval(nested_constructor,value:["first","longer",_,"first"]) == [delta<map<str,map<str,str>>>(upsert:["outer":delta<map<str,str>>(upsert:["inner":"first"])]),delta<map<str,map<str,str>>>(upsert:["outer":delta<map<str,str>>(upsert:["inner":"longer"])]),_,delta<map<str,map<str,str>>>(upsert:["outer":delta<map<str,str>>(upsert:["inner":"first"])])]}
fn alias_reinsert(value:i64)->map<str,i64> {when {let key="stable"
let alias=key
if value>0 {return delta<map<str,i64>>(upsert:[alias:value+1])} else {return delta<map<str,i64>>(remove:[alias])}}}
test alias_reinsert {assert eval(alias_reinsert,value:[1,0,2]) == [delta<map<str,i64>>(upsert:["stable":2]),delta<map<str,i64>>(remove:["stable"]),delta<map<str,i64>>(upsert:["stable":3])]}
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
native const fn opaque(value:i64)->i64 throws
native const fn opaque(value:i64)->i64 throws {}
native const fn key_start(value:i64) throws
native const fn key_start(value:i64) throws {}
native const fn key_stop(value:i64) throws
native const fn key_stop(value:i64) throws {}
fn opaque_members(value:i64,other:i64)->set<i64> {inject out
start {key_start(0)}
stop {key_stop(0)}
when {let key=opaque(value+other)
if key>=0 {upsert(out,key)
upsert(out,key+100)} else {discard(out,-key)
discard(out,-key+100)}}}
test opaque_members {assert eval(opaque_members,value:[1,2,-3,1],other:[1,1,1,1]) == [delta<set<i64>>(added:[2,102]),delta<set<i64>>(added:[3,103]),delta<set<i64>>(removed:[2,102]),delta<set<i64>>(added:[2,102])]}
fn paired_members(left:set<i64>,right:set<i64>)->set<i64> {inject out
when {for a in elements(left,added) {for b in elements(right,added) {upsert(out,a*100+b)}}}}
test paired_members {assert eval(paired_members,left:[delta<set<i64>>(added:[1,2])],right:[delta<set<i64>>(added:[3,4,5])]) == [delta<set<i64>>(added:[103,104,105,203,204,205])]}
fn bounded_members(value:i64)->set<i64> {inject out
when {var i=0
while i<5 {upsert(out,i)
i+=1}}}
test bounded_members {assert eval(bounded_members,value:[1]) == [delta<set<i64>>(added:[0,1,2,3,4])]}
fn nested_bounded_members(value:i64)->set<i64> {inject out
when {var i=2
while i>=0 {var j=0
while j<2 {upsert(out,i*10+j)
j+=1}
i=i-1}}}
test nested_bounded_members {assert eval(nested_bounded_members,value:[1]) == [delta<set<i64>>(added:[20,21,10,11,0,1])]}
fn computed_text(value:str)->str {when {return value+":"+value}}
test computed_text {assert eval(computed_text,value:["a","longer",""]) == ["a:a","longer:longer",":"]}
fn computed_members(value:i64)->set<i64> {inject out
when {let key=value+1
if value>=0 {upsert(out,key)} else {discard(out,-key)}}}
test computed_members {assert eval(computed_members,value:[1,2,-3,1]) == [delta<set<i64>>(added:[2]),delta<set<i64>>(added:[3]),delta<set<i64>>(removed:[2]),delta<set<i64>>(added:[2])]}
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
fn growing_list_publications_use_prepared_storage() -> Result<(), Box<dyn std::error::Error>> {
    execute(
        format!(
            "{}\n{}",
            include_str!("../../../external/hgraph_std/hgl/hgraph/tests/growing_list_values.hgl"),
            GROWING_CONSTRUCTORS
        ),
        RUNTIME,
        true,
    )
}
#[test]
fn bytes_construction_publication_and_recording_allocate_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    execute(r#"module bytes_prepared
const fn convert(value:list<i64>)->bytes {return bytes(value)}
fn direct(value:atomic<list<i64>>)->bytes {when {return bytes(value)}}
fn helper(value:atomic<list<i64>>)->bytes {when {return convert(value)}}
fn fixed(value:atomic<list<i64,2>>)->bytes {when {return bytes(value)}}
fn invalid(value:i64)->bytes {when {return bytes([256])}}
fn forward(value:bytes)->bytes {when {return delta_value(value)}}
fn members(value:map<bytes,bytes>)->map<bytes,bytes> {when {return delta_value(value)}}
fn rolling(value:rolling<bytes,2>)->rolling<bytes,2> {when {return delta_value(value)}}
fn compare(value:bytes,other:bytes)->bool {when {return len(value)>=0 && value==other}}
test compare {assert eval(compare,[bytes(),bytes([0,255]),_,bytes([128])],[bytes(),bytes([0,255]),_,bytes([127])])==[true,true,_,false]}
test direct {assert eval(direct,[[],[0,128,255],_,[0],[]])==[bytes(),bytes([0,128,255]),_,bytes([0]),bytes()]}
test helper {assert eval(helper,[[255,0],[],_,[255,0]])==[bytes([255,0]),bytes(),_,bytes([255,0])]}
test fixed {assert eval(fixed,[[0,255],_,[255,0]])==[bytes([0,255]),_,bytes([255,0])]}
test invalid {assert raises("value.byte_range") {eval(invalid,[1])}}
test forward {assert eval(forward,[bytes(),bytes(),_,bytes([0,255])])==[bytes(),bytes(),_,bytes([0,255])]}
test members {assert eval(members,[delta<map<bytes,bytes>>(upsert:[bytes():bytes([255])]),delta<map<bytes,bytes>>(remove:[bytes()])])==[delta<map<bytes,bytes>>(upsert:[bytes():bytes([255])]),delta<map<bytes,bytes>>(remove:[bytes()])]}
test rolling {assert eval(rolling,[bytes(),bytes([0,255]),_,bytes([0,255])])==[bytes(),bytes([0,255]),_,bytes([0,255])]}
"#.into(), RUNTIME, true)
}

#[test]
fn scalar_byte_returns_and_rolling_arrivals_allocate_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    execute(r"module bytes_return_shapes
fn copy(value:bytes)->bytes {when {return value}}
fn arrival(value:bytes)->rolling<bytes,2> {when {return value}}
fn observed_arrival(value:bytes)->rolling<bytes,2> {when {return delta_value(value)}}
test copy {assert eval(copy,[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])])==[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])]}
test arrival {assert eval(arrival,[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])])==[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])]}
test observed_arrival {assert eval(observed_arrival,[bytes([0,255]),bytes(),_,bytes([0,255])])==[bytes([0,255]),bytes(),_,bytes([0,255])]}
".into(), RUNTIME, true)
}

#[test]
fn retained_byte_locals_publish_without_allocating() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from("module retained_byte_locals\n");
    for (input, initializer) in [("direct", "value"), ("observed", "delta_value(value)")] {
        for (output, result) in [("scalar", "bytes"), ("rolling", "rolling<bytes,2>")] {
            for (copy, binding) in [("local", "held"), ("alias", "alias")] {
                let name = format!("{input}_{output}_{copy}");
                writeln!(
                    source,
                    "fn {name}(value:bytes)->{result} {{when {{let held={initializer}\nlet alias=held\nif len(alias)==len(value) && alias==value {{return {binding}}}}}}}"
                )?;
                writeln!(
                    source,
                    "test {name} {{assert eval({name},[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])])==[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4]),bytes([0,255])]}}"
                )?;
            }
        }
    }
    source.push_str(r"fn scalar_assignment(value:bytes)->bytes {inject out
when {let held=value
out=held}}
fn rolling_assignment(value:bytes)->rolling<bytes,2> {inject out
when {let held=delta_value(value)
out=held}}
test scalar_assignment {assert eval(scalar_assignment,[bytes([0,255]),bytes(),_,bytes([1,2,3,4])])==[bytes([0,255]),bytes(),_,bytes([1,2,3,4])]}
test rolling_assignment {assert eval(rolling_assignment,[bytes([0,255]),bytes(),_,bytes([1,2,3,4])])==[bytes([0,255]),bytes(),_,bytes([1,2,3,4])]}
fn independent(value:bytes,other:bytes)->rolling<bytes,2> {when {let held=value
let alias=held
let later=other
if alias!=later {return alias}}}
test independent {assert eval(independent,[bytes([0,255]),bytes(),bytes([1,2,3,4])],[bytes([1]),bytes([2]),bytes()])==[bytes([0,255]),bytes(),bytes([1,2,3,4])]}
fn pair(value:bytes)->tuple<bytes,i64> {when {let held=value
return (held,len(held))}}
test pair {assert eval(pair,[bytes([0,255]),bytes(),_,bytes([1,2,3,4])])==[(bytes([0,255]),2),(bytes(),0),_,(bytes([1,2,3,4]),4)]}
test ordinary_locals {let original=bytes([0,255])
var owned=original
owned=bytes()
assert original==bytes([0,255])
assert owned==bytes()}

");
    execute(source, RUNTIME, true)
}

#[test]
fn retained_rolling_byte_arrivals_publish_without_allocating()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from("module retained_rolling_byte_arrivals\n");
    for (output, result) in [("scalar", "bytes"), ("rolling", "rolling<bytes,2>")] {
        for (operation, publication) in [("returned", "return alias"), ("assigned", "out=alias")] {
            let name = format!("{output}_{operation}");
            writeln!(
                source,
                "fn {name}(value:rolling<bytes,2>)->{result} {{inject out\nwhen {{let held=delta_value(value)\nlet alias=held\n{publication}}}}}"
            )?;
            writeln!(
                source,
                "test {name} {{assert eval({name},[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4])])==[bytes([0,255]),bytes(),_,bytes([0,255]),bytes([1,2,3,4])]}}"
            )?;
        }
    }
    execute(source, RUNTIME, true)
}

#[test]
fn rolling_byte_conversion_uses_prepared_execution() -> Result<(), Box<dyn std::error::Error>> {
    execute(
        r#"module rolling_bytes_constructor
const fn convert(value:list<i64>)->bytes {return bytes(value)}
fn direct(value:atomic<list<i64>>)->rolling<bytes,2> {when {return bytes(value)}}
fn helper(value:atomic<list<i64>>)->rolling<bytes,2> {when {if true {return convert(value)}}}
fn fixed(value:atomic<list<i64,2>>)->rolling<bytes,2> {when {return bytes(value)}}
test direct {assert eval(direct,[[0,255],[],_,[0,255]])==[bytes([0,255]),bytes(),_,bytes([0,255])]}
test helper {assert eval(helper,[[0,255],[],_,[0,255]])==[bytes([0,255]),bytes(),_,bytes([0,255])]}
test fixed {assert eval(fixed,[[0,255],_,[128,255]])==[bytes([0,255]),_,bytes([128,255])]}
test invalid {assert raises("value.byte_range") {eval(direct,[[256]])}}
"#
        .into(),
        RUNTIME,
        true,
    )
}

#[test]
fn byte_literals_in_sparse_publications_allocate_nothing() -> Result<(), Box<dyn std::error::Error>>
{
    execute(
        r"module bytes_sparse_prepared
fn keyed(value:i64,const payload:bytes)->map<bytes,bytes> {when {
if value>0 {return delta<map<bytes,bytes>>(upsert:[bytes([1]):payload])}
else {return delta<map<bytes,bytes>>(remove:[bytes([1])])}}}
fn members(value:i64)->set<bytes> {when {
if value>0 {return delta<set<bytes>>(added:[bytes([128,255])])}
else {return delta<set<bytes>>(removed:[bytes([128,255])])}}}
test keyed {assert eval(keyed,[1,2,0,3],payload:bytes([0,255])) == [delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])]),delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])]),delta<map<bytes,bytes>>(remove:[bytes([1])]),delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])])]}
test members {assert eval(members,[1,0,2]) == [delta<set<bytes>>(added:[bytes([128,255])]),delta<set<bytes>>(removed:[bytes([128,255])]),delta<set<bytes>>(added:[bytes([128,255])])]}
".into(),
        RUNTIME,
        true,
    )?;
    execute(
        r"module bytes_sparse_fallback
fn keyed(value:i64)->map<bytes,bytes> {when {
return delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])])}}
test keyed {assert eval(keyed,[1,2]) == [delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])]),delta<map<bytes,bytes>>(upsert:[bytes([1]):bytes([0,255])])]}
".into(),
        RUNTIME,
        false,
    )
}

#[test]
fn retained_atomic_and_rolling_byte_structs_allocate_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    execute(r"module retained_byte_structs
struct Packet {data:bytes}
fn atomic_copy(value:atomic<Packet>)->atomic<Packet> {when {return delta_value(value)}}
fn rolling_copy(value:rolling<Packet,2>)->rolling<Packet,2> {when {return delta_value(value)}}
fn atomic_config(value:i64,const packet:Packet)->atomic<Packet> {when {return packet}}
fn rolling_config(value:i64,const packet:Packet)->rolling<Packet,2> {when {return packet}}
test atomic_copy {assert eval(atomic_copy,[Packet(data:bytes([0,255])),Packet(data:bytes()),_,Packet(data:bytes([128]))])==[Packet(data:bytes([0,255])),Packet(data:bytes()),_,Packet(data:bytes([128]))]}
test rolling_copy {assert eval(rolling_copy,[Packet(data:bytes([0,255])),Packet(data:bytes()),_,Packet(data:bytes([128]))])==[Packet(data:bytes([0,255])),Packet(data:bytes()),_,Packet(data:bytes([128]))]}
test atomic_config {assert eval(atomic_config,[1,2],packet:Packet(data:bytes([0,255])))==[Packet(data:bytes([0,255])),Packet(data:bytes([0,255]))]}
test rolling_config {assert eval(rolling_config,[1,2],packet:Packet(data:bytes([0,255])))==[Packet(data:bytes([0,255])),Packet(data:bytes([0,255]))]}
".into(), RUNTIME, true)
}

#[test]
fn finite_owning_publications_use_prepared_storage() -> Result<(), Box<dyn std::error::Error>> {
    execute(
        format!("{SOURCE}{}{}", scaling_source(), branch_source()),
        RUNTIME,
        true,
    )
}
#[test]
fn unknown_finite_bounds_keep_existing_evaluation_semantics()
-> Result<(), Box<dyn std::error::Error>> {
    execute(COMPATIBILITY.into(), COMPATIBILITY_RUNTIME, false)
}
fn execute(
    source: String,
    runtime: &str,
    measured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("crate parent")?
        .parent()
        .ok_or("workspace parent")?;
    let mut sources = vec![("prepared_execution.hgl".into(), source)];
    for file in [
        "replay_record.hgl",
        "impl/replay_record.hgl",
        "control.hgl",
        "impl/control.hgl",
        "standard.hgl",
        "impl/standard.hgl",
        "native/scalar_values.hgl",
        "native/scalar_values_i64.hgl",
        "native/scalar_operators.hgl",
        "native/temporal_values.hgl",
    ] {
        sources.push((
            file.into(),
            fs::read_to_string(root.join("external/hgraph_std/hgl/hgraph").join(file))?,
        ));
    }
    sources.push((
        "native-rust.hgl".into(),
        fs::read_to_string(root.join("native/stdlib/rust.hgl"))?,
    ));
    sources.push((
        "backend-interfaces.hgl".into(),
        fs::read_to_string(root.join("native/stdlib/interfaces.hgl"))?,
    ));
    let suite = hgl_program::compile_module_suite(&sources)?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-prepared-execution-{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(dir.join("src"))?;
    let mut code = hgl_program::emit_tests(&suite);
    if measured {
        code = code
            .replace("hgl_kernel::run_simulation(", "crate::measured_simulation(")
            .replace(
                "store.prepare_collection_inputs();",
                "crate::verify_pool(&mut store,&built,&graph);store.prepare_collection_inputs();",
            );
    } else {
        assert!(
            !code.contains("let capacity=prepare_capacity"),
            "unknown adapter must preserve generic execution"
        );
    }
    code.push_str(runtime);
    fs::write(dir.join("src/main.rs"), code)?;
    manifest(root, &dir)?;
    // The gate runs the suite in both profiles; each build follows the profile of this test binary.
    let profile: Vec<&str> = if cfg!(debug_assertions) {
        vec![]
    } else {
        vec!["--release"]
    };
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
        .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
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
        "hgl-semantics",
        "hgl-source",
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
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, dir))?;
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

const RUNTIME: &str = r#"
struct Provider;
mod native {pub fn key_stop_i64(_:i64)->hgl_types::NodeResult {assert_eq!(super::KEY_CALLS.load(std::sync::atomic::Ordering::SeqCst),4);assert_eq!(super::KEY_STARTS.load(std::sync::atomic::Ordering::SeqCst),1);Ok(())} pub fn opaque_i64(value:i64)->hgl_types::NodeResult<i64> {super::KEY_CALLS.fetch_add(1,std::sync::atomic::Ordering::SeqCst);Ok(value)} pub fn key_start_i64(_:i64)->hgl_types::NodeResult {assert_eq!(super::KEY_STARTS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0);Ok(())} pub fn configuration_marker_i64(value:i64)->hgl_types::NodeResult<i64> {assert_eq!(super::CONFIGURATION_CALLS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0,"configuration initializer executed more than once");Ok(value)}}
static KEY_CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static KEY_STARTS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static CONFIGURATION_CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn verify_pool(store:&mut hgl_store::Store,built:&hgl_describe::BuiltGraph,graph:&hgl_describe::GraphDescription) {
 for (name,expected) in [("::bounded_members#",15),("::nested_bounded_members#",18)] {
 if let Some(index)=graph.nodes.iter().position(|node|node.implementation.contains(name)) {let output=built.outputs[index].expect("bounded mutation output");assert_eq!(store.bindings().output(output).members.prepared.len(),expected,"constant induction bound");}
 }
 if let Some(node)=graph.nodes.iter().find(|node|node.implementation.contains("::branch_member_")) {
 let n:usize=node.implementation.split("::branch_member_").nth(1).unwrap().split('#').next().unwrap().parse().unwrap();
 let roots=built.outputs.iter().flatten().filter(|id|store.bindings().output(**id).members.live.pooled()).collect::<Vec<_>>();
 assert_eq!(roots.len(),n);for &&root in &roots {assert_eq!(store.bindings().output(root).members.prepared.len(),10,"independent outputs must not multiply one another's mutation bounds");}
 let descendants=built.outputs.iter().flatten().map(|id|count(store,*id)).sum::<usize>();assert_eq!(descendants,1+11*n);
 println!("prepared branches N={n}: temporal endpoints={descendants}");return;
 }
 if !graph.nodes.iter().any(|node|node.implementation.contains("::scale_")) {return;}
 fn count(store:&hgl_store::Store,id:hgl_store::OutputId)->usize {let output=store.bindings().output(id);1+output.members.prepared.iter().map(|(_,child)|count(store,*child)).sum::<usize>()+output.fixed.iter().map(|child|count(store,*child)).sum::<usize>()}
 let root=built.outputs.iter().flatten().copied().find(|id|store.bindings().output(*id).members.prepared.len()>=8).expect("scaling fixture root must be prepared");
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
"#;

fn branch_source() -> String {
    let mut source = String::new();
    for n in [2, 16] {
        writeln!(source,"fn branch_member_{n}(value:i64)->set<i64> {{inject out\nwhen {{upsert(out,value)\nupsert(out,value+100)}}}}\nfn branches_{n}(value:i64)->set<i64> {{").unwrap_or_else(|_|unreachable!());
        for i in 1..n {
            writeln!(source, "let member{i}=branch_member_{n}(value)")
                .unwrap_or_else(|_| unreachable!());
        }
        writeln!(source,"branch_member_{n}(value)}}\ntest branches_{n} {{assert eval(branches_{n},value:[1,2]) == [delta<set<i64>>(added:[1,101]),delta<set<i64>>(added:[2,102])]}}").unwrap_or_else(|_|unreachable!());
    }
    source
}

const COMPATIBILITY: &str = r#"module existing_execution
fn input_limit(value:i64)->set<i64> {inject out
when {var i=0
while i<value {upsert(out,i)
i+=1}}}
test input_limit {assert eval(input_limit,value:[5]) == [delta<set<i64>>(added:[0,1,2,3,4])]}
native const fn limit(value:i64)->i64 throws
native const fn limit(value:i64)->i64 throws {}
native const fn begin(value:i64) throws
native const fn begin(value:i64) throws {}
native const fn end(value:i64) throws
native const fn end(value:i64) throws {}
fn native_limit(value:i64)->set<i64> {inject out
start {begin(0)}
stop {end(0)}
when {let count=limit(value)
var i=0
while i<count {upsert(out,i)
i+=1}}}
test native_limit {assert eval(native_limit,value:[5]) == [delta<set<i64>>(added:[0,1,2,3,4])]}
fn text_loop(value:str)->str {when {var text=value
var i=0
while i<5 {text=text+text
i+=1}
return text}}
test text_loop {assert eval(text_loop,value:["a"]) == ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]}
"#;
const COMPATIBILITY_RUNTIME: &str = r"
struct Provider;
mod native {
static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
static STARTS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
pub fn limit_i64(value:i64)->hgl_types::NodeResult<i64> {CALLS.fetch_add(1,std::sync::atomic::Ordering::SeqCst);Ok(value)}
pub fn begin_i64(_:i64)->hgl_types::NodeResult {assert_eq!(STARTS.fetch_add(1,std::sync::atomic::Ordering::SeqCst),0);Ok(())}
pub fn end_i64(_:i64)->hgl_types::NodeResult {assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst),1);assert_eq!(STARTS.load(std::sync::atomic::Ordering::SeqCst),1);Ok(())}
}
";

const GROWING_CONSTRUCTORS: &str = r"fn growing_concrete(value:list<i64>)->list<i64> {when {return delta_value(value)}}
test growing_concrete_compute {assert eval(growing_concrete,[delta<list<i64>>(items:[0:1,1:2]),delta<list<i64>>(remove:[0,1]),delta<list<i64>>(items:[0:3])]) == [delta<list<i64>>(items:[1:2,0:1]),delta<list<i64>>(remove:[1,0]),delta<list<i64>>(items:[0:3])]}
fn growing_construct(value:i64)->list<i64> {when {if value>0 {return delta<list<i64>>(items:[0:value])} else {return delta<list<i64>>(remove:[0])}}}
test growing_constructed_child {assert eval(growing_construct,[1,2,0,3]) == [delta<list<i64>>(items:[0:1]),delta<list<i64>>(items:[0:2]),delta<list<i64>>(remove:[0]),delta<list<i64>>(items:[0:3])]}
";

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &std::path::Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
