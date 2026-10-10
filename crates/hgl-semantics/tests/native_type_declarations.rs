//! Opaque native declarations use the ordinary module type namespace.
use hgl_semantics::{library, name_check, source_check, struct_names};

fn sources(declaration: &str, consumer: &str) -> Vec<(String, String)> {
    vec![
        (
            "provider.hgl".into(),
            format!("module provider\n{declaration}\n"),
        ),
        (
            "consumer.hgl".into(),
            format!("module consumer\n{consumer}\n"),
        ),
        ("provider.hgl-native".into(),
            "native_descriptor 1 target \"hgl-rust-1\"\nmodule provider\nprovider \"test-provider\" fingerprint \"test-v1\"\nscalar Token canonical \"existing.Token\" capabilities {copy,text,equality,hash,order,serialization}\n".into()),
    ]
}

#[test]
fn explicit_native_types_support_selective_and_qualified_imports() -> Result<(), String> {
    for (import, name) in [
        ("use provider::{Token}", "Token"),
        ("use provider as native", "native::Token"),
    ] {
        let library = library::load(&sources("export native type Token", import))?;
        let declaration = struct_names::native_declaration(&library, "consumer", name)?
            .ok_or("native type was not indexed")?;
        assert_eq!(declaration.role, library::Role::NativeType);
        declaration.native_type().map_err(String::from)?;
        name_check::type_name(
            &library,
            "consumer",
            name,
            &name_check::Scope::default(),
            None,
        )
        .map_err(String::from)?;
        assert!(!name_check::visible(
            &library,
            declaration,
            "provider",
            "Token",
            None
        ));
    }
    Ok(())
}

#[test]
fn native_types_expose_no_fields_inheritance_arguments_or_constructors() -> Result<(), String> {
    for suffix in [" {}", " { value: str }", "<T>", " : Other", " = str"] {
        let diagnostics =
            source_check::validate_sources(&sources(&format!("native type Token{suffix}"), ""));
        assert_eq!(diagnostics.len(), 1, "{suffix}: {diagnostics:?}");
        assert_eq!(diagnostics[0].issue.code, Some("syntax.expected_token"));
    }
    let library = library::load(&sources(
        "export native type Token",
        "use provider::{Token}",
    ))?;
    assert!(struct_names::native_declaration(&library, "consumer", "Token<i64>").is_err());
    let errors = source_check::validate_sources(&sources(
        "export native type Token",
        "use provider::{Token}\nconst fn make() -> Token => Token()",
    ));
    assert!(
        errors
            .iter()
            .any(|error| error.issue.message.contains("unknown callable Token"))
    );
    Ok(())
}

#[test]
fn private_duplicate_and_test_context_native_types_are_rejected() -> Result<(), String> {
    let private = library::load(&sources("native type Token", "use provider::{Token}"))?;
    assert!(struct_names::native_declaration(&private, "consumer", "Token").is_err());
    assert!(library::load(&sources("native type Token\nnative type Token", "")).is_err());
    assert!(library::load(&sources("test { native type Token }", "")).is_err());
    Ok(())
}
