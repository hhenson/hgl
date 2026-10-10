//! Finite direct source arrival bounds and whole-adapter selection.
use hgl_semantics::ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::{Literal, Ty};
fn scalar(value: i64) -> Value {
    Value::new(Ty::I64, Kind::Literal(Literal::Int(value)))
}
fn publication() -> Statement {
    Statement::TimedYield(scalar(1), scalar(1))
}
fn source(body: Vec<Statement>) -> Node {
    Node {
        name: "source".into(),
        inputs: vec![],
        result: Ty::I64,
        alarm: false,
        generator: Some(body),
        start: vec![],
        global_state: false,
        globals: vec![],
        configuration: vec![],
        stop: vec![],
        caches: vec![],
        handlers: vec![],
    }
}
#[test]
fn independent_sources_sum_and_branches_take_maximum() {
    let plan = Plan {
        nodes: vec![
            source(vec![publication(), publication()]),
            source(vec![
                Statement::If(
                    scalar(1),
                    vec![publication()],
                    vec![publication(), publication(), publication()],
                ),
                publication(),
            ]),
        ],
        ..Plan::default()
    };
    assert_eq!(hgl_rust::execution_proof::direct_arrivals(&plan), Some(6));
    assert!(hgl_rust::execution_proof::prepared(&plan));
}
#[test]
fn unknown_generator_loop_keeps_the_generic_adapter() {
    let plan = Plan {
        nodes: vec![source(vec![Statement::While(
            scalar(1),
            vec![publication()],
        )])],
        ..Plan::default()
    };
    assert_eq!(hgl_rust::execution_proof::direct_arrivals(&plan), None);
    assert!(!hgl_rust::execution_proof::prepared(&plan));
}

#[test]
fn proved_schedule_requires_an_installed_preparation_phase() {
    let plan = Plan {
        ordinary_instantiation: true,
        nodes: vec![source(vec![publication(), publication()])],
        ..Plan::default()
    };
    assert_eq!(hgl_rust::execution_proof::direct_arrivals(&plan), Some(2));
    assert!(!hgl_rust::execution_proof::prepared(&plan));
}
