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

#[test]
fn native_nominal_resolution_normalizes_atomic_and_delta_without_structural_access()
-> Result<(), String> {
    let interface = "module native_nominal\nnative type Token\nnative type Alias\nnative type Other\nnative type TextOnly\n";
    let descriptor = "native_descriptor 1 target \"hgl-rust-1\"\nmodule native_nominal\nprovider \"fixture\" fingerprint \"v1\"\nscalar Token canonical \"nominal.Token\" capabilities {copy,text,equality,hash,order}\nscalar Alias canonical \"nominal.Token\" capabilities {copy,text,equality,hash,order}\nscalar Other canonical \"nominal.Other\" capabilities {copy,text,equality,hash,order}\nscalar TextOnly canonical \"nominal.TextOnly\" capabilities {copy,text}\n";
    let library = library::load(&[
        ("native.hgl".into(), interface.into()),
        ("native.hgl-native".into(), descriptor.into()),
    ])?;
    let resolve =
        |name: &str| hgl_semantics::value_types::resolve_ordinary(&library, "native_nominal", name);
    let token = resolve("Token")?;
    assert_eq!(token, resolve("Alias")?);
    assert_ne!(token, resolve("Other")?);
    assert_eq!(token, resolve("atomic<Token>")?);
    assert_eq!(token, resolve("delta<Token>")?);
    assert_eq!(token, resolve("delta<atomic<Token>>")?);
    assert!(token.atomic_payload() && token.publication() && token.collection_key());
    assert!(token.structure().is_err());
    assert!(resolve("Token<i64>").is_err());
    assert!(!resolve("TextOnly")?.collection_key());
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        assert_eq!(
            hgl_semantics::value_operators::binary_type(op, &token, &token)?,
            hgl_source::Ty::Bool
        );
        assert!(
            hgl_semantics::value_operators::binary_type(
                op,
                &resolve("TextOnly")?,
                &resolve("TextOnly")?
            )
            .is_err()
        );
        assert!(
            hgl_semantics::value_operators::binary_type(op, &token, &resolve("Other")?).is_err()
        );
    }
    assert!(hgl_semantics::value_operators::binary_type("+", &token, &token).is_err());
    Ok(())
}
