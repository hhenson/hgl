//! LIST-ERROR distinguishes constant checking, wiring construction and later type errors.
use hgl_program::{compile, compile_tests, emit_rust};

const BOUNDS_CASES: &[&str] = &[
    "let values:list<i64> =[]\nreturn values[0]",
    "let values:list<i64> =[1,2]\nreturn values[-1]",
    "let values:list<i64> =[1,2]\nreturn values[2]",
    "let values:list<i64,2> =[1,2]\nreturn values[2]",
    "let values:list<list<i64>> =[]\nreturn len(values[0])",
    "var values:list<i64> =[]\npush(values,7)\nreturn values[1]",
];

fn helper(body: &str) -> String {
    format!("module list_errors\nconst fn fail()->i64 {{{body}}}\n")
}

#[test]
fn evaluated_constant_list_bounds_fail_during_checking() {
    for body in BOUNDS_CASES {
        let source = format!("{}test bounds {{assert fail()==0}}", helper(body));
        let error = compile_tests(&[("list_errors.hgl".into(), source)])
            .expect_err("constant bounds must fail checking");
        assert!(error.contains("constant evaluation"), "{body}: {error}");
        assert!(
            error.contains("list index out of bounds"),
            "{body}: {error}"
        );
    }
}

#[test]
fn evaluated_wiring_list_bounds_are_retained_as_graph_construction_errors() {
    for body in BOUNDS_CASES {
        let source = format!("{}export fn main() {{let number=fail()}}", helper(body));
        let program =
            compile(&[("list_errors.hgl".into(), source)], "main").unwrap_or_else(|error| {
                panic!("wiring failure must become construction error: {body}: {error}")
            });
        let rust = emit_rust(&program);
        assert!(
            rust.contains("Err(hgl_describe::BuildError::InvalidNodeType"),
            "{body}: {rust}"
        );
        assert!(
            rust.contains("ordinary list index out of bounds"),
            "{body}: {rust}"
        );
    }
}

#[test]
fn wiring_operation_failure_does_not_hide_later_checking_errors() {
    for (later, expected) in [
        (
            "var values:list<i64> =[]\npush(values,1.0)",
            "push ordinary list element type mismatch",
        ),
        (
            "let values:list<i64> =[]\npush(values,1)",
            "push requires writable ordinary list access",
        ),
        (
            "var values:list<i64,0> =[]\npush(values,1)",
            "push requires an unbounded ordinary list",
        ),
        (
            "let values:list<i64> =[1]\nlet bad=values[1.0]",
            "ordinary list index requires i64",
        ),
        (
            "var values:list<i64> =[1]\nvalues[0]=2",
            "indexed replacement is not admitted",
        ),
    ] {
        let source = format!(
            "{}export fn main() {{let number=fail()\n{later}}}",
            helper(BOUNDS_CASES[0])
        );
        let error = compile(&[("list_errors.hgl".into(), source)], "main")
            .expect_err("wiring operation failure must not stop source checking");
        assert!(
            error.contains(expected),
            "{later}: expected {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn wiring_failure_preserves_result_type_for_subsequent_valid_checking() {
    let source = format!(
        "{}const fn identity(value:i64)->i64 {{return value}}\nexport fn main() {{let number=fail()\nlet copy:i64=identity(number)\nvar values:list<i64> =[]\npush(values,copy)}}",
        helper(BOUNDS_CASES[0])
    );
    let program = compile(&[("list_errors.hgl".into(), source)], "main")
        .expect("failed wiring result still has its checked i64 type");
    let rust = emit_rust(&program);
    assert!(rust.contains("ordinary list index out of bounds"), "{rust}");
    assert!(
        rust.contains("Err(hgl_describe::BuildError::InvalidNodeType"),
        "{rust}"
    );
}

#[test]
fn uncontextualized_nonempty_literal_fixedness_is_explicitly_unsupported() {
    let source = "module list_errors\nexport fn main() {let values=[1,2]}";
    let error = compile(&[("list_errors.hgl".into(), source.into())], "main")
        .expect_err("no inferred fixedness contract");
    assert!(
        error.contains("uncontextualized nonempty ordinary list literal inference is unsupported"),
        "{error}"
    );
}
