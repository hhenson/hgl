//! Descriptor-only native scalar and exact ordinary helper contracts.
use crate::library::{Decl, Library, Role, Signature};
use hgl_source::{Cursor, Expr, Issue, Literal};
use std::collections::{BTreeMap, BTreeSet};

/// Owning scalar operations advertised by the portable package contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scalar {
    /// Existing canonical scalar identity, independent of the HGL binding name.
    pub identity: String,
    /// Shared operations available to checked source contexts.
    pub capabilities: BTreeSet<String>,
}
/// One exact ordinary function supplied by the selected provider.
#[derive(Debug, Clone)]
pub struct Function {
    /// Exact portable value signature; no body is loaded or evaluated.
    pub signature: Signature,
    /// Reviewed target symbol, bound once before execution.
    pub symbol: String,
    /// Explicit execution contexts, excluding required constant evaluation.
    pub phases: BTreeSet<String>,
    /// Observable provider effects, checked independently of const syntax.
    pub effects: BTreeSet<String>,
}
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
fn text(c: &mut Cursor<'_>) -> Result<String, Issue> {
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
fn scalar_record(c: &mut Cursor<'_>) -> Result<(String, Scalar), Issue> {
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
fn function_record(
    source: &str,
    module: &str,
    c: &mut Cursor<'_>,
) -> Result<(String, Function), Issue> {
    let name = c.name()?;
    let start = c.pos;
    while !c.at("symbol") && !c.at("") && !c.at("\n") {
        c.consume()?;
    }
    let mut tokens = hgl_source::lex(&format!("const fn {name}"))?;
    tokens.extend_from_slice(&c.tokens[start..c.pos]);
    let declaration = Decl {
        module: module.into(),
        name: name.clone(),
        role: Role::Function,
        tokens,
        source: source.into(),
        doc: String::new(),
        test_only: false,
    };
    let signature = declaration.signature_checked()?;
    if !signature.body.is_empty()
        || !signature.generics.is_empty()
        || signature
            .parameters
            .iter()
            .any(|p| p.constant || p.default.is_some())
    {
        return Err("native descriptor requires one exact ordinary value signature".into());
    }
    c.need("symbol")?;
    let symbol = text(c)?;
    if !symbol
        .bytes()
        .enumerate()
        .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
    {
        return Err("native descriptor symbol requires a Rust identifier".into());
    }
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
            signature,
            symbol,
            phases,
            effects,
        },
    ))
}
/// Parse one descriptor using only the source lexer and inert metadata.
pub fn parse(source: &str, input: &str) -> Result<Descriptor, Issue> {
    let tokens = hgl_source::lex(input)?;
    let mut c = Cursor::new(&tokens);
    c.lines();
    c.need("native_descriptor")?;
    c.need("1")?;
    c.need("target")?;
    if text(&mut c)? != "hgl-rust-1" {
        return Err("incompatible native descriptor target".into());
    }
    c.lines();
    c.need("module")?;
    let mut module = c.name()?;
    while c.take(".") {
        module.push('.');
        module.push_str(&c.name()?);
    }
    c.lines();
    c.need("provider")?;
    let provider = text(&mut c)?;
    c.need("fingerprint")?;
    let fingerprint = text(&mut c)?;
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
            let (name, scalar) = scalar_record(&mut c)?;
            if descriptor.scalars.insert(name, scalar).is_some() {
                return Err("duplicate native scalar descriptor binding".into());
            }
        } else {
            c.need("value")?;
            let (name, function) = function_record(source, &descriptor.module, &mut c)?;
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
        && ty.atomic_payload()
        && !matches!(
            ty,
            hgl_source::Ty::Atomic(_)
                | hgl_source::Ty::List(..)
                | hgl_source::Ty::Tuple(_)
                | hgl_source::Ty::Map(..)
                | hgl_source::Ty::Set(_)
        )
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
    let names = signature
        .parameters
        .iter()
        .map(|p| p.ty.as_str())
        .chain(std::iter::once(signature.result.as_str()));
    let mut native = false;
    for name in names {
        native |=
            crate::struct_names::native_declaration(library, &declaration.module, name)?.is_some();
    }
    let Some(selected) = selected else {
        return if native {
            Err(format!(
                "{}::{}: missing exact native helper provider descriptor",
                declaration.module, declaration.name
            )
            .into())
        } else {
            Ok(None)
        };
    };
    if !signature.generics.is_empty()
        || signature.parameters.len() != selected.signature.parameters.len()
        || signature.throws != selected.signature.throws
        || canonical(library, &declaration.module, &signature.result)?
            != canonical(library, &declaration.module, &selected.signature.result)?
    {
        return Err("incompatible native helper provider signature".into());
    }
    for (source, provider) in signature
        .parameters
        .iter()
        .zip(&selected.signature.parameters)
    {
        if source.name != provider.name
            || source.constant
            || source.default.is_some()
            || canonical(library, &declaration.module, &source.ty)?
                != canonical(library, &declaration.module, &provider.ty)?
        {
            return Err("incompatible native helper provider parameter contract".into());
        }
    }
    Ok(Some(selected))
}
