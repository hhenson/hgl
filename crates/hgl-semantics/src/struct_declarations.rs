//! Declaration-owned struct grammar and finite constraints.
use hgl_source::{Cursor, Expr, Token};
#[derive(Debug, Clone)]
/// Concrete ordinary struct contract before specialization.
pub struct RequiredStruct {
    /// Abstract declarations cannot be constructed.
    pub abstract_type: bool,
    /// Single declared ancestor application.
    pub parent: Option<String>,
    /// Ordered distinct type parameters.
    pub generics: Vec<String>,
    /// Declaration-ordered source field types.
    pub fields: Vec<(String, String)>,
    /// Fields whose introducing default is null.
    pub optional: Vec<usize>,
    /// Declaration-indexed fixed defaults; null marks absence.
    pub defaults: Vec<(usize, Expr)>,
    /// Supported finite type-domain constraint.
    pub type_domain: Option<(String, Vec<String>)>,
}
/// Parse one complete struct declaration without resolving names.
pub fn parse(tokens: &[Token]) -> Result<RequiredStruct, String> {
    let mut c = Cursor::new(tokens);
    c.take("export");
    let abstract_type = c.take("abstract");
    c.need("struct")?;
    c.name()?;
    let generics = parameters(&mut c)?;
    c.lines();
    let parent = if c.take(":") {
        let parent = c.type_name()?;
        if c.at(",") {
            return Err("multiple struct parents are unsupported".into());
        }
        Some(parent)
    } else {
        None
    };
    c.lines();
    let type_domain = if c.take("requires") {
        if c.tokens.get(c.pos + 1).is_none_or(|t| t.text != "in") {
            return Err("unsupported struct constraint".into());
        }
        Some(parse_domain(&mut c, &generics)?)
    } else {
        None
    };
    c.lines();
    if !c.take("{") {
        return Err("expected struct field body".into());
    }
    let mut fields = Vec::new();
    let mut defaults = Vec::new();
    let mut optional = Vec::new();
    c.lines();
    while !c.take("}") {
        let name = c.name()?;
        c.need(":")?;
        let ty = c.type_name()?;
        if fields.iter().any(|(field, _)| *field == name) {
            return Err(format!("duplicate struct field {name}"));
        }
        if c.take("=") {
            let value = scalar_default(c.expr()?)?;
            if matches!(value, Expr::Null) {
                optional.push(fields.len());
            }
            defaults.push((fields.len(), value));
        }
        if !c.at("}") && !c.at("\n") {
            return Err("expected struct field end".into());
        }
        fields.push((name, ty));
        c.lines();
    }
    c.lines();
    if !c.at("") {
        return Err("unsupported struct declaration suffix".into());
    }
    Ok(RequiredStruct {
        abstract_type,
        parent,
        generics,
        fields,
        optional,
        defaults,
        type_domain,
    })
}
/// Parse a finite domain shared by struct and callable signatures.
pub fn parse_domain(
    c: &mut Cursor<'_>,
    generics: &[String],
) -> Result<(String, Vec<String>), String> {
    let parameter = c.name()?;
    if !generics.contains(&parameter) {
        return Err("type domain requires a generic parameter".into());
    }
    c.need("in")?;
    c.need("{")?;
    let mut types = Vec::new();
    loop {
        c.lines();
        let ty = c.type_name()?;
        if hgl_source::Ty::parse(&ty).is_none() || types.contains(&ty) {
            return Err("type domain requires distinct supported types".into());
        }
        types.push(ty);
        c.lines();
        if !c.take(",") {
            c.need("}")?;
            break;
        }
    }
    Ok((parameter, types))
}

fn scalar_default(expr: Expr) -> Result<Expr, String> {
    if matches!(&expr, Expr::Null | Expr::TemporalLiteral(_) | Expr::Name(_)) {
        return Ok(expr);
    }
    expr.fixed()
        .map(Expr::Literal)
        .ok_or_else(|| "struct defaults require supported non-null fixed scalar expressions".into())
}

fn parameters(c: &mut Cursor<'_>) -> Result<Vec<String>, String> {
    let mut generics = Vec::new();
    if c.take("<") {
        c.lines();
        loop {
            if c.at("const") {
                return Err("const-generic structs are not supported".into());
            }
            let parameter = c.name()?;
            if parameter == "_" || generics.contains(&parameter) {
                return Err("struct type parameters must be distinct names".into());
            }
            generics.push(parameter);
            c.lines();
            if c.at("=") {
                return Err("generic parameter defaults are not supported".into());
            }
            if !c.take(",") {
                c.need(">")?;
                break;
            }
            c.lines();
        }
    }
    Ok(generics)
}
