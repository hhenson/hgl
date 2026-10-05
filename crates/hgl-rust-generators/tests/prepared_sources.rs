//! Generated source resumes from immutable slots without retaining pending owners.
use hgl_rust_generators::Generator;
use hgl_rust_ir::{Kind, Plan, Statement, Value};
use hgl_source::{Literal, Ty};
use std::{fmt::Write as _, fs, process::Command, time::SystemTime};
fn at(time: i64, payload: Value) -> Statement {
    Statement::TimedYield(
        Value::new(Ty::DateTime, Kind::Literal(Literal::DateTime(time))),
        payload,
    )
}
#[test]
fn emitted_pending_slots_execute_without_allocation_and_mixed_sources_keep_ordinary_values()
-> Result<(), Box<dyn std::error::Error>> {
    let text = Generator::lower(&[
        at(1, Value::new(Ty::Str, Kind::Configuration(0))),
        at(3, Value::new(Ty::Str, Kind::Configuration(0))),
    ]);
    let mixed = Generator::lower(&[
        at(1, Value::new(Ty::I64, Kind::Configuration(0))),
        at(2, Value::new(Ty::I64, Kind::Literal(Literal::Int(9)))),
        at(3, Value::new(Ty::I64, Kind::Configuration(0))),
    ]);
    let mut code = String::from(
        "#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;\n",
    );
    for (name, ty, generator, native, expected) in [
        (
            "Text",
            Ty::Str,
            text,
            "String::from(\"retained\")",
            "[\"retained\",\"retained\"]",
        ),
        ("Mixed", Ty::I64, mixed, "7_i64", "[7_i64,9,7]"),
    ] {
        let rust = if ty == Ty::Str { "String" } else { "i64" };
        writeln!(
            code,
            "struct {name} {{_output:hgl_store::Out<{rust}>,configuration_columns:hgl_store::ValueColumns,configuration_slot0:hgl_store::ValueSlot<{rust}>,{}}}",
            generator.fields(&ty)
        )?;
        writeln!(
            code,
            "impl hgl_kernel::Node for {name} {{fn start(&mut self,_ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult {{{} Ok(())}} fn eval(&mut self,_ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult {{{}}}}}",
            generator.start(),
            generator.evaluation(&Plan::default(), &ty)
        )?;
        writeln!(
            code,
            r#"fn run_{name}()->hgl_types::NodeResult {{
            use hgl_store::PreparedValue;
            let value={native}; let mut bounds=Default::default(); <{rust} as PreparedValue>::include(&mut bounds,&value);
            let mut configuration_columns=hgl_store::ValueColumns::default();
            let configuration_slot0=<{rust} as PreparedValue>::allocate(&mut configuration_columns,&bounds)?;
            <{rust} as PreparedValue>::check_native(&configuration_columns,configuration_slot0,&value)?;
            <{rust} as PreparedValue>::copy_native(&mut configuration_columns,configuration_slot0,&value);
            let mut store=hgl_store::Store::new(); let output=store.add_output::<{rust}>(hgl_types::NodeId(0));
            store.prepared().prepare_scalar::<{rust}>(output.id(),64)?;
            let node={name}{{_output:output,configuration_columns,configuration_slot0,{}}};
            let slot=hgl_kernel::NodeSlot {{node:Box::new(node),node_type:hgl_types::NodeType{{uses_scheduler:true,..Default::default()}},label:"prepared".into(),required:vec![]}};
            let mut graph=hgl_kernel::Graph::new("prepared".into(),vec![slot]); graph.start(&mut store,hgl_types::EngineTime::MIN_START)?;
            for expected in {expected} {{let now=graph.next_scheduled_time(); let (result,allocations)=hgl_alloc_count::count_in(||graph.evaluate(&mut store,now)); result?; assert_eq!(allocations,0); assert_eq!(store.output_ref(output),Some(&expected.into()));}}
            assert_eq!(graph.next_scheduled_time(),hgl_types::EngineTime::FOREVER); Ok(())
        }}
        "#,
            generator.initialize()
        )?;
    }
    code.push_str("fn main()->hgl_types::NodeResult {run_Text()?;run_Mixed()}\n");
    execute(&code)
}
fn execute(code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-prepared-generator-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"prepared-generator\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in ["hgl-store", "hgl-kernel", "hgl-types", "hgl-alloc-count"] {
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
        let result = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            .args(profile)
            .current_dir(&dir)
            .output()?;
        assert!(
            result.status.success(),
            "{}\n{}",
            dir.display(),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}
