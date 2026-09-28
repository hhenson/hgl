//! Source documentation attachment and backend preservation.
use hgl_compiler::{Source, check, emit_documentation, emit_rust};

fn source(text: &str) -> Source {
    Source {
        name: "docs.hgl".into(),
        text: text.into(),
    }
}

#[test]
fn documents_survive_checking_and_rust_emission() -> Result<(), Vec<hgl_compiler::Diagnostic>> {
    let shared = source(
        r"/** Module α. */
module docs
/**
Print a value.

Args:
    value: Input value: unmodified.

Notes:
    .. math::

        y = \frac{x}{2}

    .. mermaid::

        sequenceDiagram
            Node->>Output: Publish
*/
native const fn print(value: i64)
",
    );
    let implementation = source(
        "module docs part rust_impl\n/** Rust provider. */\nnative const fn print(value: i64) {}\n",
    );
    let checked = check(&[shared, implementation])?;
    let docs = checked.documentation();
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[1].name, "docs.print");
    assert_eq!(docs[1].part, "");
    assert_eq!(docs[2].part, "rust_impl");
    assert_eq!(docs[1].declaration, "native const fn print(value: i64)");
    assert!(docs[1].text.contains("        y = \\frac{x}{2}"));
    let generated = emit_rust(&checked)?;
    assert!(generated.contains("// Print a value."));
    assert!(generated.contains("// Rust provider."));
    let rst = emit_documentation(&checked);
    assert!(rst.contains(".. math::\n\n    y = \\frac{x}{2}"));
    assert!(rst.contains(".. mermaid::\n\n    sequenceDiagram"));
    Ok(())
}

#[test]
fn invalid_attachment_and_keys_are_diagnostics() {
    for text in [
        "module docs\n/** Orphan. */",
        "module docs\n/** First. */\n/** Second. */\nfn f() {}",
        "module docs\n/** Broken. */\n# barrier\nfn f() {}",
        "module docs\nfn f() {\n/** Not a declaration. */\n}\nfn g() {}",
        "module docs\n/**\nArgs:\n    missing: Wrong.\n*/\nfn f() {}",
        "module docs\n/**\nType Args:\n    T: Wrong.\n*/\nfn f() {}",
        "module docs\n/**\nProperties:\n    <i64>: Wrong.\n*/\nfn f() {}",
        "module docs\n/**\nRequires:\n    native::f(i64): Wrong.\n*/\nfn f() {}",
    ] {
        assert!(check(&[source(text)]).is_err(), "{text}");
    }
}

#[test]
fn ordinary_comments_and_crlf_are_handled() -> Result<(), Vec<hgl_compiler::Diagnostic>> {
    let checked = check(&[source("module docs\n/**/\n/* Ordinary. */\nfn f() {}\n")])?;
    assert!(checked.documentation().is_empty());
    let checked = check(&[source(
        "module docs\r\n    /** Same line.\r\n\r\n    Notes:\r\n        Nested.\r\n    */\r\nfn f() {}\r\n",
    )])?;
    assert_eq!(
        checked.documentation()[0].text,
        "Same line.\n\nNotes:\n    Nested."
    );
    Ok(())
}

#[test]
fn a_function_can_share_its_modules_name() -> Result<(), Vec<hgl_compiler::Diagnostic>> {
    let checked = check(&[source(
        "/** Module. */\nmodule docs\n/** Function. */\nfn docs() {}",
    )])?;
    assert_eq!(checked.documentation()[0].name, "docs");
    assert_eq!(checked.documentation()[1].name, "docs.docs");
    Ok(())
}

#[test]
fn all_lexer_line_endings_preserve_sections_and_validate_keys()
-> Result<(), Vec<hgl_compiler::Diagnostic>> {
    let text = "module docs\n/**\nPreserve sections.\n\nArgs:\n    value: Input.\n\nNotes:\n    .. math::\n\n        y = x\n*/\nfn f(value: i64) -> i64 { return value }\n";
    let expected = check(&[source(text)])?;
    for ending in ["\n", "\r\n", "\r"] {
        let checked = check(&[source(&text.replace('\n', ending))])?;
        assert_eq!(
            checked.documentation()[0].text,
            expected.documentation()[0].text
        );
        assert_eq!(emit_documentation(&checked), emit_documentation(&expected));
        assert_eq!(emit_rust(&checked)?, emit_rust(&expected)?);
        let invalid = text
            .replace("value: Input.", "missing: Input.")
            .replace('\n', ending);
        let diagnostics = check(&[source(&invalid)]).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("unknown key 'missing'"))
        );
    }
    Ok(())
}

#[test]
fn multibyte_leading_text_is_not_sliced_at_a_byte_offset()
-> Result<(), Vec<hgl_compiler::Diagnostic>> {
    let checked = check(&[source("module docs\n/**\n a\n　b\n*/\nfn f() {}")])?;
    assert_eq!(checked.documentation()[0].text, " a\n　b");
    Ok(())
}
