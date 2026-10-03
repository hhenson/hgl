//! VAL15 checked time arithmetic is an operation failure, never wrapping.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Literal, Ty};
use hgl_value_eval::{EvalError, Evaluator};

fn literal(value: Literal) -> Value {
    Value::new(value.ty(), Kind::Literal(value))
}
fn binary(op: &str, a: Literal, b: Literal, result: Ty) -> Value {
    Value::new(
        result,
        Kind::Binary(op.into(), Box::new(literal(a)), Box::new(literal(b))),
    )
}
#[test]
fn time_results_follow_the_exact_operand_table() {
    for (op, a, b, expected) in [
        (
            "+",
            Literal::DateTime(10),
            Literal::Duration(3),
            Literal::DateTime(13),
        ),
        (
            "+",
            Literal::Duration(3),
            Literal::DateTime(10),
            Literal::DateTime(13),
        ),
        (
            "-",
            Literal::DateTime(10),
            Literal::Duration(3),
            Literal::DateTime(7),
        ),
        (
            "-",
            Literal::DateTime(3),
            Literal::DateTime(10),
            Literal::Duration(-7),
        ),
        (
            "+",
            Literal::Duration(-10),
            Literal::Duration(3),
            Literal::Duration(-7),
        ),
        (
            "-",
            Literal::Duration(-10),
            Literal::Duration(3),
            Literal::Duration(-13),
        ),
    ] {
        let result = Evaluator::default()
            .value(&binary(op, a, b, expected.ty()))
            .unwrap();
        assert!(matches!(result.kind, Kind::Literal(actual) if actual == expected));
    }
}

#[test]
fn both_range_boundaries_fail_instead_of_becoming_a_different_time() {
    for (op, a, b, ty) in [
        (
            "+",
            Literal::DateTime(i64::MAX),
            Literal::Duration(1),
            Ty::DateTime,
        ),
        (
            "+",
            Literal::Duration(1),
            Literal::DateTime(i64::MAX),
            Ty::DateTime,
        ),
        (
            "-",
            Literal::DateTime(i64::MIN),
            Literal::Duration(1),
            Ty::DateTime,
        ),
        (
            "-",
            Literal::DateTime(i64::MAX),
            Literal::DateTime(-1),
            Ty::Duration,
        ),
        (
            "+",
            Literal::Duration(i64::MAX),
            Literal::Duration(1),
            Ty::Duration,
        ),
        (
            "-",
            Literal::Duration(i64::MIN),
            Literal::Duration(1),
            Ty::Duration,
        ),
    ] {
        assert!(
            matches!(Evaluator::default().value(&binary(op, a, b, ty)), Err(EvalError::Operation(message)) if message == "time arithmetic overflow")
        );
    }
    let negated = Value::new(
        Ty::Duration,
        Kind::Unary("-".into(), Box::new(literal(Literal::Duration(i64::MIN)))),
    );
    assert!(
        matches!(Evaluator::default().value(&negated), Err(EvalError::Operation(message)) if message == "time arithmetic overflow")
    );
}
