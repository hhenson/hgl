//! Execute ordinary HGL hooks against a shared, typed run-owned store.
use hgl_program::{compile, emit_rust};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

const SOURCE: &str = r#"module shared
fn ticks()->i64 {
    inject alarm, clock
    cache count:i64=0
    start { schedule(alarm,0s) }
    when {
        count += 1
        if count < 2 { schedule_at(alarm,clock.next_cycle_evaluation_time) }
        return count
    }
}
fn increment<T>(value:T,const key:str)->i64 {
    inject global_state
    start { set(global_state,"started",true) }
    when {
        let count:i64=get(global_state,key)
        set(global_state,key,count+1)
        return count+1
    }
    stop {
        let count:i64=get(global_state,key)
        set(global_state,"stopped",count)
    }
}
fn observe(value:i64) {
    inject global_state
    when {
        let count:i64=get(global_state,"counter")
        set(global_state,"seen",count)
        set(global_state,"published",delta_value(value))
    }
}
fn text(value:i64) {
    inject global_state
    start {
        set(global_state,"text","")
        let original:str=get(global_state,"text")
        set(global_state,"text","changed")
        set(global_state,"snapshot",original)
    }
    when {}
}
export fn main() {
    let input=ticks()
    observe(increment(input,"counter"))
    text(input)
    scalars(input)
}
"#;

const RUNNER: &str = r#"
use hgl_describe::{Registry,instantiate_complete};
use hgl_kernel::{RunConfig,run_simulation};
use hgl_store::Store;
use hgl_types::{EngineTime,Date,Time,EngineDelta};
mod graph;
fn run(seed:i64) {
    let mut registry=Registry::new();
    graph::register(&mut registry).unwrap();
    let description=graph::main(&registry).unwrap();
    let mut wrong=Store::new();
    wrong.provision_global_state();
    let wrong_counter=wrong.bind_global::<bool>("counter").unwrap();
    wrong.global_set(wrong_counter,&false).unwrap();
    assert!(instantiate_complete(&description,&registry,&mut wrong).is_err());
    let started=wrong.bind_global::<bool>("started").unwrap();
    assert!(wrong.global_get(started).is_err());
    let mut store=Store::new();
    assert!(instantiate_complete(&description,&registry,&mut store).is_err());
    store.provision_global_state();
    let counter=store.bind_global::<i64>("counter").unwrap();
    store.global_set(counter,&seed).unwrap();
    let mut built=instantiate_complete(&description,&registry,&mut store).unwrap();
    run_simulation(&mut built.graph,&mut store,&RunConfig {
        start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(10)
    }).unwrap();
    assert_eq!(store.global_get(counter).unwrap(),seed+2);
    for name in ["seen","published","stopped"] {
        let entry=store.bind_global::<i64>(name).unwrap();
        assert_eq!(store.global_get(entry).unwrap(),seed+2);
    }
    let started=store.bind_global::<bool>("started").unwrap();
    assert!(store.global_get(started).unwrap());
    let snapshot=store.bind_global::<String>("snapshot").unwrap();
    assert_eq!(store.global_get(snapshot).unwrap(),"");
    let text=store.bind_global::<String>("text").unwrap();
    assert_eq!(store.global_get(text).unwrap(),"changed");
    scalar_assertions(&mut store);
}
fn main() { run(40); run(40); run(-2); missing_values(); }
"#;

fn scalar_sources() -> Result<(String, String), std::fmt::Error> {
    let mut source = String::from("fn scalars(value:i64) { inject global_state\nstart {\n");
    let mut reads = String::new();
    let mut checks = String::from("fn scalar_assertions(store:&mut Store) {\n");
    for (i, (ty, literal, rust, expected)) in [
        ("bool", "false", "bool", "false"),
        ("i64", "0", "i64", "0"),
        ("f64", "0.0", "f64", "0.0"),
        ("str", "\"\"", "String", "\"\""),
        ("date", "@1970-01-01", "Date", "Date(0)"),
        ("time", "@00:00:00", "Time", "Time(0)"),
        (
            "datetime",
            "@1970-01-01T00:00:00Z",
            "EngineTime",
            "EngineTime::from_micros(0)",
        ),
        (
            "duration",
            "0us",
            "EngineDelta",
            "EngineDelta::from_micros(0)",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        writeln!(source, "set(global_state,\"scalar{i}\",{literal})")?;
        writeln!(
            reads,
            "let value{i}:{ty}=get(global_state,\"scalar{i}\")\nset(global_state,\"copy{i}\",value{i})"
        )?;
        writeln!(
            checks,
            "let entry=store.bind_global::<{rust}>(\"copy{i}\").unwrap();\nassert_eq!(store.global_get(entry).unwrap(),{expected});"
        )?;
    }
    write!(source, "}}\nwhen {{ {reads} }}\n}}")?;
    checks.push_str("}\n");
    Ok((source, checks))
}

#[test]
fn generated_hgl_shares_typed_values_through_all_hooks_and_fresh_runs()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-global-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    let (scalars, checks) = scalar_sources()?;
    let plan = compile(
        &[("shared.hgl".into(), format!("{SOURCE}\n{scalars}"))],
        "main",
    )?;
    fs::write(dir.join("src/graph.rs"), emit_rust(&plan))?;
    let failures = failure_sources(&dir)?;
    fs::write(
        dir.join("src/main.rs"),
        format!("{RUNNER}\n{checks}\n{failures}"),
    )?;
    manifest(&root, &dir)?;
    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .current_dir(&dir)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"global-hook-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in ["hgl-types", "hgl-store", "hgl-kernel", "hgl-describe"] {
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

fn failure_sources(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut modules = String::new();
    let mut calls = String::new();
    for phase in ["start", "when", "stop"] {
        let name = format!("missing_{phase}");
        let hook = if phase == "when" {
            String::new()
        } else {
            "when {}".into()
        };
        let source = format!(
            "module missing\nfn source()->i64 {{ inject alarm\nstart {{ schedule(alarm,0s) }}\nwhen {{ return 1 }} }}\nfn sink(value:i64) {{ inject global_state\n{phase} {{ let missing:i64=get(global_state,\"absent_{phase}\") }}\n{hook} }}\nexport fn main() {{ sink(source()) }}"
        );
        let plan = compile(&[("missing.hgl".into(), source)], "main")?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
        writeln!(modules, "mod {name};")?;
        writeln!(
            calls,
            "let mut registry=Registry::new();\n{name}::register(&mut registry).unwrap();\nlet description={name}::main(&registry).unwrap();\nlet mut store=Store::new();\nstore.provision_global_state();\nlet mut built=instantiate_complete(&description,&registry,&mut store).unwrap();\nlet failure=run_simulation(&mut built.graph,&mut store,&RunConfig {{ start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(10) }}).unwrap_err();\nassert!(format!(\"{{failure:?}}\").contains(\"absent_{phase}\"));"
        )?;
    }
    Ok(format!("{modules}\nfn missing_values() {{ {calls} }}"))
}
