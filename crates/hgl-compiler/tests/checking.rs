//! Source and selected-part checking contracts.
use hgl_compiler::{Source, check, emit_rust};

const PROGRAM: &str = include_str!("../../../examples/const-debug/main.hgl");
const PART: &str = include_str!("../../../examples/const-debug/rust.hgl");

fn sources(program: &str, part: &str) -> Vec<Source> {
    vec![
        Source {
            name: "main.hgl".into(),
            text: program.into(),
        },
        Source {
            name: "rust.hgl".into(),
            text: part.into(),
        },
    ]
}

#[test]
fn complete_parts_resolve_in_either_order() -> Result<(), String> {
    let mut input = sources(PROGRAM, PART);
    let normal = check(&input).map_err(|e| format!("{e:?}"))?;
    input.reverse();
    let reversed = check(&input).map_err(|e| format!("{e:?}"))?;
    assert_eq!(emit_rust(&normal), emit_rust(&reversed));
    assert!(
        emit_rust(&normal)
            .map_err(|e| format!("{e:?}"))?
            .contains("fn r#print_i64(r#value: i64)")
    );
    Ok(())
}

#[test]
fn malformed_and_unsupported_programs_fail_before_emission() {
    for (old, new, message) in [
        ("const_(42)", "missing(42)", "unknown function"),
        ("const_(42)", "const_()", "argument count"),
        ("debug_print(value)", "debug_print(42)", "scalar/temporal"),
        ("debug_print(value)", "const_(value)", "scalar/temporal"),
        ("print_i64(value)", "const_(value)", "node handlers"),
        ("return value", "return missing", "unknown value"),
        ("inject scheduler", "", "scheduler access"),
        ("42", "9223372036854775808", "invalid i64"),
        ("42", "true", "non-reserved"),
        ("42", "1 + 2", "unsupported token"),
        ("const_(42)", "main()", "recursive"),
        (
            "let value = const_(42)",
            "let const_ = 42\n    let value = const_(42)",
            "shadowed",
        ),
        ("fn const_", "fn const", "non-reserved"),
        (
            "fn debug_print(value: i64)",
            "fn debug_print(value: i64, value: i64)",
            "duplicate parameter",
        ),
    ] {
        let changed = PROGRAM.replacen(old, new, 1);
        let result = check(&sources(&changed, PART));
        let errors = result.err().unwrap_or_default();
        assert!(
            errors.iter().any(|e| e.message.contains(message)),
            "{old} -> {new}: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .all(|e| e.start <= e.end && e.end <= changed.len())
        );
    }
}

#[test]
fn selected_native_parts_must_match_and_be_unique() {
    for (part, message) in [
        (PART.replace("value: i64", "other: i64"), "signature"),
        (PART.replace("value: i64", "const value: i64"), "signature"),
        (PART.replace(") {}", ") -> i64 {}"), "signature"),
        (PART.replace("examples.const_debug", "other"), "same module"),
        (
            format!("{PART}\nnative const fn print_i64(value: i64) {{}}\n"),
            "duplicate native implementation",
        ),
    ] {
        let errors = check(&sources(PROGRAM, &part)).err().unwrap_or_default();
        assert!(
            errors.iter().any(|e| e.message.contains(message)),
            "{errors:?}"
        );
    }
    let mut input = sources(PROGRAM, PART);
    input.pop();
    let result = check(&input).and_then(|checked| emit_rust(&checked));
    let errors = result.err().unwrap_or_default();
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("selected implementation"))
    );
}

#[test]
fn comments_crlf_and_i64_min_are_source_faithful() -> Result<(), String> {
    let input = PROGRAM
        .replace("42", "-9223372036854775808")
        .replace('\n', "\r\n");
    check(&sources(
        &format!("# comment\r\n/* block */\r\n{input}"),
        PART,
    ))
    .map_err(|e| format!("{e:?}"))?;
    for bad in [format!("{PROGRAM}\n/* missing"), format!("{PROGRAM}\n☃")] {
        let errors = check(&sources(&bad, PART)).err().unwrap_or_default();
        assert!(!errors.is_empty());
        for error in errors {
            assert!(bad.is_char_boundary(error.start));
            assert!(bad.is_char_boundary(error.end));
        }
    }
    Ok(())
}

#[test]
fn unrepresentable_public_rust_names_are_emission_errors() -> Result<(), String> {
    for (program, part) in [
        (
            PROGRAM.replace("print_i64", "self"),
            PART.replace("print_i64", "self"),
        ),
        (PROGRAM.replace("fn main", "fn register"), PART.into()),
    ] {
        let checked = check(&sources(&program, &part)).map_err(|errors| format!("{errors:?}"))?;
        assert!(emit_rust(&checked).is_err());
    }
    Ok(())
}
