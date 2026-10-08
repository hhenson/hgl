//! Source function phase and service-header admission.
use crate::library::Signature;
use hgl_source::{Cursor, Stmt, Token};

/// Classify node-only constructs without evaluating the body.
pub fn runtime_body(signature: &Signature) -> Result<bool, String> {
    let constructs = statement_constructs(&signature.body);
    if signature.value_function
        && constructs
            .iter()
            .any(|word| matches!(*word, "yield" | "while"))
    {
        return Err("yield/while require a runtime body, not a const value function".into());
    }
    Ok(signature.value_function
        || constructs.iter().any(|word| {
            matches!(
                *word,
                "state" | "cache" | "inject" | "start" | "when" | "stop" | "yield"
            )
        }))
}

fn statement_constructs(tokens: &[Token]) -> Vec<&str> {
    let mut constructs = Vec::new();
    let mut delimiters = 0;
    let mut boundary = true;
    for (index, token) in tokens.iter().enumerate() {
        let word = token.text.as_str();
        if delimiters == 0 && boundary && is_construct(tokens, index) {
            constructs.push(word);
        }
        match word {
            "(" | "[" => delimiters += 1,
            ")" | "]" => delimiters -= 1,
            _ => {}
        }
        boundary = matches!(word, "{" | "}" | "\n");
    }
    constructs
}

fn is_construct(tokens: &[Token], index: usize) -> bool {
    let mut cursor = Cursor {
        tokens,
        pos: index + 1,
    };
    match tokens[index].text.as_str() {
        "yield" | "while" => true,
        "start" | "stop" => cursor.at("{"),
        "when" => cursor.at("{") || (cursor.expr().is_ok() && cursor.at("{")),
        "state" | "cache" => cursor.name().is_ok() && cursor.at(":"),
        "for" => cursor.name().is_ok() && cursor.at("in"),
        "inject" => cursor.name().is_ok(),
        _ => false,
    }
}

/// Parse an ordinary body, retaining its function-level service declarations.
pub fn ordinary_body(cursor: &mut Cursor<'_>) -> Result<(Vec<String>, Vec<Stmt>), String> {
    if cursor.take("=>") {
        cursor.lines();
        return Ok((Vec::new(), vec![Stmt::Return(cursor.expr()?)]));
    }
    cursor.need("{")?;
    cursor.lines();
    let mut services = Vec::new();
    while cursor.take("inject") {
        cursor.lines();
        loop {
            let service = cursor.name()?;
            if services.contains(&service) {
                return Err(format!("duplicate injectable {service}"));
            }
            services.push(service);
            cursor.lines();
            if !cursor.take(",") {
                break;
            }
            cursor.lines();
            if matches!(
                cursor.peek(),
                "}" | "inject" | "let" | "var" | "if" | "while" | "yield" | "return"
            ) {
                break;
            }
        }
    }
    let body = cursor.block_contents()?;
    cursor.lines();
    if !cursor.at("") {
        return Err("unsupported function body suffix".into());
    }
    Ok((services, body))
}
/// Admit generator syntax before ordinary operand and lexical checking.
pub fn generator_body(cursor: &mut Cursor<'_>) -> Result<(Vec<String>, Vec<Stmt>), String> {
    if let Some(word) = statement_constructs(cursor.tokens)
        .into_iter()
        .find(|word| matches!(*word, "state" | "cache" | "start" | "stop" | "when" | "for"))
    {
        return Err(format!(
            "generator does not admit {word}; use while for iteration"
        ));
    }
    if !cursor.at("{") {
        return Err("generator requires a statement body".into());
    }
    let (services, body) = ordinary_body(cursor)?;
    for service in &services {
        if !matches!(service.as_str(), "clock" | "logger") {
            return Err(format!("generator does not admit injectable {service}"));
        }
    }
    Ok((services, body))
}

/// Parse a composition body and reject unsupported trailing syntax.
pub fn composition_body(cursor: &mut Cursor<'_>) -> Result<Vec<Stmt>, String> {
    let statements = if cursor.take("=>") {
        cursor.lines();
        vec![Stmt::Return(cursor.expr()?)]
    } else {
        cursor.block()?
    };
    cursor.lines();
    if !cursor.at("") {
        return Err("unsupported graph body suffix".into());
    }
    Ok(statements)
}

/// Bind an already ordered invocation to its declared parameter names.
pub fn parameter_scope<T>(
    signature: &Signature,
    values: impl IntoIterator<Item = T>,
) -> std::collections::BTreeMap<String, T> {
    signature
        .parameters
        .iter()
        .map(|p| p.name.clone())
        .zip(values)
        .collect()
}
/// Establish ordinary helper bindings, preserving constant values outside preparation.
pub fn value_scope(
    signature: &Signature,
    args: &[crate::ir::Value],
    positions: &[usize],
    preparation: bool,
) -> std::collections::BTreeMap<String, crate::ir::Value> {
    parameter_scope(
        signature,
        signature.parameters.iter().zip(positions).map(|(p, id)| {
            let value = &args[*id];
            if p.constant && !preparation {
                value.clone()
            } else {
                crate::ir::Value::new(value.ty.clone(), crate::ir::Kind::Local(*id))
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::statement_constructs;
    use hgl_source::lex;

    #[test]
    fn classification_does_not_replace_identifier_admission() {
        // Reserved-name rejection is the parser's responsibility, not phase selection.
        for word in ["state", "cache", "start", "stop", "when", "inject", "for"] {
            for body in [
                format!("{{let {word}=1}}"),
                format!("{{Thing(\n{word}:1\n)}}"),
                format!("{{{word}(1)}}"),
                format!("{{let value=object.{word}}}"),
            ] {
                let tokens = lex(&body).unwrap();
                assert!(statement_constructs(&tokens).is_empty(), "{body}");
            }
        }
    }

    #[test]
    fn nested_statement_constructs_keep_their_source_order() {
        let tokens = lex("{inject clock\nstate x:i64=1\ncache y:i64=2\nstart {}\nwhen (true) {while true {yield 0us:x}}\nstop {}\nfor item in items {}}")
            .unwrap();
        assert_eq!(
            statement_constructs(&tokens),
            [
                "inject", "state", "cache", "start", "when", "while", "yield", "stop", "for"
            ]
        );
    }
}
