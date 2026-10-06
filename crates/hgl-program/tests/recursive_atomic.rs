//! Finite recursive atomic shared semantics and actual evaluation allocation checks.
use hgl_program::compile_tests;

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn sources(source: &str) -> Vec<(String, String)> {
    vec![
        ("recursive.hgl".into(), source.into()),
        ("recursive_empty.hgl".into(),r"module hgraph.std part recursive_empty_extra
struct EmptyRecursive {next:atomic<EmptyRecursive> = null}
test {
 fn empty_recursive_snapshot(value:atomic<EmptyRecursive>)->atomic<EmptyRecursive> => pass_through(value)
 test recursive_leaf_with_every_field_unset_is_present {
  let leaf=EmptyRecursive()
  let deep=EmptyRecursive(next:EmptyRecursive(next:leaf))
  assert eval(empty_recursive_snapshot,[leaf,_,deep,leaf,deep]) == [leaf,_,deep,EmptyRecursive(next:null),deep]
 }
}".into()),
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
fn shared_recursive_cases_typecheck() {
    let result = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/recursive_atomic_values.hgl"
    )));
    assert!(result.is_ok(), "{result:?}");
}
#[test]
fn shared_recursive_cases_execute_without_tick_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    run_shared(true)
}
fn run_shared(measure: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-recursive-{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(dir.join("src"))?;
    let suite = compile_tests(&sources(include_str!(
        "../../../external/hgraph_std/hgl/hgraph/tests/recursive_atomic_values.hgl"
    )))?;
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
    eprintln!("recursive measured cycles={cycles} tick_allocations={total}");
    assert_eq!(total,0,"recursive first/repeated publication and recording must allocate nothing");
    Ok(cycles)
}
"#);
    }
    code.push_str("struct Provider;\n");
    fs::write(dir.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"recursive-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, &dir))?;
    for profile in [vec![], vec!["--release"]] {
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
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &std::path::Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
