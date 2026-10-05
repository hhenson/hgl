use hgl_rust_ir::{Kind, Value};
use hgl_source::Ty;

fn text_parts(value: &Value, parts: &mut Vec<String>) -> Option<()> {
    match &value.kind {
        Kind::Binary(op, a, b) if op == "+" && value.ty == Ty::Str => {
            text_parts(a, parts)?;
            text_parts(b, parts)?;
        }
        Kind::Input(id, _) if value.ty == Ty::Str => parts.push(format!(
            "observation.scalar::<String>(self.input{id}.id())?"
        )),
        Kind::Literal(hgl_source::Literal::Str(value)) => parts.push(format!("{value:?}")),
        Kind::Query(op, args) if op == "delta_value" => text_parts(args.first()?, parts)?,
        Kind::Delta(_)
        | Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::List(_)
        | Kind::Index(..)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::ValueCall(..)
        | Kind::Configuration(_)
        | Kind::Construct(_)
        | Kind::Field(..)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => return None,
    }
    Some(())
}
/// Compose a pure observed text expression directly into prepared output capacity.
pub fn text(value: &Value) -> Option<String> {
    if !matches!(&value.kind,Kind::Binary(op,_,_) if op=="+" && value.ty==Ty::Str) {
        return None;
    }
    let mut parts = Vec::new();
    text_parts(value, &mut parts)?;
    let measure=parts.iter().map(|part|format!("bytes=bytes.checked_add(({part}).len()).ok_or_else(||hgl_types::NodeError::new(\"prepared text capacity overflow\"))?;")).collect::<Vec<_>>().concat();
    let compose = parts
        .iter()
        .map(|part| {
            format!(
                "destination.push_str({});",
                part.replace(
                    '?',
                    ".unwrap_or_else(|_|unreachable!(\"measured valid input\"))"
                )
            )
        })
        .collect::<Vec<_>>()
        .concat();
    Some(format!(
        "_ctx.prepared().text(self._output.id(),self._output.generation(),|observation|{{let mut bytes=0usize;{measure}Ok(bytes)}},|destination,observation|{{{compose}}})?;return Ok(());"
    ))
}
