//! Source function phase and service-header admission.
use hgl_library::Signature;
use hgl_source::{Cursor, Stmt};

/// Classify node-only constructs without evaluating the body.
pub fn runtime_body(signature: &Signature) -> Result<bool, String> {
    if signature.value_function
        && signature
            .body
            .iter()
            .any(|token| matches!(token.text.as_str(), "yield" | "while"))
    {
        return Err("yield/while require a runtime body, not a const value function".into());
    }
    Ok(signature.value_function
        || signature.body.iter().any(|token| {
            matches!(
                token.text.as_str(),
                "state" | "cache" | "inject" | "start" | "when" | "stop" | "yield"
            )
        }))
}

/// Parse an ordinary body, retaining its function-level service declarations.
pub fn ordinary_body(cursor: &mut Cursor<'_>) -> Result<(Vec<String>, Vec<Stmt>), String> {
    if cursor.take("=>") {
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
    if let Some(token) = cursor.tokens.iter().find(|token| {
        matches!(
            token.text.as_str(),
            "state" | "cache" | "start" | "stop" | "when" | "for"
        )
    }) {
        return Err(format!(
            "generator does not admit {}; use while for iteration",
            token.text
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
