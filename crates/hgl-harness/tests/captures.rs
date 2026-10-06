//! Sparse captures retain a logical horizon without allocating silent slots.
use hgl_harness_ir::{CapturedEval, Evaluation, Step, Test};
use hgl_rust_ir::{Kind, Statement, Value};
use hgl_source::{Literal, Ty};
fn int(value: i64) -> Value {
    Value::new(Ty::I64, Kind::Literal(Literal::Int(value)))
}
fn local(ty: Ty, id: usize) -> Value {
    Value::new(ty, Kind::Local(id))
}
fn binary(op: &str, a: Value, b: Value) -> Value {
    Value::new(Ty::Bool, Kind::Binary(op.into(), Box::new(a), Box::new(b)))
}
fn eval(case: usize) -> Evaluation {
    Evaluation {
        case,
        arguments: vec![],
        expected: None,
    }
}
#[test]
fn enormous_silent_horizon_remains_sparse_and_earlier_capture_survives_later_eval() {
    let horizon = 1_000_000_000_000_i64;
    let nullable = Ty::Nullable(Box::new(Ty::I64));
    let ty = Ty::List(Box::new(nullable.clone()), None);
    let first = local(nullable.clone(), 1);
    let index = |offset| {
        Value::new(
            nullable.clone(),
            Kind::Index(Box::new(local(ty.clone(), 0)), Box::new(int(offset))),
        )
    };
    let test = Test {
        name: "sparse".into(),
        steps: vec![
            Step::BindEval(0, ty.clone(), eval(0)),
            Step::Eval(eval(1)),
            Step::Assert(binary(
                "==",
                Value::new(Ty::I64, Kind::Length(Box::new(local(ty.clone(), 0)))),
                int(horizon),
            )),
            Step::Ordinary(Statement::Let(1, index(0))),
            Step::If(
                Value::new(Ty::Bool, Kind::IsPresent(Box::new(first.clone()))),
                vec![Step::Assert(binary(
                    "==",
                    Value::new(Ty::I64, Kind::Present(Box::new(first))),
                    int(7),
                ))],
                vec![Step::Eval(eval(99))],
            ),
            Step::Ordinary(Statement::Let(2, index(horizon - 1))),
            Step::Assert(Value::new(
                Ty::Bool,
                Kind::Unary(
                    "!".into(),
                    Box::new(Value::new(
                        Ty::Bool,
                        Kind::IsPresent(Box::new(local(nullable, 2))),
                    )),
                ),
            )),
        ],
    };
    let mut calls = vec![];
    let count = hgl_harness::execute(
        &test,
        |_| unreachable!(),
        |case, _| {
            calls.push(case);
            Ok(CapturedEval {
                length: usize::try_from(horizon).unwrap(),
                ticks: vec![(0, int(if case == 0 { 7 } else { 9 }))],
            })
        },
    )
    .unwrap();
    assert_eq!(count, 5);
    assert_eq!(calls, [0, 1]);
}
