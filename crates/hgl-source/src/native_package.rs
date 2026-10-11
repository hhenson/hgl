//! Inert complete package contracts and code locations; no provider code is read.
use crate::{Cursor, Issue};
use hgl_value_model::native_contracts::{Function, Scalar};
use std::collections::BTreeMap;
/// A versioned package descriptor, separate from its implementation image.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// Canonical source module supplied by this package.
    pub module: String,
    /// Selected provider identity retained by consumers.
    pub provider: String,
    /// Semantic interface fingerprint checked again before execution.
    pub fingerprint: String,
    /// Optional target image location; checking never reads or loads its code.
    pub image: Option<String>,
    /// Native declaration bindings in the ordinary module namespace.
    pub scalars: BTreeMap<String, Scalar>,
    /// Explicit target markers for typed executable scalar bindings.
    pub markers: BTreeMap<String, String>,
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
/// Resolve a declared target location using data only, without accessing its contents.
pub fn image(source: &str, c: &mut Cursor<'_>) -> Result<Option<String>, Issue> {
    if c.take("image") {
        let path = crate::native_descriptor::text(c)?;
        if std::path::Path::new(&path)
            .extension()
            .is_none_or(|extension| extension != "rs")
        {
            return Err("native provider image requires a Rust source package".into());
        }
        let source = std::path::absolute(source).map_err(|e| Issue::from(e.to_string()))?;
        Ok(Some(
            source.with_file_name(path).to_string_lossy().into_owned(),
        ))
    } else {
        Ok(None)
    }
}

/// Parse a package header before semantic declaration and canonical binding checks.
pub fn header(source: &str, c: &mut Cursor<'_>) -> Result<Descriptor, Issue> {
    crate::native_descriptor::version(c)?;
    c.need("module")?;
    let mut module = c.name()?;
    while c.take(".") {
        module.push('.');
        module.push_str(&c.name()?);
    }
    c.lines();
    c.need("provider")?;
    let provider = crate::native_descriptor::text(c)?;
    c.need("fingerprint")?;
    let fingerprint = crate::native_descriptor::text(c)?;
    let image = image(source, c)?;
    c.lines();
    Ok(Descriptor {
        module,
        provider,
        fingerprint,
        image,
        scalars: BTreeMap::new(),
        markers: BTreeMap::new(),
        functions: BTreeMap::new(),
    })
}

/// Require repeated canonical identities to advertise exactly one capability contract.
pub fn validate_aliases<'a>(
    descriptors: impl Iterator<Item = &'a Descriptor>,
) -> Result<(), Issue> {
    let mut identities = BTreeMap::new();
    for descriptor in descriptors {
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
    Ok(())
}
