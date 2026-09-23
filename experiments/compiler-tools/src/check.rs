use crate::model::{Decl, Expr, Parsed, Ty};
use std::collections::{BTreeMap, BTreeSet};

pub fn check(parsed: &Parsed) -> Result<Vec<Decl>, String> {
    instantiate(parsed, &BTreeMap::new())
}
pub fn instantiate(parsed: &Parsed, arguments: &BTreeMap<String, Ty>) -> Result<Vec<Decl>, String> {
    if !parsed.issues.is_empty() {
        return Err("syntax errors".into());
    }
    let mut structs = BTreeMap::new();
    let mut names = BTreeSet::new();
    for d in &parsed.declarations {
        let name = match d {
            Decl::Struct(n, fields) => {
                structs.insert(n.clone(), fields.clone());
                n
            }
            Decl::Function { name, .. } => name,
        };
        if !names.insert(name) {
            return Err(format!("duplicate declaration {name}"));
        }
    }
    fn source_type(t: &Ty) -> Result<(), String> {
        match t {
            Ty::Ref(inner) if matches!(inner.as_ref(), Ty::Ref(_)) => {
                Err("explicit nested ref is invalid".into())
            }
            Ty::Ref(inner) | Ty::List(inner, _) => source_type(inner),
            _ => Ok(()),
        }
    }
    for declaration in &parsed.declarations {
        match declaration {
            Decl::Struct(_, fields) => {
                for (_, t) in fields {
                    source_type(t)?;
                }
            }
            Decl::Function { params, result, .. } => {
                for (_, t) in params {
                    source_type(t)?;
                }
                source_type(result)?;
            }
        }
    }
    fn canonical(
        t: &Ty,
        defs: &BTreeMap<String, Vec<(String, Ty)>>,
        stack: &mut Vec<String>,
        arguments: &BTreeMap<String, Ty>,
    ) -> Result<Ty, String> {
        Ok(match t {
            Ty::I64 => Ty::I64,
            Ty::Named(name) => {
                if let Some(t) = arguments.get(name) {
                    return canonical(t, defs, stack, &BTreeMap::new());
                }
                if stack.contains(name) {
                    return Err(format!("recursive structure {name}"));
                }
                let fields = defs
                    .get(name)
                    .ok_or_else(|| format!("unknown type {name}"))?;
                stack.push(name.clone());
                let mut names = BTreeSet::new();
                let mut result = Vec::new();
                for (n, t) in fields {
                    if !names.insert(n) {
                        return Err(format!("duplicate field {n}"));
                    }
                    result.push((n.clone(), canonical(t, defs, stack, arguments)?));
                }
                stack.pop();
                Ty::Bundle(result)
            }
            Ty::Ref(t) => match canonical(t, defs, stack, arguments)? {
                inner @ Ty::Ref(_) => inner,
                inner => Ty::Ref(Box::new(inner)),
            },
            Ty::List(t, n) => Ty::List(Box::new(canonical(t, defs, stack, arguments)?), *n),
            Ty::Bundle(_) => return Err("already lowered type".into()),
        })
    }
    fn infer(e: &Expr, params: &BTreeMap<String, Ty>) -> Result<Ty, String> {
        match e {
            Expr::Int(_) => Ok(Ty::I64),
            Expr::Name(n) => params
                .get(n)
                .cloned()
                .ok_or_else(|| format!("unknown name {n}")),
            Expr::Add(a, b) | Expr::Mul(a, b) => {
                if infer(a, params)? != Ty::I64 || infer(b, params)? != Ty::I64 {
                    return Err("arithmetic requires i64".into());
                }
                Ok(Ty::I64)
            }
        }
    }
    let mut result = Vec::new();
    for d in &parsed.declarations {
        match d {
            Decl::Struct(n, _) => {
                canonical(
                    &Ty::Named(n.clone()),
                    &structs,
                    &mut Vec::new(),
                    &BTreeMap::new(),
                )?;
            }
            Decl::Function {
                name,
                generics,
                params,
                result: ty,
                body,
            } => {
                if generics.iter().collect::<BTreeSet<_>>().len() != generics.len() {
                    return Err("duplicate generic parameter".into());
                }
                let substitutions = generics
                    .iter()
                    .map(|g| {
                        arguments
                            .get(g)
                            .cloned()
                            .map(|t| (g.clone(), t))
                            .ok_or_else(|| format!("missing type argument {g}"))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let params: Vec<_> = params
                    .iter()
                    .map(|(n, t)| {
                        Ok((
                            n.clone(),
                            canonical(t, &structs, &mut Vec::new(), &substitutions)?,
                        ))
                    })
                    .collect::<Result<_, String>>()?;
                let lookup: BTreeMap<_, _> = params.iter().cloned().collect();
                if lookup.len() != params.len() {
                    return Err("duplicate parameter".into());
                }
                let ty = canonical(ty, &structs, &mut Vec::new(), &substitutions)?;
                if infer(body, &lookup)? != ty {
                    return Err(format!("return type mismatch in {name}"));
                }
                result.push(Decl::Function {
                    name: name.clone(),
                    generics: Vec::new(),
                    params,
                    result: ty,
                    body: body.clone(),
                });
            }
        }
    }
    Ok(result)
}
