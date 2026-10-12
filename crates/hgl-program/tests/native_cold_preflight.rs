//! Fresh provider results must pass fallible ownership validation before cold capture.
use std::{fmt::Write as _, fs, process::Command, time::SystemTime};

#[test]
fn native_cold_result_preflight_preserves_provider_error_and_once_only_execution()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-native-cold-preflight-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let provider = fs::read_to_string(root.join("native/stdlib/atomic_values.rs"))?
        .replacen(
            "if data.len() > capacity {",
            "if data == b\"bad-copy\" { return Err(NodeError::coded(\"provider copy rejected\",\"native.copy.failure\")); } if data.len() > capacity {",
            1,
        )
        .replacen(
            "pub(crate) fn token_str(text: &str) -> NativeView<'_, Token> {",
            "pub(crate) fn token_str(text: &str) -> NativeView<'_, Token> { crate::CALLS.fetch_add(1,std::sync::atomic::Ordering::Relaxed);",
            1,
        );
    fs::write(directory.join("provider.rs"), provider)?;
    let mut sources = hgl_program::library_files::sources(
        &[
            root.join("external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl"),
            root.join("native/stdlib/atomic_values.hgl-native"),
        ],
        &[],
    )?;
    sources.insert(0, ("cold.hgl".into(), "module cold\nuse hgraph.std::{token}\ntest ownership {let bad=token(\"bad-copy\")\nlet empty=token(\"\")\nlet good=token(\"fine\")}\n".into()));
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(name, _)| name.ends_with(".hgl-native"))
        .ok_or("missing descriptor")?;
    *descriptor = descriptor.replace(
        "\"atomic_values.rs\"",
        &format!("{:?}", directory.join("provider.rs").to_string_lossy()),
    );
    let suite = hgl_program::compile_module_suite(&sources)?;
    let mut generated = hgl_program::emit_tests(&suite).replace("fn main()", "fn unused_suite()");
    generated.push_str("static CALLS:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);\nfn main(){let mut context=NativeContext(hgl_source::time_context::RunContext::from_bundled().unwrap());let mut evaluator=hgl_semantics::value_eval::Evaluator::default();");
    for (index, step) in suite.tests[0].steps.iter().enumerate() {
        let hgl_semantics::harness_ir::Step::Ordinary(hgl_semantics::ir::Statement::Let(_, value)) =
            step
        else {
            panic!("checked native initializer")
        };
        writeln!(
            generated,
            "let result=evaluator.value_in_context(&{},&mut context);",
            hgl_rust::checked_data::value(value)
        )?;
        if index == 0 {
            generated.push_str("assert!(matches!(result,Err(hgl_semantics::value_eval::EvalError::Coded(\"native.copy.failure\",ref message)) if message==\"provider copy rejected\"));");
        } else {
            generated.push_str("result.unwrap();");
        }
    }
    generated.push_str("assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed),3);}");
    fs::write(directory.join("src/main.rs"), generated)?;
    execute(&root, &directory)?;
    fs::remove_dir_all(directory)?;
    Ok(())
}

fn execute(
    root: &std::path::Path,
    directory: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = "[package]\nname=\"hgl-native-cold-preflight\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_owned();
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-source",
        "hgl-semantics",
        "hgl-testkit",
    ] {
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name).to_string_lossy()
        )?;
    }
    fs::write(directory.join("Cargo.toml"), manifest)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--offline", "--quiet"])
        .arg("--target-dir")
        .arg(root.join("target/source-tests"))
        .current_dir(directory);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        directory.display()
    );
    Ok(())
}
