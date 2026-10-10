//! Ordinary scalar operations and their phase-independent failure contract.
use std::fmt;

/// Failure during ordinary evaluation, separated from an unsupported operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// An admitted operation failed when evaluated.
    Operation(String),
    /// An operation with a stable execution identifier.
    Coded(&'static str, String),
    /// A provider-dependent value needs the host construction context.
    ContextRequired,
    /// The checked IR cannot be executed by this ordinary evaluator.
    Unsupported(String),
}
impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(message) | Self::Coded(_, message) | Self::Unsupported(message) => {
                f.write_str(message)
            }
            Self::ContextRequired => {
                f.write_str("temporal literal requires a construction context")
            }
        }
    }
}
impl std::error::Error for EvalError {}
impl From<String> for EvalError {
    fn from(message: String) -> Self {
        Self::Operation(message)
    }
}
fn unsupported(message: &str) -> EvalError {
    EvalError::Unsupported(message.into())
}
mod scalar;
pub use scalar::{binary, unary};

/// Check the ordinary constructor without moving execution to checking time.
pub fn bytes(
    args: &[(Option<String>, hgl_source::Expr)],
    mut check: impl FnMut(
        &hgl_source::Expr,
        &hgl_source::Ty,
    ) -> Result<crate::ir::Value, hgl_source::Issue>,
) -> Result<crate::ir::Value, hgl_source::Issue> {
    use crate::ir::{Kind, Value};
    use hgl_source::Ty;
    if args.len() > 1 || args.iter().any(|(label, _)| label.is_some()) {
        return Err("bytes requires zero or one positional argument".into());
    }
    let list = Ty::List(Box::new(Ty::I64), None);
    let argument = args
        .first()
        .map(|(_, expr)| check(expr, &list))
        .transpose()?
        .unwrap_or_else(|| Value::new(list, Kind::List(Vec::new())));
    if !matches!(
        (&argument.ty, &argument.kind),
        (Ty::Atomic(_), Kind::Input(_, false))
    ) {
        crate::endpoint_check::require_payload(&argument)?;
    }
    if !matches!(&argument.ty, Ty::List(element, _) if **element == Ty::I64)
        && !matches!(&argument.ty, Ty::Atomic(payload) if matches!(payload.as_ref(), Ty::List(element, _) if **element == Ty::I64))
    {
        return Err("bytes requires one ordinary i64 list".into());
    }
    Ok(Value::new(
        Ty::Bytes,
        Kind::Unary("bytes".into(), Box::new(argument)),
    ))
}
