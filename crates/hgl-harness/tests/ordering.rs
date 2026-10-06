//! Executed lexical preparation, without a graph or provider implementation.
use hgl_harness_ir::{Argument, CapturedEval, Evaluation, Step, Test};
use hgl_rust_ir::{Kind, Statement, Value};
use hgl_source::{Literal, TemporalLiteral, Ty};
use std::cell::RefCell;
fn recipe(name: &str) -> Value {
    Value::new(
        Ty::TimeZone,
        Kind::TemporalLiteral(TemporalLiteral::TimeZone(name.into())),
    )
}
fn local(id: usize) -> Value {
    Value::new(Ty::TimeZone, Kind::Local(id))
}
fn materialize(
    recipe: &TemporalLiteral,
    trace: &RefCell<Vec<String>>,
) -> Result<Literal, hgl_value_eval::EvalError> {
    let TemporalLiteral::TimeZone(name) = recipe else {
        unreachable!()
    };
    trace.borrow_mut().push(name.clone());
    if name == "invalid" {
        return Err(hgl_value_eval::EvalError::Operation("invalid zone".into()));
    }
    Ok(Literal::TimeZone(
        hgl_types::time_values::ZoneId::from_validated_name(name.clone()),
    ))
}
fn constant(binding: usize, value: Value) -> Argument {
    Argument::Constant { binding, value }
}
fn eval(case: usize, arguments: Vec<Argument>, expected: Option<Vec<Option<Value>>>) -> Step {
    Step::Eval(Evaluation {
        case,
        arguments,
        expected,
    })
}
#[test]
fn setup_and_arguments_execute_once_and_expectations_follow_the_graph() {
    let trace = RefCell::new(Vec::new());
    let entry = Ty::Struct(
        "hgraph.std::TimedValue".into(),
        vec![
            ("time".into(), Ty::DateTime),
            ("value".into(), Ty::TimeZone),
        ],
        vec![],
    );
    let test = Test {
        name: "order".into(),
        steps: vec![
            Step::Ordinary(Statement::Let(0, recipe("setup"))),
            eval(
                0,
                vec![
                    constant(1, recipe("first")),
                    Argument::Dense {
                        parameter: "value".into(),
                        binding: 0,
                        shape: Ty::TimeZone,
                        entry_type: entry,
                        slots: vec![Some(recipe("second")), None],
                    },
                    constant(2, local(0)),
                ],
                Some(vec![Some(recipe("expected"))]),
            ),
            eval(1, vec![constant(0, local(0))], None),
        ],
    };
    let count=hgl_harness::execute(&test,|r|materialize(r,&trace),|case,prepared| {
        trace.borrow_mut().push(format!("graph{case}"));
        assert_eq!(prepared.input_length,if case==0 {2} else {0});
        let slot=if case==0 {2} else {0};
        assert!(matches!(&prepared.arguments[slot].kind,Kind::Literal(Literal::TimeZone(zone)) if zone.as_str()=="setup"));
        let captured=Value::new(Ty::TimeZone,Kind::Literal(Literal::TimeZone(hgl_types::time_values::ZoneId::from_validated_name("expected".into()))));
        Ok(CapturedEval {length:1,ticks:vec![(0,captured)]})
    }).unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        *trace.borrow(),
        ["setup", "first", "second", "graph0", "expected", "graph1"]
    );
}
#[test]
fn unused_executed_binding_fails_but_unexecuted_branch_does_not_materialize() {
    for executed in [false, true] {
        let trace = RefCell::new(Vec::new());
        let test = Test {
            name: "branch".into(),
            steps: vec![
                Step::Ordinary(Statement::If(
                    Value::new(Ty::Bool, Kind::Literal(Literal::Bool(executed))),
                    vec![Statement::Let(0, recipe("invalid"))],
                    vec![],
                )),
                eval(0, vec![], None),
            ],
        };
        let result = hgl_harness::execute(
            &test,
            |r| materialize(r, &trace),
            |_, _| {
                trace.borrow_mut().push("graph".into());
                Ok(CapturedEval {
                    length: 0,
                    ticks: vec![],
                })
            },
        );
        assert_eq!(result.is_err(), executed);
        assert_eq!(
            *trace.borrow(),
            if executed {
                vec!["invalid"]
            } else {
                vec!["graph"]
            }
        );
    }
}
#[test]
fn argument_failure_stops_later_expressions_and_target_start() {
    let trace = RefCell::new(Vec::new());
    let test = Test {
        name: "failure".into(),
        steps: vec![eval(
            0,
            vec![
                constant(0, recipe("first")),
                constant(1, recipe("invalid")),
                constant(2, recipe("last")),
            ],
            None,
        )],
    };
    let error = hgl_harness::execute(
        &test,
        |r| materialize(r, &trace),
        |_, _| {
            trace.borrow_mut().push("graph".into());
            Ok(CapturedEval {
                length: 0,
                ticks: vec![],
            })
        },
    )
    .unwrap_err();
    assert_eq!(error, "invalid zone");
    assert_eq!(*trace.borrow(), ["first", "invalid"]);
}
