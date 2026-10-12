//! External users opt into Source spelling parsing; Model alone cannot parse source text.
use std::{fs, path::PathBuf, process::Command, time::SystemTime};

#[test]
fn external_consumers_migrate_parsing_to_source() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-source-api-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src"))?;
    let check = |dependency: &str,
                 source: &str|
     -> Result<std::process::Output, Box<dyn std::error::Error>> {
        fs::write(
            directory.join("Cargo.toml"),
            format!(
                "[package]\nname=\"source-api-consumer\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n{dependency}={{path={:?}}}\n",
                root.join("crates").join(dependency).to_string_lossy()
            ),
        )?;
        fs::write(directory.join("src/main.rs"), source)?;
        Ok(Command::new(env!("CARGO"))
            .args(["check", "--offline", "--quiet", "--target-dir"])
            .arg(root.join("target/source-api-consumer"))
            .current_dir(&directory)
            .output()?)
    };
    let source = check(
        "hgl-source",
        r#"
        use hgl_source::{Ty, TypeSpelling};
        fn main() {
            assert_eq!(Ty::parse("i64"), Some(Ty::I64));
            assert_eq!(Ty::list_parts("list<i64,2>"), Some(("i64",Some(2))));
            assert!(hgl_source::literals::numeric("7",false).is_ok());
        }
    "#,
    )?;
    assert!(
        source.status.success(),
        "{}",
        String::from_utf8_lossy(&source.stderr)
    );
    let model = check(
        "hgl-value-model",
        r"
        use hgl_value_model::{Ty, Literal, ir::{Kind,Value}};
        fn main() { let _ = Value::new(Ty::I64,Kind::Literal(Literal::Int(7))); }
    ",
    )?;
    assert!(
        model.status.success(),
        "{}",
        String::from_utf8_lossy(&model.stderr)
    );
    let parsing = check(
        "hgl-value-model",
        r#"
        use hgl_value_model::Ty;
        fn main() {
            let _ = Ty::parse("i64");
            let _ = hgl_value_model::literals::numeric("7",false);
        }
    "#,
    )?;
    assert!(!parsing.status.success(), "Model exposed source parsers");
    let errors = String::from_utf8_lossy(&parsing.stderr);
    assert!(
        errors.contains("E0599") && errors.contains("parse"),
        "{errors}"
    );
    assert!(
        errors.contains("E0425") && errors.contains("numeric"),
        "{errors}"
    );
    fs::remove_dir_all(directory)?;
    Ok(())
}
