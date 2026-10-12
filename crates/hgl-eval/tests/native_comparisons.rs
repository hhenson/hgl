//! Ordinary Native partial order agrees across cold, boxed and generated values.
use hgl_eval::{
    ir::{Kind, Value},
    value_eval::EvalError,
};
use hgl_types::{NodeError, NodeResult};
use hgl_value_model::{
    Ty,
    native_value::{NativeValue, Operations},
};
use std::cmp::Ordering;
fn identity(data: &[u8]) -> NodeResult<u64> {
    Ok(u64::from_le_bytes(
        data.get(..8)
            .ok_or_else(|| NodeError::new("provider encoding"))?
            .try_into()
            .map_err(|e| NodeError::new(format!("{e}")))?,
    ))
}
fn copy(data: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(data);
}
fn equal(a: &[u8], b: &[u8]) -> NodeResult<bool> {
    Ok(identity(a)? == identity(b)?)
}
fn hash(data: &[u8]) -> NodeResult<u64> {
    identity(data)
}
fn value(name: &str, id: u64, tag: u8, ops: &'static Operations) -> Value {
    let mut data = id.to_le_bytes().to_vec();
    data.push(tag);
    Value::new(
        Ty::Native(hgl_value_model::native_contracts::Scalar {
            identity: name.into(),
            capabilities: ["copy", "text", "equality", "hash", "order"]
                .into_iter()
                .map(String::from)
                .collect(),
        }),
        Kind::Opaque(NativeValue::new(data, ops)),
    )
}

#[test]
fn native_partial_order_preserves_unordered_pairs_provider_errors_and_nan_policy()
-> Result<(), EvalError> {
    static PARTIAL: Operations = Operations {
        copy,
        equal,
        hash,
        compare: |a, b| {
            let a = identity(a)?;
            let b = identity(b)?;
            if a == 99 {
                return Err(NodeError::coded(
                    "provider ordering rejected",
                    "native.order.failure",
                ));
            }
            Ok((a == b).then_some(Ordering::Equal))
        },
    };
    let a = value("partial", 1, 0, &PARTIAL);
    let b = value("partial", 2, 0, &PARTIAL);
    for op in ["<", "<=", ">", ">="] {
        for (a, b) in [(&a, &b), (&b, &a)] {
            let result = hgl_eval::value_operations::binary(op, a, b)?;
            assert!(matches!(
                result.kind,
                Kind::Literal(hgl_value_model::Literal::Bool(false))
            ));
            let result = hgl_eval::value_operations::binary(
                op,
                &hgl_eval::any_values::retain(a.clone()),
                &hgl_eval::any_values::retain(b.clone()),
            )?;
            assert!(matches!(
                result.kind,
                Kind::Literal(hgl_value_model::Literal::Bool(false))
            ));
        }
    }
    for (op, expected) in [
        ("<", false),
        ("<=", true),
        (">", false),
        (">=", true),
        ("==", true),
        ("!=", false),
    ] {
        assert!(
            matches!(hgl_eval::value_operations::binary(op,&a,&a)?.kind,Kind::Literal(hgl_value_model::Literal::Bool(actual)) if actual==expected)
        );
    }
    let rejected = value("partial", 99, 0, &PARTIAL);
    assert!(matches!(
        hgl_eval::value_operations::binary("<", &rejected, &a),
        Err(EvalError::Coded("native.order.failure", _))
    ));
    let nan = Value::new(
        Ty::F64,
        Kind::Literal(hgl_value_model::Literal::Float(f64::NAN)),
    );
    assert!(
        matches!(hgl_eval::value_operations::binary("<",&nan,&nan),Err(EvalError::Unsupported(message)) if message=="NaN comparison policy is not specified")
    );
    Ok(())
}
