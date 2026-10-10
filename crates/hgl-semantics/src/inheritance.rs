//! Scoped single-inheritance fields and invariant ancestor patterns.
use crate::library::{Decl, Library, RequiredStruct};
use hgl_source::{Ty, application};
use std::collections::{BTreeMap, BTreeSet};
/// Type syntax whose symbols retain their declaration scope through substitution.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// One parameter of the selected concrete root.
    Parameter(String),
    /// Declaration scope, base spelling and invariant argument patterns.
    Named(String, String, Vec<Self>),
}
impl Pattern {
    /// Resolve the finite tree using checked concrete root arguments.
    pub fn resolve(
        &self,
        bindings: &BTreeMap<String, Ty>,
        resolve: &mut impl FnMut(&str, &str, &[Ty]) -> Result<Ty, String>,
    ) -> Result<Ty, String> {
        match self {
            Self::Parameter(name) => bindings
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unresolved struct type parameter {name}")),
            Self::Named(module, name, args) => {
                let args = args
                    .iter()
                    .map(|arg| arg.resolve(bindings, resolve))
                    .collect::<Result<Vec<_>, _>>()?;
                resolve(module, name, &args)
            }
        }
    }
    /// Infer root parameters after the caller matches one scoped type application.
    pub fn infer(
        &self,
        actual: &Ty,
        bindings: &mut BTreeMap<String, Ty>,
        decompose: &mut impl FnMut(&str, &str, usize, &Ty) -> Result<Vec<Ty>, String>,
    ) -> Result<(), String> {
        match self {
            Self::Parameter(name) => {
                if bindings
                    .insert(name.clone(), actual.clone())
                    .is_some_and(|old| old != *actual)
                {
                    return Err(format!("conflicting struct inference for {name}"));
                }
            }
            Self::Named(module, name, args) => {
                let children = decompose(module, name, args.len(), actual)?;
                if children.len() != args.len() {
                    return Err("inherited field type arity mismatch".into());
                }
                for (pattern, actual) in args.iter().zip(children) {
                    pattern.infer(&actual, bindings, decompose)?;
                }
            }
        }
        Ok(())
    }
}
fn pattern(module: &str, name: &str, bindings: &BTreeMap<String, Pattern>) -> Pattern {
    if let Some(value) = bindings.get(name) {
        return value.clone();
    }
    if let Some((base, args)) = application(name)
        && matches!(base, "list" | "rolling")
        && args.len() >= 2
    {
        return Pattern::Named(
            module.into(),
            format!("{base}<{}>", args[1..].join(",")),
            vec![pattern(module, args[0], bindings)],
        );
    }
    let (base, args) = application(name).unwrap_or((name, Vec::new()));
    Pattern::Named(
        module.into(),
        base.into(),
        args.into_iter()
            .map(|arg| pattern(module, arg, bindings))
            .collect(),
    )
}
/// Flatten fields in ancestor-first order while preserving all symbol scopes.
pub fn schema(library: &Library, decl: &Decl) -> Result<(RequiredStruct, Vec<Pattern>), String> {
    let own = decl.required_struct()?;
    let bindings = own
        .generics
        .iter()
        .map(|p| (p.clone(), Pattern::Parameter(p.clone())))
        .collect();
    flatten(library, decl, &bindings, &mut BTreeSet::new())
}
fn flatten(
    library: &Library,
    decl: &Decl,
    bindings: &BTreeMap<String, Pattern>,
    active: &mut BTreeSet<String>,
) -> Result<(RequiredStruct, Vec<Pattern>), String> {
    let id = format!("{}::{}", decl.module, decl.name);
    if !active.insert(id.clone()) {
        return Err("cyclic struct inheritance".into());
    }
    let mut own = decl.required_struct()?;
    crate::struct_names::exported_fields(library, decl)?;
    for (_, value) in &mut own.defaults {
        if matches!(value.syntax(), hgl_source::Expr::Name(_)) {
            *value = match crate::enums::default(library, &decl.module, value)? {
                hgl_source::ParsedLiteral::Value(value) => hgl_source::Expr::Literal(value),
                hgl_source::ParsedLiteral::Contextual(value) => {
                    hgl_source::Expr::TemporalLiteral(value)
                }
            };
        }
    }
    let mut patterns = own
        .fields
        .iter()
        .map(|(_, name)| pattern(&decl.module, name, bindings))
        .collect::<Vec<_>>();
    if let Some(parent) = &own.parent {
        let (base, args) = application(parent).unwrap_or((parent.as_str(), Vec::new()));
        let parent = crate::struct_names::declaration(library, &decl.module, base)?
            .ok_or("unresolved struct ancestor")?;
        let parameters = abstract_schema(parent)?.generics;
        if parameters.len() != args.len() {
            return Err("ancestor requires complete type arguments".into());
        }
        let bindings = parameters
            .into_iter()
            .zip(
                args.into_iter()
                    .map(|arg| pattern(&decl.module, arg, bindings)),
            )
            .collect();
        let (mut inherited, mut inherited_patterns) = flatten(library, parent, &bindings, active)?;
        if own
            .fields
            .iter()
            .any(|(name, _)| inherited.fields.iter().any(|(old, _)| old == name))
        {
            return Err("inherited field redeclaration is unsupported".into());
        }
        let offset = inherited.fields.len();
        inherited.fields.append(&mut own.fields);
        inherited
            .optional
            .extend(own.optional.iter().map(|index| index + offset));
        inherited.defaults.extend(
            own.defaults
                .into_iter()
                .map(|(index, value)| (index + offset, value)),
        );
        inherited_patterns.append(&mut patterns);
        own.fields = inherited.fields;
        own.optional = inherited.optional;
        own.defaults = inherited.defaults;
        patterns = inherited_patterns;
    }
    active.remove(&id);
    Ok((own, patterns))
}
/// Exact scoped ancestor applications in nearest-first order.
pub fn ancestors(library: &Library, decl: &Decl) -> Result<Vec<Pattern>, String> {
    let own = decl.required_struct()?;
    let mut bindings = own
        .generics
        .iter()
        .map(|p| (p.clone(), Pattern::Parameter(p.clone())))
        .collect();
    let mut current = decl;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    while let Some(parent) = current.required_struct()?.parent {
        if !seen.insert(format!("{}::{}", current.module, current.name)) {
            return Err("cyclic struct inheritance".into());
        }
        out.push(pattern(&current.module, &parent, &bindings));
        let (base, args) = application(&parent).unwrap_or((&parent, Vec::new()));
        let next = crate::struct_names::declaration(library, &current.module, base)?
            .ok_or("unresolved struct ancestor")?;
        let parameters = abstract_schema(next)?.generics;
        if parameters.len() != args.len() {
            return Err("ancestor requires complete type arguments".into());
        }
        bindings = parameters
            .into_iter()
            .zip(
                args.into_iter()
                    .map(|arg| pattern(&current.module, arg, &bindings)),
            )
            .collect();
        current = next;
    }
    Ok(out)
}

fn abstract_schema(decl: &Decl) -> Result<RequiredStruct, String> {
    let schema = decl.required_struct()?;
    if !schema.abstract_type {
        return Err("struct inheritance requires an abstract base".into());
    }
    Ok(schema)
}
