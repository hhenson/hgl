//! Checked nominal enum identities; no enum lookup occurs on graph ticks.
use crate::library::{Decl, Library, Role};
use hgl_source::{Cursor, EnumType, Expr, Literal, ParsedLiteral};
use std::collections::BTreeSet;

fn parse(decl: &Decl) -> Result<EnumType, String> {
    let mut cursor = Cursor::new(&decl.tokens);
    cursor.take("export");
    cursor.need("enum")?;
    cursor.name()?;
    cursor.lines();
    cursor.need("{")?;
    cursor.lines();
    let mut members = Vec::new();
    let mut names = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    let mut previous: Option<i64> = None;
    while !cursor.take("}") {
        let name = cursor.name()?;
        let number = if cursor.take("=") {
            let Some(Literal::Int(value)) = cursor.expr()?.fixed() else {
                return Err("enum assignment requires an i64 constant".into());
            };
            value
        } else {
            previous
                .map_or(Some(0), |value| value.checked_add(1))
                .ok_or("enum automatic numbering overflow")?
        };
        if !names.insert(name.clone()) || !numbers.insert(number) {
            return Err("duplicate enum member name or assigned number".into());
        }
        members.push((name, number));
        previous = Some(number);
        cursor.lines();
        if !cursor.take(",") {
            cursor.need("}")?;
            break;
        }
        cursor.lines();
    }
    cursor.lines();
    if !cursor.at("") {
        return Err("unsupported enum declaration suffix".into());
    }
    Ok(EnumType {
        origin: format!("{}::{}", decl.module, decl.name),
        members,
    })
}
/// Validate every indexed declaration, including unused enums.
pub fn validate(library: &Library) -> Result<(), String> {
    for declaration in &library.declarations {
        if declaration.role == Role::Enum {
            if library
                .declarations
                .iter()
                .filter(|candidate| {
                    candidate.module == declaration.module
                        && candidate.name == declaration.name
                        && matches!(candidate.role, Role::Enum | Role::Struct)
                })
                .count()
                != 1
            {
                return Err(format!(
                    "duplicate nominal declaration {}::{}",
                    declaration.module, declaration.name
                ));
            }
            parse(declaration)?;
        }
    }
    Ok(())
}
/// Resolve a visible declared enum, retaining its canonical identity.
pub fn resolve(library: &Library, module: &str, name: &str) -> Result<Option<EnumType>, String> {
    crate::struct_names::enum_declaration(library, module, name)?
        .map(parse)
        .transpose()
}
/// Resolve a qualified member constant without converting its assigned number.
pub fn member(library: &Library, module: &str, name: &str) -> Result<Option<Literal>, String> {
    let Some((owner, member)) = name.rsplit_once("::") else {
        return Ok(None);
    };
    let Some(ty) = resolve(library, module, owner)? else {
        return Ok(None);
    };
    let number = ty
        .members
        .iter()
        .find(|(name, _)| name == member)
        .map(|(_, value)| *value)
        .ok_or_else(|| format!("unknown enum member {name}"))?;
    Ok(Some(Literal::Enum(ty, number)))
}
/// Resolve a supported scalar default in its declaring module.
pub fn default(library: &Library, module: &str, expr: &Expr) -> Result<ParsedLiteral, String> {
    match expr {
        Expr::Literal(value) => Ok(ParsedLiteral::Value(value.clone())),
        Expr::TemporalLiteral(value) => Ok(ParsedLiteral::Contextual(value.clone())),
        Expr::Name(name) => member(library, module, name)?
            .map(ParsedLiteral::Value)
            .ok_or_else(|| format!("unsupported scalar default {name}")),
        Expr::Null
        | Expr::Property(..)
        | Expr::Index(..)
        | Expr::Sequence(_)
        | Expr::Sparse(_)
        | Expr::Tuple(_)
        | Expr::Unary(..)
        | Expr::Call(..)
        | Expr::Applied(..)
        | Expr::Binary(..) => Err("unsupported scalar default expression".into()),
    }
}

/// Reject enum operations outside the declared-member publication profile.
pub fn check_call(
    library: &Library,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
) -> Result<(), String> {
    if resolve(library, module, name)?.is_some() {
        return Err("enum construction is unsupported; use a qualified declared member".into());
    }
    if matches!(name, "keys" | "values" | "elements")
        && let [(_, Expr::Name(owner))] = args
        && resolve(library, module, owner)?.is_some()
    {
        return Err("enum enumeration is unsupported in this backend".into());
    }
    Ok(())
}
