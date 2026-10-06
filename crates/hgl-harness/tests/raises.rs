//! Exact semantic matching cannot turn another kind of failure into a pass.
use hgl_harness_ir::{CapturedEval, Evaluation, Failure, Step, Test};
use hgl_node_error::{NodeError, Phase};
use hgl_rust_ir::{Kind, Statement, Value};
use hgl_source::{Literal, Ty};
const CODE: &str = "yield.negative_duration";
fn evaluation(case: usize) -> Step {
    Step::Eval(Evaluation {
        case,
        arguments: vec![],
        expected: None,
    })
}
fn raises(body: Vec<Step>) -> Step {
    Step::Raises(CODE.into(), body)
}
fn bool_value(value: bool) -> Value {
    Value::new(Ty::Bool, Kind::Literal(Literal::Bool(value)))
}
fn run(
    body: Vec<Step>,
    mut callback: impl FnMut(usize) -> Result<CapturedEval, Failure>,
) -> Result<usize, String> {
    hgl_harness::execute(
        &Test {
            name: "raises".into(),
            steps: body,
        },
        |_| unreachable!(),
        |case, _| callback(case),
    )
}
#[test]
fn exact_code_consumes_once_and_continues_after_cleanup() {
    let mut calls = Vec::new();
    let body = vec![raises(vec![evaluation(0), evaluation(1)]), evaluation(2)];
    assert_eq!(
        run(body, |case| {
            calls.push(case);
            if case == 0 {
                Err(Failure::Execution(NodeError::coded(
                    "arbitrary words",
                    CODE,
                )))
            } else {
                Ok(CapturedEval {
                    length: 0,
                    ticks: vec![],
                })
            }
        }),
        Ok(2)
    );
    assert_eq!(calls, [0, 2]);
}
#[test]
fn wrong_uncoded_and_cleanup_failures_cannot_match() {
    let mut wrong = NodeError::coded(CODE, "yield.non_increasing_time");
    wrong.message = CODE.into();
    let uncoded = NodeError::new(CODE);
    let mut cleanup_only = NodeError::coded("stop", CODE);
    cleanup_only.phase = Phase::Stop;
    let mut failed_cleanup = NodeError::coded("primary", CODE);
    failed_cleanup
        .cleanup
        .push(*NodeError::new("cleanup sentinel"));
    for error in [wrong, uncoded, cleanup_only, failed_cleanup] {
        let mut error = Some(error);
        assert!(
            run(vec![raises(vec![evaluation(0)])], |_| Err(
                Failure::Execution(error.take().unwrap())
            ))
            .is_err()
        );
    }
}
#[test]
fn empty_normal_assertion_and_nested_failures_cannot_match() {
    let ordinary_failure = Step::Assert(bool_value(false));
    for body in [
        vec![],
        vec![evaluation(0)],
        vec![ordinary_failure],
        vec![raises(vec![])],
    ] {
        assert!(
            run(vec![raises(body)], |_| Ok(CapturedEval {
                length: 0,
                ticks: vec![]
            }))
            .is_err()
        );
    }
    assert!(run(vec![raises(vec![evaluation(0)])], |_| Err(CODE.into())).is_err());
}
#[test]
fn successful_inner_consumption_does_not_satisfy_outer() {
    assert!(
        run(vec![raises(vec![raises(vec![evaluation(0)])])], |_| Err(
            Failure::Execution(NodeError::coded("error", CODE))
        ))
        .is_err()
    );
}
#[test]
fn earlier_outer_writes_survive_and_inner_locals_are_released() {
    let local = Value::new(Ty::Bool, Kind::MutableLocal(0));
    let body = vec![
        Step::Ordinary(Statement::Var(0, bool_value(false))),
        raises(vec![
            Step::Ordinary(Statement::Assign(local.clone(), bool_value(true))),
            evaluation(0),
        ]),
        Step::Assert(local),
    ];
    assert_eq!(
        run(body, |_| Err(Failure::Execution(NodeError::coded(
            "error", CODE
        )))),
        Ok(2)
    );
}
#[test]
fn panics_are_not_caught_by_raises() {
    assert!(
        std::panic::catch_unwind(|| run(vec![raises(vec![evaluation(0)])], |_| panic!("crash")))
            .is_err()
    );
}
