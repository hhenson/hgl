//! Descriptor-only native scalar and exact ordinary helper contracts.
use crate::ir::Value;
use crate::library::{Decl, Library, Role, Signature};
use hgl_source::TypeSpelling;
use hgl_source::{Cursor, Issue, Ty};
use std::collections::BTreeMap;

pub use hgl_source::native_descriptor::{Function, Scalar};
pub use hgl_source::native_package::Descriptor;
/// Checked package descriptors indexed by their ordinary module identity.
pub type Contracts = BTreeMap<String, Descriptor>;
#[derive(Clone, Copy, PartialEq)]
enum IdentityKind {
    Scalar,
    Composite,
    Native,
}
fn nominal_identity(library: &Library, name: &str, arguments: usize) -> Option<IdentityKind> {
    for declaration in library.declarations.iter() {
        if name != format!("{}::{}", declaration.module, declaration.name)
            || declaration.test_only && library.test_scope.as_ref() != Some(&declaration.module)
        {
            continue;
        }
        if declaration.role == Role::Enum && arguments == 0 {
            return Some(IdentityKind::Scalar);
        }
        if declaration.role == Role::Struct
            && declaration
                .required_struct()
                .is_ok_and(|schema| schema.generics.len() == arguments)
        {
            return Some(IdentityKind::Composite);
        }
    }
    None
}
// Canonical namespace checks need only declaration identities, not field layouts or execution.
fn identity_kind(library: &Library, name: &str) -> Option<IdentityKind> {
    if let Some(ty) = Ty::parse(name)
        && ty.source_name() == name
    {
        return Some(
            if ty.scalar_kind().is_some() || matches!(ty, Ty::Any | Ty::Void) {
                IdentityKind::Scalar
            } else {
                IdentityKind::Composite
            },
        );
    }
    if let Some((child, size)) = Ty::list_parts(name)
        && name
            == size.map_or_else(
                || format!("list<{child}>"),
                |size| format!("list<{child},{size}>"),
            )
        && identity_kind(library, child).is_some()
    {
        return Some(IdentityKind::Composite);
    }
    let (base, arguments) = hgl_source::application(name).unwrap_or((name, Vec::new()));
    let canonical =
        arguments.is_empty() && base == name || name == format!("{base}<{}>", arguments.join(","));
    if canonical {
        if base == "rolling"
            && (2..=3).contains(&arguments.len())
            && identity_kind(library, arguments[0]).is_some()
            && let Some(window) = hgl_source::Window::parse(arguments[1], arguments.get(2).copied())
            && name == format!("rolling<{},{}>", arguments[0], window.source_name())
        {
            return Some(IdentityKind::Composite);
        }
        let children = arguments
            .iter()
            .map(|arg| identity_kind(library, arg))
            .collect::<Option<Vec<_>>>();
        if let Some(children) = children {
            if matches!(
                (base, children.as_slice()),
                ("tuple", _)
                    | ("map", [_, _])
                    | ("set" | "ref" | "nullable", [_])
                    | ("atomic" | "delta", [IdentityKind::Composite])
            ) {
                return Some(IdentityKind::Composite);
            }
            if (!arguments.is_empty() || name == base)
                && let Some(kind) = nominal_identity(library, base, arguments.len())
            {
                return Some(kind);
            }
        }
    }
    library
        .native_contracts
        .values()
        .flat_map(|d| d.scalars.values())
        .any(|scalar| scalar.identity == name)
        .then_some(IdentityKind::Native)
}
/// Identify a data sidecar in an ordinary compiler source bundle.
pub fn sidecar(source: &str) -> bool {
    source.ends_with(".hgl-native")
}
/// Parse one descriptor using only the source lexer and inert metadata.
pub fn parse(source: &str, input: &str) -> Result<Descriptor, Issue> {
    let tokens = hgl_source::lex(input)?;
    let mut c = Cursor::new(&tokens);
    let mut descriptor = hgl_source::native_package::header(source, &mut c)?;
    while !c.at("") {
        if c.take("scalar") {
            let (name, scalar) = hgl_source::native_descriptor::scalar(&mut c)?;
            if c.take("marker") {
                descriptor
                    .markers
                    .insert(name.clone(), hgl_source::native_descriptor::symbol(&mut c)?);
            }
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
    hgl_source::native_package::validate_aliases(library.native_contracts.values())?;
    for declaration in library.declarations.iter() {
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
    let scalar = library
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
        })?;
    if identity_kind(library, &scalar.identity).is_some_and(|kind| kind != IdentityKind::Native) {
        return Err(format!(
            "{}::{}: native canonical identity conflicts with non-native HGL type {}",
            declaration.module, declaration.name, scalar.identity
        )
        .into());
    }
    Ok(scalar)
}
fn canonical(library: &Library, module: &str, name: &str) -> Result<String, Issue> {
    if let Some(declaration) = crate::struct_names::native_declaration(library, module, name)? {
        return Ok(scalar(library, declaration)?.identity.clone());
    }
    if let Some(ty) = Ty::parse(name)
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

/// Select and intern a native provider binding without invoking it.
pub fn bind(
    library: &Library,
    declaration: &Decl,
    signature: &Signature,
    (values, result): (Vec<Value>, Ty),
    (runtime, phase): (bool, &str),
    natives: &mut Vec<crate::ir::Native>,
) -> Result<Value, String> {
    let (module, name) = (declaration.module.as_str(), declaration.name.as_str());
    let args = values.iter().map(|v| v.ty.clone()).collect::<Vec<_>>();
    let (provider, method) =
        if let Some(contract) = function(library, declaration, signature).map_err(String::from)? {
            if !contract.phases.contains(phase) {
                return Err(format!("native helper is unavailable in {phase}"));
            }
            if runtime && result.owning_scalar() && contract.bound.is_none() {
                return Err("native owning evaluation result requires a pure target bound".into());
            }
            let descriptor = &library.native_contracts[module];
            let path = descriptor
                .image
                .as_ref()
                .ok_or("missing native helper executable image contract")?;
            (
                Some(hgl_source::native_contracts::Image {
                    provider: descriptor.provider.clone(),
                    fingerprint: descriptor.fingerprint.clone(),
                    path: path.clone(),
                    bounds: descriptor
                        .functions
                        .values()
                        .filter_map(|function| {
                            function
                                .bound
                                .as_ref()
                                .map(|bound| (function.symbol.clone(), bound.clone()))
                        })
                        .collect(),
                    scalars: descriptor
                        .scalars
                        .iter()
                        .filter_map(|(name, scalar)| {
                            descriptor
                                .markers
                                .get(name)
                                .map(|marker| (scalar.clone(), marker.clone()))
                        })
                        .collect(),
                }),
                contract.symbol.clone(),
            )
        } else {
            if !runtime {
                return Err("native value calls in composition are not yet supported".into());
            }
            legacy(library, declaration, signature)?;
            (
                None,
                format!(
                    "{}_{}",
                    name,
                    args.iter().map(Ty::name).collect::<Vec<_>>().join("_")
                ),
            )
        };
    let full_name = format!("{module}::{name}");
    let native = intern(
        crate::ir::Native {
            name: full_name,
            method,
            provider,
            throws: signature.throws,
            args,
            result: result.clone(),
        },
        natives,
    )?;
    let mut invocation = natives[native].invocation(values, result);
    invocation.cold_call = matches!(phase, "setup" | "cold");
    Ok(invocation)
}

fn legacy(library: &Library, declaration: &Decl, signature: &Signature) -> Result<(), String> {
    let (module, name) = (&declaration.module, &declaration.name);
    let selected = library
        .declarations
        .iter()
        .filter(|d| d.module == *module && d.name == *name && d.role == Role::Native)
        .filter_map(|d| d.signature().ok())
        .filter(|s| {
            s.generics.is_empty()
                && s.parameters.len() == signature.parameters.len()
                && s.parameters
                    .iter()
                    .zip(&signature.parameters)
                    .all(|(a, b)| a.name == b.name && a.ty == b.ty && a.constant == b.constant)
                && s.throws == signature.throws
                && s.result == signature.result
                && !s.body.is_empty()
        })
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(format!(
            "{module}::{name}: expected one selected native implementation"
        ));
    }
    if !selected[0]
        .body
        .iter()
        .filter(|token| !matches!(token.text.as_str(), "\n" | ""))
        .map(|token| token.text.as_str())
        .eq(["{", "}"])
    {
        return Err("unsupported native implementation".into());
    }
    Ok(())
}

/// Intern one checked signature while preserving legacy target name uniqueness.
pub fn intern(
    native: crate::ir::Native,
    natives: &mut Vec<crate::ir::Native>,
) -> Result<usize, String> {
    if let Some(id) = natives
        .iter()
        .position(|n| n.name == native.name && n.args == native.args)
    {
        return Ok(id);
    }
    if native.provider.is_none()
        && natives
            .iter()
            .any(|n| n.provider.is_none() && n.method == native.method)
    {
        return Err("native Rust binding name collision".into());
    }
    let id = natives.len();
    natives.push(native);
    Ok(id)
}

/// Form a canonical ordinary native scalar after exact declaration visibility checks.
pub fn resolve(library: &Library, declaration: &Decl, applied: bool) -> Result<Ty, Issue> {
    if applied {
        return Err("native scalar type arguments are not admitted".into());
    }
    Ok(Ty::Native(scalar(library, declaration)?.clone()))
}
