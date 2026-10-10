//! Admission for complete ordinary Tuple publication and prepared child ownership.
use crate::ir::Value;
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
                || (crate::tuple_values::endpoint(value)
                    && matches!(value.kind, crate::ir::Kind::Field(..))))
    }) {
        return Err(
            "retained text projections cannot yet cross an ordinary value-function boundary".into(),
        );
    }
    Ok(())
}
