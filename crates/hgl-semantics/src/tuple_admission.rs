//! Admission for complete ordinary Tuple publication and prepared child ownership.
use crate::ir::{Kind, Value};
pub use crate::structural_admission::{expected, result};
use hgl_source::Ty;
/// Admit prepared observation children while keeping native collection construction bounded.
pub fn constructor_child(value: &Value, constant_context: bool) -> Result<(), &'static str> {
    if !constant_context
        && !value.snapshot
        && (crate::structural_admission::collection(&value.ty)
            || matches!(value.ty, Ty::Str | Ty::Bytes))
    {
        return Err(
            "runtime ordinary tuple collection, text or bytes children require prepared retained observations",
        );
    }
    Ok(())
}

/// Variable-sized Tuple projections keep their prepared representation at helper boundaries.
pub fn value_arguments(args: &[Value]) -> Result<(), String> {
    if args.iter().any(|value| {
        value.ty == Ty::Str
            && (value.snapshot
                || (crate::tuple_values::endpoint(value) && matches!(value.kind, Kind::Field(..))))
    }) {
        return Err(
            "retained text projections cannot yet cross an ordinary value-function boundary".into(),
        );
    }
    Ok(())
}

/// Retain readable byte initializers in independently owned prepared local slots.
pub fn retain_local(value: Value) -> Value {
    let source = if let Kind::Query(op, args) = &value.kind
        && op == "delta_value"
    {
        args.first()
    } else {
        Some(&value)
    };
    if value.ty == Ty::Bytes
        && let Some(source) = source
        && (source.ty == Ty::Bytes
            || matches!(&source.ty, Ty::Rolling(child, _) if **child == Ty::Bytes))
        && matches!(source.kind, Kind::Input(_, false) | Kind::Field(..))
        && crate::tuple_values::endpoint(source)
    {
        let mut retained = Value::new(
            Ty::Bytes,
            Kind::Unary("snapshot".into(), Box::new(source.clone())),
        );
        retained.snapshot = true;
        retained
    } else {
        crate::tuple_values::retain(value, false)
    }
}
