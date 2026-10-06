//! Emitted sparse publication, independent retained observations and timed effects.
use hgl_program::{compile, compile_tests, emit_rust, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[test]
fn structural_values_and_nested_generator_effects_execute_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-structural-{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(dir.join("src"))?;
    direct_images(&dir)?;
    positive_images(&dir)?;
    let calls = failure_images(&dir)?;
    let modules = (0..12)
        .map(|i| format!("mod failure{i};\n"))
        .collect::<Vec<_>>()
        .concat();
    let runner = include_str!("fixtures/structural_runner.rs")
        .replace("fn main() {", &format!("{modules}fn main() {{ {calls}"));
    fs::write(dir.join("src/main.rs"), runner)?;
    let mut manifest = String::from(
        "[package]\nname=\"structural-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
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

fn failure_images(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut calls = String::new();
    for (index, shape, slots, position) in [
        (11, "list<i64,0>", "[delta<list<i64,0>>() ]", 0),
        (
            0,
            "set<i64>",
            "[delta<set<i64>>(added:[1]),_,delta<set<i64>>(added:[1])]",
            2,
        ),
        (1, "set<bool>", "[_,delta<set<bool>>(removed:[false])]", 1),
        (2, "map<i64,i64>", "[delta<map<i64,i64>>(remove:[7])]", 0),
        (
            3,
            "list<set<i64>,2>",
            "[delta<list<set<i64>,2>>(items:[0:delta<set<i64>>()])]",
            0,
        ),
        (
            4,
            "map<i64,list<i64,2>>",
            "[delta<map<i64,list<i64,2>>>(upsert:[7:delta<list<i64,2>>()])]",
            0,
        ),
        (
            5,
            "map<i64,set<i64>>",
            "[delta<map<i64,set<i64>>>(upsert:[7:delta<set<i64>>(added:[1])]),delta<map<i64,set<i64>>>(remove:[7]),delta<map<i64,set<i64>>>(upsert:[7:delta<set<i64>>(removed:[1])])]",
            2,
        ),
    ] {
        let source = format!(
            "module failure{index}\nnative const fn count_start(value:i64) throws\nnative const fn count_start(value:i64) throws {{}}\nfn target(value:{shape})->{shape} {{start {{count_start(1)}}\nwhen {{return delta_value(value)}}}}\ntest fails {{eval(target,value:{slots})}}"
        );
        let suite = compile_tests(&with_std(source))?;
        let mut code = emit_tests(&suite);
        let diagnostic = format!(
            "eval: input delta outside publication profile: value at position {position}:{}",
            if index == 11 {
                " empty structural publication"
            } else {
                ""
            }
        );
        writeln!(
            code,
            "pub fn verify() {{crate::STARTS.store(0,std::sync::atomic::Ordering::SeqCst); let error=test0::test().unwrap_err(); assert!(error.contains({diagnostic:?}),\"{{error}}\"); assert_eq!(crate::STARTS.load(std::sync::atomic::Ordering::SeqCst),0);}}"
        )?;
        fs::write(dir.join(format!("src/failure{index}.rs")), code)?;
        writeln!(calls, "failure{index}::verify();")?;
    }
    for (index, expected) in [
        (
            6,
            "[delta<list<i64,2>>(items:[0:1]),delta<list<i64,2>>(items:[0:3])]",
        ),
        (
            7,
            "[delta<list<i64,2>>(items:[0:1,1:2]),delta<list<i64,2>>(items:[0:3,1:2])]",
        ),
    ] {
        let source = format!(
            "module failure{index}\nfn target(value:list<i64,2>)->list<i64,2> {{when {{return delta_value(value)}}}}\ntest mismatch {{assert eval(target,[delta<list<i64,2>>(items:[0:1,1:2]),delta<list<i64,2>>(items:[0:3])])=={expected}}}"
        );
        let suite = compile_tests(&with_std(source))?;
        let mut code = emit_tests(&suite);
        writeln!(
            code,
            "pub fn verify() {{let error=test0::test().unwrap_err(); assert!(error.contains(\"payload or presence differs\"),\"{{error}}\");}}"
        )?;
        fs::write(dir.join(format!("src/failure{index}.rs")), code)?;
        writeln!(calls, "failure{index}::verify();")?;
    }
    calls.push_str(&runtime_failure_images(dir)?);
    Ok(calls)
}
fn with_std(source: String) -> Vec<(String, String)> {
    vec![
        ("case.hgl".into(), source),
        (
            "std.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "std-impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}

fn direct_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (module, source) in [
        ("values", include_str!("fixtures/structural_values.hgl")),
        ("effects", include_str!("fixtures/structural_effects.hgl")),
    ] {
        let plan = compile(&[(format!("{module}.hgl"), source.into())], "main")?;
        let mut code = emit_rust(&plan);
        code.push_str("pub fn verify(store:&mut hgl_store::Store) {\n");
        let keys: &[&str] = if module == "values" {
            &[
                "fixed",
                "tupled",
                "quoted",
                "membership",
                "nested",
                "scalar8",
                "empty",
                "repeated",
            ]
        } else {
            &["effects"]
        };
        for key in keys {
            let suffix = format!(">({key:?})?");
            let marker = code
                .lines()
                .find_map(|line| {
                    line.split_once("ports.global::<")
                        .and_then(|(_, rest)| rest.split_once(&suffix).map(|(marker, _)| marker))
                })
                .ok_or("missing generated typed recording handle")?
                .to_owned();
            writeln!(
                code,
                "let {key}_handle=store.global_state().bind::<{marker}>({key:?}).unwrap(); let {key}=store.global_state().get({key}_handle).unwrap();"
            )?;
        }
        code.push_str(if module=="values" {include_str!("fixtures/structural_values_verify.rs")}else{
            "assert_eq!(effects.len(),2); assert_eq!(effects[0].0.micros(),5); assert_eq!(effects[1].0.micros(),6); assert_eq!(effects[0].1,(vec![(vec![3],vec![2])],vec![1])); assert_eq!(effects[1].1,(vec![(Vec::<i64>::new(),vec![4])],Vec::<i64>::new()));"
        });
        code.push_str("}\n");
        fs::write(dir.join(format!("src/{module}.rs")), code)?;
    }
    Ok(())
}
fn positive_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let suite = compile_tests(&[
        (
            "atomic_edges.hgl".into(),
            include_str!("fixtures/atomic_edges.hgl").into(),
        ),
        (
            "structural_evals.hgl".into(),
            include_str!("fixtures/structural_evals.hgl").into(),
        ),
        (
            "ordinary-delta-types.hgl".into(),
            include_str!(
                "../../../external/hgraph_spec/language/examples/ordinary-delta-types.hgl"
            )
            .into(),
        ),
        (
            "std.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "std-impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ])?;
    fs::write(
        dir.join("src/evaluations.rs"),
        emit_tests(&suite).replace("fn main() {", "pub fn main() {"),
    )?;
    Ok(())
}

fn runtime_failure_images(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut calls = String::new();
    for (index, shape, body, error) in [
        (
            8,
            "set<i64>",
            "yield 0us:delta<set<i64>>(added:[1])\nyield 1us:delta<set<i64>>(added:[1])",
            "noncanonical set addition",
        ),
        (
            9,
            "set<bool>",
            "yield 0us:delta<set<bool>>(removed:[false])",
            "noncanonical set removal",
        ),
        (
            10,
            "map<i64,i64>",
            "yield 0us:delta<map<i64,i64>>(remove:[7])",
            "noncanonical map removal",
        ),
    ] {
        let source = format!("module failure{index}\nexport fn main()->{shape} {{{body}}}");
        let mut code = emit_rust(&compile(&[("runtime.hgl".into(), source)], "main")?);
        writeln!(
            code,
            "pub fn verify() {{let mut registry=hgl_describe::Registry::new(); register(&mut registry).unwrap(); let description=main(&registry).unwrap(); let mut store=hgl_store::Store::new();let mut graph=hgl_describe::instantiate(&description,&registry,&mut store).unwrap();let error=hgl_kernel::run_simulation(&mut graph,&mut store,&hgl_kernel::RunConfig{{start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::from_micros(10)}}).unwrap_err();assert!(format!(\"{{error:?}}\").contains({error:?}));}}"
        )?;
        fs::write(dir.join(format!("src/failure{index}.rs")), code)?;
        writeln!(calls, "failure{index}::verify();")?;
    }
    Ok(calls)
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
