//! Test capture comparisons retain sparse horizon and checked ordinary payload identity.
use super::{Checker, Env, Expr, Issue, Kind, Ty, Value, ValueContext};
fn captured<'a>(expr: &Expr, env: &'a Env) -> Option<(&'a Value, &'a Ty)> {
    let Expr::Name(name) = expr.syntax() else {
        return None;
    };
    let value = env.get(name)?;
    let Ty::List(child, _) = &value.ty else {
        return None;
    };
    let Ty::Nullable(payload) = child.as_ref() else {
        return None;
    };
    Some((value, payload))
}
impl Checker {
    pub(super) fn captured_comparison(
        &mut self,
        module: &str,
        op: &str,
        (left, right): (&Expr, &Expr),
        env: &Env,
        runtime: bool,
    ) -> Result<Option<Value>, Issue> {
        if runtime
            || self.preparing_graph
            || self.value_context != ValueContext::Preparation
            || !matches!(op, "==" | "!=")
        {
            return Ok(None);
        }
        let (capture, payload, other, reverse) = if let Some((value, payload)) = captured(left, env)
        {
            (value, payload, right, false)
        } else if let Some((value, payload)) = captured(right, env) {
            (value, payload, left, true)
        } else {
            return Ok(None);
        };
        if matches!(payload, Ty::Result(..)) {
            hgl_semantics::value_operators::binary_type(op, payload, payload)?;
        }
        let other = if let Expr::Sequence(cells) = other.syntax() {
            let mut slots = Vec::new();
            for (index, expr) in cells.iter().enumerate() {
                let Some(expr) = expr else { continue };
                let value = self.expected_expression(module, expr, env, false, Some(payload))?;
                super::require_payload(&value)?;
                if value.ty != *payload {
                    return Err(Issue::typed(
                        expr.span(),
                        "captured sequence payload type mismatch",
                    ));
                }
                slots.push((index, value));
            }
            Value::new(capture.ty.clone(), Kind::Captured(cells.len(), slots))
        } else {
            self.expression(module, other, env, false)?
        };
        if other.ty != capture.ty {
            return Err(Issue::typed(left.span(), "captured sequence type mismatch"));
        }
        let (a, b) = if reverse {
            (other, capture.clone())
        } else {
            (capture.clone(), other)
        };
        Ok(Some(Value::new(
            Ty::Bool,
            Kind::Binary(op.into(), Box::new(a), Box::new(b)),
        )))
    }
}
