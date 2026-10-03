//! Contextual specialization and field checking for ordinary constructors.
use hgl_library::{Decl, Library, RequiredStruct};
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Expr, Ty, application};
use hgl_value_types::{declaration, specialize, substitute, unify};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
/// An inference session; checking values remains the caller's responsibility.
pub struct Constructor {
    module: String,
    declaration: Decl,
    schema: RequiredStruct,
    bindings: BTreeMap<String, Ty>,
    fields: Vec<usize>,
    values: Vec<Option<Value>>,
}
impl Constructor {
    /// Resolve a constructor and validate its field names before checking values.
    pub fn new(
        library: &Library,
        module: &str,
        name: &str,
        args: &[(Option<String>, Expr)],
        expected: Option<&Ty>,
        ambient: &BTreeMap<String, Ty>,
    ) -> Result<Self, String> {
        let (base, explicit) =
            application(name).map_or((name, None), |(base, args)| (base, Some(args)));
        let declaration = declaration(library, module, base)?.ok_or("explicit generic arguments require a struct constructor; generic function calls are not admitted")?.clone();
        let schema = declaration.required_struct()?;
        let mut bindings = BTreeMap::new();
        if let Some(explicit) = explicit {
            if explicit.len() != schema.generics.len() {
                return Err(format!(
                    "{base}: struct application requires {} complete type arguments",
                    schema.generics.len()
                ));
            }
            for (parameter, argument) in schema.generics.iter().zip(explicit) {
                if argument == "_" {
                    return Err("struct arguments require complete concrete types; placeholders are not admitted".into());
                }
                bindings.insert(
                    parameter.clone(),
                    substitute(library, module, argument, ambient, &mut BTreeSet::new())?,
                );
            }
        }
        if let Some(Ty::Struct(identity, _)) = expected
            && !schema.generics.is_empty()
        {
            if identity.origin != format!("{}::{}", declaration.module, declaration.name) {
                return Err("constructor expected nominal type mismatch".into());
            }
            for (parameter, argument) in schema.generics.iter().zip(&identity.arguments) {
                if bindings
                    .insert(parameter.clone(), argument.clone())
                    .is_some_and(|old| old != *argument)
                {
                    return Err(format!("conflicting struct inference for {parameter}"));
                }
            }
        }
        let mut fields = Vec::new();
        for (name, _) in args {
            let name = name
                .as_ref()
                .ok_or("struct construction requires named fields")?;
            let index = schema
                .fields
                .iter()
                .position(|(field, _)| field == name)
                .ok_or_else(|| format!("unknown argument {name}"))?;
            if fields.contains(&index) {
                return Err(format!("duplicate struct field {name}"));
            }
            fields.push(index);
        }
        if fields.len() != schema.fields.len() {
            return Err("struct construction: missing or wrong-type argument".into());
        }
        Ok(Self {
            module: module.into(),
            declaration,
            schema,
            bindings,
            fields,
            values: vec![None; args.len()],
        })
    }
    /// Select one unchecked field and its available concrete expected type.
    pub fn next(
        &mut self,
        library: &Library,
        args: &[(Option<String>, Expr)],
    ) -> Result<Option<(usize, Option<Ty>)>, String> {
        if let Some(arguments) = self
            .schema
            .generics
            .iter()
            .map(|name| self.bindings.get(name).cloned())
            .collect::<Option<Vec<_>>>()
        {
            specialize(library, &self.declaration, arguments, &mut BTreeSet::new())?;
        }
        for (index, (_, expr)) in args.iter().enumerate() {
            if self.values[index].is_none() {
                self.literal_evidence(
                    library,
                    &self.schema.fields[self.fields[index]].1.clone(),
                    expr,
                )?;
            }
        }
        for index in 0..self.fields.len() {
            if self.values[index].is_none()
                && let Ok(ty) = substitute(
                    library,
                    &self.declaration.module,
                    &self.schema.fields[self.fields[index]].1,
                    &self.bindings,
                    &mut BTreeSet::new(),
                )
            {
                return Ok(Some((index, Some(ty))));
            }
        }
        let pending = self
            .values
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_none())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if let Some(index) = pending
            .iter()
            .find(|&&index| !needs_context(library, &self.module, &args[index].1))
        {
            return Ok(Some((*index, None)));
        }
        if let Some(index) = pending.first() {
            return Ok(Some((*index, None)));
        }
        Ok(None)
    }
    fn literal_evidence(
        &mut self,
        library: &Library,
        pattern: &str,
        expr: &Expr,
    ) -> Result<(), String> {
        if let (Some((element, _)), Expr::Sequence(values)) = (Ty::list_parts(pattern), expr) {
            for expr in values.iter().flatten() {
                if let Some(literal) = expr.fixed() {
                    unify(
                        library,
                        &self.declaration.module,
                        element,
                        &literal.ty(),
                        &self.schema.generics,
                        &mut self.bindings,
                    )?;
                } else {
                    self.literal_evidence(library, element, expr)?;
                }
            }
        }
        Ok(())
    }
    /// Unify and retain the checked field once, without executing its expression.
    pub fn checked(&mut self, library: &Library, index: usize, value: Value) -> Result<(), String> {
        let pattern = &self.schema.fields[self.fields[index]].1;
        unify(
            library,
            &self.declaration.module,
            pattern,
            &value.ty,
            &self.schema.generics,
            &mut self.bindings,
        )
        .map_err(|error| format!("struct construction: missing or wrong-type argument: {error}"))?;
        self.values[index] = Some(value);
        Ok(())
    }
    /// Validate complete inference and assemble source-order constructor IR.
    pub fn finish(self, library: &Library) -> Result<Value, String> {
        let arguments = self
            .schema
            .generics
            .iter()
            .map(|name| {
                self.bindings
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("unresolved struct type parameter {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ty = specialize(library, &self.declaration, arguments, &mut BTreeSet::new())?;
        let fields = self
            .fields
            .into_iter()
            .zip(self.values)
            .map(|(index, value)| {
                value
                    .map(|value| (index, value))
                    .ok_or_else(|| "unchecked struct field".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Value::new(ty, Kind::Construct(fields)))
    }
}
fn needs_context(library: &Library, module: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Sequence(_) | Expr::Null => true,
        Expr::Call(name, args) => {
            if let Ok(Some(decl)) = declaration(library, module, name)
                && let Ok(schema) = decl.required_struct()
            {
                let evidence = args
                    .iter()
                    .filter(|(_, expr)| !needs_context(library, module, expr))
                    .filter_map(|(name, _)| {
                        schema
                            .fields
                            .iter()
                            .find(|(field, _)| name.as_ref() == Some(field))
                    })
                    .flat_map(|(_, pattern)| {
                        pattern.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    })
                    .collect::<BTreeSet<_>>();
                return schema
                    .generics
                    .iter()
                    .any(|parameter| !evidence.contains(parameter.as_str()));
            }
            args.iter()
                .any(|(_, expr)| needs_context(library, module, expr))
        }
        Expr::Applied(..)
        | Expr::Literal(_)
        | Expr::Name(_)
        | Expr::Property(..)
        | Expr::Index(..)
        | Expr::Unary(..)
        | Expr::Binary(..) => false,
    }
}
