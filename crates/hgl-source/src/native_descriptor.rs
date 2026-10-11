//! Inert native contract records parsed independently of provider code.
use crate::{Cursor, Expr, Issue, Literal};
pub use hgl_value_model::native_contracts::{Function, Scalar};
use std::collections::BTreeSet;
/// Admit the exact descriptor format and target before reading its records.
pub fn version(c: &mut Cursor<'_>) -> Result<(), Issue> {
    c.lines();
    c.need("native_descriptor")?;
    c.need("1")?;
    c.need("target")?;
    if text(c)? != "hgl-rust-1" {
        return Err("incompatible native descriptor target".into());
    }
    c.lines();
    Ok(())
}
/// Parse a nonempty quoted descriptor identity without evaluating any helper.
pub fn text(c: &mut Cursor<'_>) -> Result<String, Issue> {
    let Expr::Literal(Literal::Str(text)) = c.expr()?.syntax().clone() else {
        return Err("native descriptor requires quoted text".into());
    };
    if text.is_empty() {
        return Err("native descriptor identity must not be empty".into());
    }
    Ok(text)
}
fn names(c: &mut Cursor<'_>, allowed: &[&str]) -> Result<BTreeSet<String>, Issue> {
    c.need("{")?;
    c.lines();
    let mut values = BTreeSet::new();
    while !c.take("}") {
        let name = c.name()?;
        if !allowed.contains(&name.as_str()) || !values.insert(name) {
            return Err("native descriptor has an unknown or duplicate contract member".into());
        }
        c.lines();
        if !c.take(",") {
            c.need("}")?;
            break;
        }
        c.lines();
    }
    Ok(values)
}
/// Parse a scalar record with mandatory owning copy and text capabilities.
pub fn scalar(c: &mut Cursor<'_>) -> Result<(String, Scalar), Issue> {
    let name = c.name()?;
    c.need("canonical")?;
    let identity = text(c)?;
    c.need("capabilities")?;
    let capabilities = names(
        c,
        &["copy", "text", "equality", "hash", "order", "serialization"],
    )?;
    if !capabilities.contains("copy") || !capabilities.contains("text") {
        return Err("native scalar requires owning copy and text operations".into());
    }
    Ok((
        name,
        Scalar {
            identity,
            capabilities,
        },
    ))
}
fn value_type(c: &mut Cursor<'_>) -> Result<String, Issue> {
    let ty = c.type_name()?;
    if !crate::value_type(&ty) {
        return Err("native descriptor requires ordinary value types".into());
    }
    Ok(ty)
}
/// Parse an exact readonly value signature and its owned target contract.
pub fn function(c: &mut Cursor<'_>) -> Result<(String, Function), Issue> {
    let name = c.name()?;
    c.need("(")?;
    let mut parameters: Vec<(String, String)> = Vec::new();
    while !c.take(")") {
        let name = c.name()?;
        if parameters.iter().any(|(declared, _)| *declared == name) {
            return Err("duplicate native descriptor parameter".into());
        }
        c.need(":")?;
        parameters.push((name, value_type(c)?));
        if !c.take(",") {
            c.need(")")?;
            break;
        }
    }
    c.need("->")?;
    let result = value_type(c)?;
    let throws = c.take("throws");
    c.need("symbol")?;
    let symbol = symbol(c)?;
    let bound = if c.take("bound") {
        Some(self::symbol(c)?)
    } else {
        None
    };
    c.need("ownership")?;
    c.need("owned")?;
    c.need("phases")?;
    let phases = names(c, &["setup", "cold", "evaluation", "start", "stop"])?;
    if phases.is_empty() {
        return Err("native helper requires a permitted phase".into());
    }
    c.need("effects")?;
    let effects = names(c, &["allocation", "io", "blocking", "mutation"])?;
    if effects.contains("blocking") && phases.contains("evaluation") {
        return Err("native evaluation helpers must be nonblocking".into());
    }
    if effects.contains("mutation") {
        return Err("ordinary native scalar helpers require readonly arguments".into());
    }
    Ok((
        name,
        Function {
            parameters,
            result,
            throws,
            symbol,
            bound,
            phases,
            effects,
        },
    ))
}

/// Parse an explicit Rust target identifier without inferring it from source names.
pub fn symbol(c: &mut Cursor<'_>) -> Result<String, Issue> {
    let symbol = text(c)?;
    if !symbol
        .bytes()
        .enumerate()
        .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
    {
        return Err("native descriptor symbol requires a Rust identifier".into());
    }
    Ok(symbol)
}
