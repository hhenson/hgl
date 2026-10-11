//! Each module owns a different compiled layout table; retained boxes cross that boundary.
use std::{fmt::Write as _, fs, process::Command};
#[test]
fn independently_emitted_any_descriptors_survive_nested_retention()
-> Result<(), Box<dyn std::error::Error>> {
    let producer = emit("module producer\nfn main()->any {when {return any((1,1.0))}}")?;
    let consumer = emit(
        "module consumer\nstruct Holder {value:any}\nfn main()->any {when {return any(Holder(value:any(true)))}}",
    )?;
    let holder = hgl_source::Ty::Struct(
        "consumer::Holder".into(),
        vec![("value".into(), hgl_source::Ty::Any)],
        vec![],
    );
    let wrap = hgl_rust::any_values::construct(&holder, "(value,)");
    let mut holder_name = String::new();
    for byte in holder.source_name().bytes() {
        write!(holder_name, "{byte:03}")?;
    }
    let integer = hgl_rust::any_values::construct(&hgl_source::Ty::I64, "value");
    run(&format!(
        r#"
mod producer {{ {producer} pub fn make(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue> {{Ok({integer})}} }}
mod consumer {{ {consumer}
pub fn check(value:hgl_types::AnyValue)->hgl_types::NodeResult {{
 let expected=value.retain()?;let boxed={wrap};
 let retained=boxed.retain()?;drop(boxed);
 let decoded=any_read_{holder_name}(retained.data()).0;
 assert!(decoded.equal(&expected)?,"foreign descriptor was replaced by a consumer-local table entry");
 assert_eq!(decoded.key_hash()?,expected.key_hash()?);
 assert_eq!(decoded.compare(&expected)?,Some(std::cmp::Ordering::Equal));
 assert!(!decoded.equal(&crate::producer::make(2)?)?);
 assert_eq!(decoded.compare(&crate::producer::make(2)?)?,Some(std::cmp::Ordering::Less));
 let captured=encode_any(&retained).map_err(hgl_types::NodeError::new)?;
 let payload=hgl_semantics::any_values::contents(&captured).unwrap();
 assert_eq!(payload.ty.source_name(),"consumer::Holder");
 let hgl_semantics::ir::Kind::Construct(fields)=&payload.kind else {{panic!("retained fields")}};
 assert_eq!(hgl_semantics::any_values::contents(&fields[0].1).unwrap().ty,hgl_source::Ty::I64);
 let expected_outer=retained.retain()?;let value=retained.retain()?;let nested={wrap};
 let nested_saved=nested.retain()?;drop(nested);drop(retained);
 let outer=any_read_{holder_name}(nested_saved.data()).0;
 assert!(outer.equal(&expected_outer)?);
 let inner=any_read_{holder_name}(outer.data()).0;
 assert!(inner.equal(&expected)?);
 Ok(())
}}
}}
fn main(){{consumer::check(producer::make(1).unwrap()).unwrap();}}
"#
    ))
}
#[test]
fn native_returned_boxes_use_the_producers_cold_capture() -> Result<(), Box<dyn std::error::Error>>
{
    let producer = emit("module producer\nfn main()->any {when {return any((1,1.0))}}")?;
    let integer = hgl_rust::any_values::construct(&hgl_source::Ty::I64, "value");
    let source = "module hgraph.std part foreign_boxes\nstruct Holder {value:any}\nnative const fn foreign_box(value:i64)->any throws\nnative const fn foreign_box(value:i64)->any throws {}\nfn target(value:i64)->any {when {return any(Holder(value:foreign_box(value)))}}\ntest imported_box {assert eval(target,[1,2])==[any(Holder(value:any(1))),any(Holder(value:any(2)))]}";
    let sources = vec![
        ("consumer.hgl".into(), source.into()),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ];
    let generated = hgl_program::emit_tests(&hgl_program::compile_tests(&sources)?);
    run(&format!(
        "mod producer {{{producer} pub fn make(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({integer})}}}}struct Provider;mod native {{pub fn foreign_box_i64(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue>{{crate::producer::make(value)}}}}{generated}"
    ))
}
#[test]
fn shared_nominal_declarations_have_one_identity_across_emitted_modules()
-> Result<(), Box<dyn std::error::Error>> {
    let shared =
        "module shared\nexport struct Record<T> {value:T}\nexport struct Other {value:i64}";
    let compile = |source: &str| -> Result<String, String> {
        Ok(hgl_program::emit_rust(&hgl_program::compile(
            &[
                ("main.hgl".into(), source.into()),
                ("shared.hgl".into(), shared.into()),
            ],
            "main",
        )?))
    };
    let producer = compile(
        "module producer\nuse shared::{Record}\nfn main()->any {when {return any(Record<i64>(value:1))}}",
    )?;
    let consumer = compile(
        "module consumer\nuse shared::{Record,Other}\nfn main()->any {when {return any((Record<atomic<i64>>(value:1),Record<bool>(value:true),Other(value:1),true))}}",
    )?;
    let nominal = |name: &str| {
        hgl_source::Ty::Struct(
            format!("shared::{name}").into(),
            vec![("value".into(), hgl_source::Ty::I64)],
            vec![],
        )
    };
    let record = hgl_rust::any_values::construct(&nominal("Record<i64>"), "(value,)");
    let other = hgl_rust::any_values::construct(&nominal("Other"), "(value,)");
    run(&format!(
        r#"
mod producer {{{producer} pub fn make(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({record})}}}}
mod consumer {{{consumer} pub fn make(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({record})}} pub fn other(value:i64)->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({other})}}}}
fn main(){{
 let left=producer::make(1).unwrap();let right=consumer::make(1).unwrap();
 assert!(left.equal(&right).unwrap(),"same imported nominal declaration acquired different identities");
 assert_eq!(left.key_hash().unwrap(),right.key_hash().unwrap());
 assert_eq!(left.compare(&right).unwrap(),Some(std::cmp::Ordering::Equal));
 assert_eq!(left.compare(&consumer::make(2).unwrap()).unwrap(),Some(std::cmp::Ordering::Less));
 assert!(!left.equal(&consumer::other(1).unwrap()).unwrap());
 assert_eq!(left.compare(&consumer::other(1).unwrap()).unwrap(),None);
}}
"#
    ))
}
#[test]
fn exact_identity_preserves_generic_arguments_lengths_and_normalization()
-> Result<(), Box<dyn std::error::Error>> {
    use hgl_source::Ty;
    let names = [
        "identity::Name",
        "identity::Name\0",
        "identity::Name\0\0",
        "identity::Generic<i64>",
        "identity::Generic<bool>",
        "identity::Καλημέρα",
    ];
    let mut types: Vec<_> = names
        .iter()
        .map(|name| Ty::Struct((*name).into(), vec![], vec![]))
        .collect();
    types.extend([Ty::I64, Ty::I64.atomic(), Ty::Any]);
    let plan = hgl_rust::Plan {
        ordinary_types: types.clone(),
        ..Default::default()
    };
    let generated = hgl_rust::emit(&plan);
    let mut checks = String::new();
    for (index, ty) in types[..8].iter().enumerate() {
        let mut name = String::new();
        for byte in ty.source_name().bytes() {
            write!(name, "{byte:03}")?;
        }
        writeln!(checks, "let id{index}=(ANY_{name}.identity)();")?;
    }
    run(&format!(
        "mod generated{{{generated}pub fn check(){{{checks}let distinct=[id0,id1,id2,id3,id4,id5,id6];for(i,left)in distinct.iter().enumerate(){{for right in &distinct[i+1..]{{assert_ne!(left,right);}}}}assert_eq!(id6,id7);}}}}fn main(){{generated::check();}}"
    ))
}
#[test]
fn long_source_names_keep_balanced_shared_identity() -> Result<(), Box<dyn std::error::Error>> {
    let name = "Long".repeat(1024);
    let source = format!(
        "module long_identity\nstruct {name} {{value:i64}}\nfn main()->any {{when {{return any({name}(value:1))}}}}"
    );
    let emitted = emit(&source)?;
    let ty = hgl_source::Ty::Struct(
        format!("long_identity::{name}").into(),
        vec![("value".into(), hgl_source::Ty::I64)],
        vec![],
    );
    let value = hgl_rust::any_values::construct(&ty, "(1_i64,)");
    run(&format!(
        "mod first{{{emitted}pub fn make()->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({value})}}}}mod second{{{emitted}pub fn make()->hgl_types::NodeResult<hgl_types::AnyValue>{{Ok({value})}}}}fn main(){{let a=first::make().unwrap();let b=second::make().unwrap();assert!(a.equal(&b).unwrap());assert_eq!(a.key_hash().unwrap(),b.key_hash().unwrap());}}"
    ))
}
fn emit(source: &str) -> Result<String, String> {
    Ok(hgl_program::emit_rust(&hgl_program::compile(
        &[("module.hgl".into(), source.into())],
        "main",
    )?))
}
fn run(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-cross-any-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let package = directory
        .file_name()
        .ok_or("generated package name")?
        .to_string_lossy();
    let mut manifest = format!(
        "[package]\nname=\"{package}\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n"
    );
    for name in [
        "hgl-types",
        "hgl-source",
        "hgl-semantics",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-testkit",
        "hgl-stdlib",
    ] {
        writeln!(
            manifest,
            "{name}={{path=\"{}\"}}",
            root.join("crates")
                .join(name)
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        )?;
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    fs::write(directory.join("src/main.rs"), source)?;

    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .args(if cfg!(debug_assertions) {
            &[][..]
        } else {
            &["--release"][..]
        })
        .arg("--target-dir")
        .arg(root.join("target/source-tests"))
        .current_dir(&directory)
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory)?;
    Ok(())
}
