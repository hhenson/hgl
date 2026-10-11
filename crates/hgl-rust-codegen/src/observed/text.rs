use hgl_semantics::ir::{Kind, Value};
use hgl_source::Ty;

struct TextPart {
    checked: String,
    validated: String,
}
impl TextPart {
    fn immutable(value: String) -> Self {
        Self {
            checked: value.clone(),
            validated: value,
        }
    }
}
fn text_leaf(value: &Value) -> Option<TextPart> {
    if let Kind::Input(id, _) = value.kind {
        let read = format!("observation.scalar::<String>(self.input{id}.id())");
        Some(TextPart {
            checked: format!("{read}?"),
            validated: format!(
                "{read}.unwrap_or_else(|_|unreachable!(\"validated immutable input\"))"
            ),
        })
    } else if let Kind::Configuration(id) = value.kind {
        Some(TextPart::immutable(format!(
            "self.configuration_columns.scalar::<String>(self.configuration_slot{id}.fields())"
        )))
    } else if let Kind::Literal(hgl_source::Literal::Str(value)) = &value.kind {
        Some(TextPart::immutable(format!("{value:?}")))
    } else {
        None
    }
}
/// Each leaf is resolved before the next operand, including index evaluation and validation.
fn text_parts(
    value: &Value,
    parts: &mut Vec<TextPart>,
    leaf: &mut impl FnMut(&Value) -> Option<TextPart>,
) -> Option<()> {
    if value.ty != Ty::Str {
        return None;
    }
    if let Kind::Binary(op, a, b) = &value.kind
        && op == "+"
    {
        text_parts(a, parts, leaf)?;
        text_parts(b, parts, leaf)?;
    } else if let Kind::Query(op, args) = &value.kind
        && op == "delta_value"
    {
        text_parts(args.first()?, parts, leaf)?;
    } else {
        parts.push(leaf(value)?);
    }
    Some(())
}
/// Compose a pure observed text expression directly into prepared output capacity.
pub fn text(value: &Value, result: Option<&Ty>) -> Option<String> {
    if !matches!(&value.kind,Kind::Binary(op,_,_) if op=="+" && value.ty==Ty::Str) {
        return None;
    }
    let mut parts = Vec::new();
    text_parts(value, &mut parts, &mut text_leaf)?;
    let measure=parts.iter().map(|part|format!("bytes=bytes.checked_add(({}).len()).ok_or_else(||hgl_types::NodeError::new(\"prepared text capacity overflow\"))?;",part.checked)).collect::<Vec<_>>().concat();
    let compose = parts
        .iter()
        .map(|part| format!("destination.push_str({});", part.validated))
        .collect::<Vec<_>>()
        .concat();
    let target = if result.is_some_and(|ty| matches!(ty, Ty::Rolling(..))) {
        "rolling_text(self._output"
    } else if result.is_none_or(|ty| *ty == Ty::Str) {
        "text(self._output.id(),self._output.generation()"
    } else {
        return None;
    };
    Some(format!(
        "_ctx.prepared().{target},|observation|{{let mut bytes=0usize;{measure}Ok(bytes)}},|destination,observation|{{{compose}}})?;"
    ))
}

/// Encode immutable text fragments after their ordered read validation.
pub(crate) fn text_encoding(
    value: &Value,
    prelude: &mut Vec<String>,
    emit: &impl Fn(&Value) -> String,
) -> Option<String> {
    let mut retained = Vec::new();
    let mut parts = Vec::new();
    text_parts(value, &mut parts, &mut |value| {
        let part = text_leaf(value).or_else(|| {
            let slot = crate::source_slots::projection(value, emit)?;
            let id = prelude.len() + retained.len();
            retained.push(format!("let expression_text_slot{id}={slot};"));
            Some(TextPart::immutable(format!(
                "self.configuration_columns.scalar::<String>(expression_text_slot{id}.fields())"
            )))
        })?;
        retained.push(format!("_ctx.with_observations(|observation,_|->hgl_types::NodeResult{{let _=({}).len();Ok(())}})?;", part.checked));
        Some(part)
    })?;
    prelude.extend(retained);
    Some(
        parts
            .into_iter()
            .map(|part| format!("out.write(({}).as_bytes());", part.validated))
            .collect::<Vec<_>>()
            .concat(),
    )
}
