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
    let mut manifest = "[package]\nname=\"cross-any\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
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
