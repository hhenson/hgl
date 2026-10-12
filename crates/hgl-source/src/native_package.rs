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
        let resolved = source.with_file_name(path);
        let mut path = std::path::PathBuf::new();
        for component in resolved.components() {
            if component == std::path::Component::ParentDir {
                if path.file_name().is_some() {
                    path.pop();
                }
            } else if component != std::path::Component::CurDir {
                path.push(component);
            }
        }
        Ok(Some(path.to_string_lossy().into_owned()))
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

/// Require one capability contract and compatible selected target mappings per identity.
pub fn validate_aliases<'a>(
    descriptors: impl Iterator<Item = &'a Descriptor>,
) -> Result<(), Issue> {
    let mut identities = BTreeMap::new();
    let mut mappings = BTreeMap::new();
    let mut marker_identities = BTreeMap::new();
    let mut symbol_bounds = BTreeMap::new();
    let mut providers = BTreeMap::new();
    for descriptor in descriptors {
        if let Some(image) = &descriptor.image
            && providers
                .insert(&descriptor.provider, (&descriptor.fingerprint, image))
                .is_some_and(|old| old != (&descriptor.fingerprint, image))
        {
            return Err("native provider image has conflicting paths or fingerprints".into());
        }
        for (name, scalar) in &descriptor.scalars {
            if identities
                .insert(&scalar.identity, &scalar.capabilities)
                .is_some_and(|old| old != &scalar.capabilities)
            {
                return Err(
                    "aliases of one canonical native scalar require the same capabilities".into(),
                );
            }
            if let Some(image) = &descriptor.image
                && let Some(marker) = descriptor.markers.get(name)
            {
                let mapping = (&descriptor.provider, &descriptor.fingerprint, image, marker);
                if mappings
                    .insert(&scalar.identity, mapping)
                    .is_some_and(|old| old != mapping)
                {
                    return Err(format!(
                        "canonical native scalar {} has incompatible selected Rust target mappings",
                        scalar.identity
                    )
                    .into());
                }
                if marker_identities
                    .insert(mapping, &scalar.identity)
                    .is_some_and(|old| old != &scalar.identity)
                {
                    return Err(format!(
                        "Rust target marker maps to distinct canonical native scalars: {marker}"
                    )
                    .into());
                }
            }
        }
        for function in descriptor.functions.values() {
            let symbol = (
                &descriptor.provider,
                &descriptor.fingerprint,
                &descriptor.image,
                &function.symbol,
            );
            if symbol_bounds
                .insert(symbol, &function.bound)
                .is_some_and(|old| old != &function.bound)
            {
                return Err(format!(
                    "conflicting native helper symbol bounds: {}",
                    function.symbol
                )
                .into());
            }
        }
    }
    Ok(())
}
