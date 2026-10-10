//! Descriptor-only native scalar and exact ordinary helper contracts.
use crate::library::{Decl, Library, Role, Signature};
use hgl_source::{Cursor, Issue};
use std::collections::BTreeMap;

pub use hgl_source::native_descriptor::{Function, Scalar};
/// A versioned package descriptor, separate from its implementation image.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// Canonical source module supplied by this package.
    pub module: String,
    /// Selected provider identity retained by consumers.
    pub provider: String,
    /// Semantic interface fingerprint checked again before execution.
    pub fingerprint: String,
    /// Native declaration bindings in the ordinary module namespace.
    pub scalars: BTreeMap<String, Scalar>,
    /// Exact helper bindings keyed by their source declaration name.
    pub functions: BTreeMap<String, Function>,
}
impl Descriptor {
    /// Require the selected image to implement the exact checked interface.
    pub fn verify_image(&self, provider: &str, fingerprint: &str) -> Result<(), String> {
        if self.provider != provider || self.fingerprint != fingerprint {
            return Err("native descriptor/provider identity or fingerprint mismatch".into());
        }
        Ok(())
    }
}
/// Checked package descriptors indexed by their ordinary module identity.
pub type Contracts = BTreeMap<String, Descriptor>;
/// Identify a data sidecar in an ordinary compiler source bundle.
pub fn sidecar(source: &str) -> bool {
    source.ends_with(".hgl-native")
}
/// Parse one descriptor using only the source lexer and inert metadata.
pub fn parse(_source: &str, input: &str) -> Result<Descriptor, Issue> {
    let tokens = hgl_source::lex(input)?;
    let mut c = Cursor::new(&tokens);
    hgl_source::native_descriptor::version(&mut c)?;
    c.need("module")?;
    let mut module = c.name()?;
    while c.take(".") {
        module.push('.');
        module.push_str(&c.name()?);
    }
    c.lines();
    c.need("provider")?;
    let provider = hgl_source::native_descriptor::text(&mut c)?;
    c.need("fingerprint")?;
    let fingerprint = hgl_source::native_descriptor::text(&mut c)?;
    c.lines();
    let mut descriptor = Descriptor {
        module,
        provider,
        fingerprint,
        scalars: BTreeMap::new(),
        functions: BTreeMap::new(),
    };
    while !c.at("") {
        if c.take("scalar") {
            let (name, scalar) = hgl_source::native_descriptor::scalar(&mut c)?;
            if descriptor.scalars.insert(name, scalar).is_some() {
                return Err("duplicate native scalar descriptor binding".into());
            }
        } else {
            c.need("value")?;
            let (name, function) = hgl_source::native_descriptor::function(&mut c)?;
            if descriptor.functions.insert(name, function).is_some() {
                return Err("duplicate native helper descriptor binding".into());
            }
        }
        if !c.at("\n") && !c.at("") {
            return Err("unexpected native descriptor suffix".into());
        }
        c.lines();
    }
    Ok(descriptor)
}
/// Validate aliases and declaration bindings before any provider activation.
pub fn validate(library: &Library) -> Result<(), Issue> {
    let mut identities = BTreeMap::new();
    for descriptor in library.native_contracts.values() {
        for scalar in descriptor.scalars.values() {
            if identities
                .insert(&scalar.identity, &scalar.capabilities)
                .is_some_and(|old| old != &scalar.capabilities)
            {
                return Err(
                    "aliases of one canonical native scalar require the same capabilities".into(),
                );
            }
        }
    }
    for declaration in &library.declarations {
        if declaration.role == Role::NativeType {
            declaration.native_type()?;
            crate::struct_names::native_declaration(
                library,
                &declaration.module,
                &declaration.name,
            )?;
            scalar(library, declaration)?;
        } else if declaration.role == Role::Native {
            let signature = declaration.signature_checked()?;
            function(library, declaration, &signature)?;
        }
    }
    for descriptor in library.native_contracts.values() {
        for (name, role) in descriptor
            .scalars
            .keys()
            .map(|name| (name, Role::NativeType))
            .chain(descriptor.functions.keys().map(|name| (name, Role::Native)))
        {
            if !library
                .declarations
                .iter()
                .any(|d| d.module == descriptor.module && d.name == *name && d.role == role)
            {
                return Err(format!(
                    "{}::{name}: descriptor has no native interface declaration",
                    descriptor.module
                )
                .into());
            }
        }
    }
    Ok(())
}
/// Obtain one exact canonical binding, never inferring capabilities from target code.
pub fn scalar<'a>(library: &'a Library, declaration: &Decl) -> Result<&'a Scalar, Issue> {
    library
        .native_contracts
        .get(&declaration.module)
        .and_then(|descriptor| descriptor.scalars.get(&declaration.name))
        .ok_or_else(|| {
            Issue::from(format!(
                "{}::{}: missing native scalar provider descriptor",
                declaration.module, declaration.name
            ))
            .at(declaration.tokens.first().map_or(0..0, |t| t.span.clone()))
            .in_source(&declaration.source)
        })
}
fn canonical(library: &Library, module: &str, name: &str) -> Result<String, Issue> {
    if let Some(declaration) = crate::struct_names::native_declaration(library, module, name)? {
        return Ok(scalar(library, declaration)?.identity.clone());
    }
    if let Some(ty) = hgl_source::Ty::parse(name)
        && ty.scalar_kind().is_some()
    {
        return Ok(ty.source_name());
    }
    Err(format!("native descriptor requires an exact canonical scalar value type: {name}").into())
}
/// Bind a declaration to inert provider metadata using canonical exact types.
pub fn function<'a>(
    library: &'a Library,
    declaration: &Decl,
    signature: &Signature,
) -> Result<Option<&'a Function>, Issue> {
    let selected = library
        .native_contracts
        .get(&declaration.module)
        .and_then(|d| d.functions.get(&declaration.name));
    let Some(selected) = selected else {
        let names = signature
            .parameters
            .iter()
            .map(|p| p.ty.as_str())
            .chain(std::iter::once(signature.result.as_str()));
        for name in names {
            if crate::struct_names::native_declaration(library, &declaration.module, name)?
                .is_some()
            {
                return Err("missing exact native helper provider descriptor".into());
            }
        }
        return Ok(None);
    };
    if !signature.generics.is_empty()
        || signature.parameters.len() != selected.parameters.len()
        || signature.throws != selected.throws
        || canonical(library, &declaration.module, &signature.result)?
            != canonical(library, &declaration.module, &selected.result)?
    {
        return Err("incompatible native helper provider signature".into());
    }
    for (source, provider) in signature.parameters.iter().zip(&selected.parameters) {
        if source.name != provider.0
            || source.constant
            || source.default.is_some()
            || canonical(library, &declaration.module, &source.ty)?
                != canonical(library, &declaration.module, &provider.1)?
        {
            return Err("incompatible native helper provider parameter contract".into());
        }
    }
    Ok(Some(selected))
}
