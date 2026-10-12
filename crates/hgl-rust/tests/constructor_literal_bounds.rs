//! Owner normalization must preserve the finite proof of literal collection recipes.
use hgl_semantics::ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::{Literal, Ty};

#[test]
fn nested_literal_owners_keep_collection_proof_and_standalone_staging() {
    let ty = Ty::Struct(
        "bounds::Item".into(),
        vec![("text".into(), Ty::Str)],
        vec![],
    );
    let item = Value::new(
        ty.clone(),
        Kind::Construct(vec![(
            0,
            Value::new(Ty::Str, Kind::Literal(Literal::Str("owned".into()))),
        )]),
    );
    for collection in [false, true] {
        let value = if collection {
            Value::new(
                Ty::List(Box::new(ty.clone()), None),
                Kind::List(vec![item.clone()]),
            )
        } else {
            item.clone()
        };
        let plan = Plan {
            nodes: vec![Node {
                name: "literal".into(),
                inputs: vec![],
                result: value.ty.clone(),
                alarm: false,
                generator: None,
                start: vec![],
                stop: vec![],
                global_state: false,
                globals: vec![],
                configuration: vec![],
                caches: vec![],
                handlers: vec![(None, vec![Statement::Return(value)])],
            }],
            ..Plan::default()
        };
        assert!(hgl_rust::execution_proof::prepared(&plan));
        let prepared = hgl_rust::any_prepared::prepare(&plan);
        assert!(hgl_rust::execution_proof::prepared(&prepared));
        assert_eq!(prepared.nodes[0].globals.len(), usize::from(!collection));
    }
}
