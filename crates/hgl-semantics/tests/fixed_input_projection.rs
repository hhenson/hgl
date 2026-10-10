//! Static input projection preserves endpoint identity and exact position checks.
use hgl_semantics::{
    ir::{Kind, Value},
    tuple_values,
};
use hgl_source::{Literal, Ty};
fn parent(size: usize) -> Value {
    Value::new(
        Ty::List(Box::new(Ty::I64), Some(size)),
        Kind::Input(0, false),
    )
}
fn index(position: i64) -> Value {
    Value::new(Ty::I64, Kind::Literal(Literal::Int(position)))
}
#[test]
fn fixed_input_indices_preserve_endpoint_identity_and_reject_dynamic_or_absent_positions() {
    let child = tuple_values::indexed(parent(2), index(1)).unwrap();
    assert_eq!(child.ty, Ty::I64);
    assert!(tuple_values::endpoint(&child));
    assert!(matches!(child.kind, Kind::Field(_, 1)));
    for (size, position) in [(0, 0), (2, 2), (2, -1)] {
        assert!(
            tuple_values::indexed(parent(size), index(position))
                .unwrap_err()
                .contains("out of bounds")
        );
    }
    let dynamic = Value::new(Ty::I64, Kind::Input(1, false));
    assert!(
        tuple_values::indexed(parent(2), dynamic)
            .unwrap_err()
            .contains("constant integer position")
    );
}
