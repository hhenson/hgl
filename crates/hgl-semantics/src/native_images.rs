//! Checked native type reachability retains inert target mappings without helper calls.
use crate::harness_ir::Test;
use crate::ir::Plan;
use crate::library::Library;
use hgl_source::native_contracts::{Image, Scalar};
use hgl_source::{Issue, Ty};
use std::collections::{BTreeMap, BTreeSet};

fn collect(
    ty: &Ty,
    scalars: &mut BTreeMap<String, Scalar>,
    seen: &mut BTreeSet<String>,
    ordinary: &mut BTreeSet<String>,
) {
    if !matches!(ty, Ty::Native(_)) {
        ordinary.insert(ty.source_name());
    }
    match ty {
        Ty::Native(scalar) => {
            scalars.insert(scalar.identity.clone(), scalar.clone());
        }
        Ty::Struct(_, fields, _) => {
            for (_, child) in fields {
                collect(child, scalars, seen, ordinary);
            }
        }
        Ty::Tuple(children) => {
            for child in children {
                collect(child, scalars, seen, ordinary);
            }
        }
        Ty::Result(key, child) | Ty::Map(key, child) => {
            collect(key, scalars, seen, ordinary);
            collect(child, scalars, seen, ordinary);
        }
        Ty::Atomic(child)
        | Ty::Ref(child)
        | Ty::Nullable(child)
        | Ty::Set(child)
        | Ty::Delta(child)
        | Ty::List(child, _)
        | Ty::Rolling(child, _) => collect(child, scalars, seen, ordinary),
        Ty::Family(family) => {
            for (_, child) in family.members() {
                collect(child, scalars, seen, ordinary);
            }
        }
        Ty::Recursive(batch)
            if !batch.definitions().is_empty() && seen.insert(batch.identity().source_name()) =>
        {
            for definition in batch.definitions() {
                for (_, child) in definition.fields() {
                    collect(child, scalars, seen, ordinary);
                }
            }
        }
        Ty::Error
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Any
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => {}
    }
}

/// Native scalar contracts reached by checked graph storage and expressions.
pub fn scalars(plan: &Plan) -> Result<BTreeMap<String, Scalar>, Issue> {
    combined_scalars(std::slice::from_ref(plan), &[])
}
/// Native scalar contracts reached by ordinary lexical test expressions.
pub fn harness_scalars(tests: &[Test]) -> Result<BTreeMap<String, Scalar>, Issue> {
    combined_scalars(&[], tests)
}
/// Check the common canonical namespace before plans and lexical values share layouts.
pub fn combined_scalars(plans: &[Plan], tests: &[Test]) -> Result<BTreeMap<String, Scalar>, Issue> {
    let mut scalars = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut ordinary = BTreeSet::new();
    for plan in plans {
        for ty in &plan.ordinary_types {
            collect(ty, &mut scalars, &mut seen, &mut ordinary);
        }
        for value in &plan.wiring {
            crate::ir_visit::value(value, &mut |v| {
                collect(&v.ty, &mut scalars, &mut seen, &mut ordinary);
                true
            });
        }
        for node in &plan.nodes {
            for ty in std::iter::once(&node.result)
                .chain(node.inputs.iter().map(|(_, _, ty)| ty))
                .chain(node.globals.iter().map(|(_, ty)| ty))
            {
                collect(ty, &mut scalars, &mut seen, &mut ordinary);
            }
            for value in &node.configuration {
                crate::ir_visit::value(value, &mut |v| {
                    collect(&v.ty, &mut scalars, &mut seen, &mut ordinary);
                    true
                });
            }
            crate::ir_visit::hooks(node, &mut |v| {
                collect(&v.ty, &mut scalars, &mut seen, &mut ordinary);
                true
            });
        }
        for native in &plan.natives {
            for ty in std::iter::once(&native.result).chain(&native.args) {
                collect(ty, &mut scalars, &mut seen, &mut ordinary);
            }
        }
    }
    crate::ir_visit::harness(tests, &mut |v| {
        collect(&v.ty, &mut scalars, &mut seen, &mut ordinary);
        true
    });
    checked(scalars, &ordinary)
}

fn checked(
    scalars: BTreeMap<String, Scalar>,
    ordinary: &BTreeSet<String>,
) -> Result<BTreeMap<String, Scalar>, Issue> {
    if let Some(identity) = scalars.keys().find(|identity| ordinary.contains(*identity)) {
        return Err(format!(
            "native canonical identity conflicts with non-native HGL type {identity}"
        )
        .into());
    }
    Ok(scalars)
}

/// Select exact validated type mappings without opening a provider image.
pub fn select(
    library: &Library,
    scalars: &BTreeMap<String, Scalar>,
    selected: &[Image],
) -> Result<Vec<Image>, Issue> {
    let mut images = Vec::new();
    for scalar in scalars.values() {
        if let Some((image, marker)) = selected.iter().find_map(|image| {
            image
                .scalars
                .iter()
                .find(|(candidate, _)| candidate.identity == scalar.identity)
                .map(|(_, marker)| (image, marker))
        }) {
            images.push(Image {
                provider: image.provider.clone(),
                fingerprint: image.fingerprint.clone(),
                path: image.path.clone(),
                scalars: vec![(scalar.clone(), marker.clone())],
                bounds: BTreeMap::new(),
            });
            continue;
        }
        let (descriptor, name) = library
            .native_contracts
            .values()
            .find_map(|descriptor| {
                descriptor
                    .scalars
                    .iter()
                    .find(|(name, candidate)| {
                        candidate.identity == scalar.identity
                            && descriptor.image.is_some()
                            && descriptor.markers.contains_key(*name)
                    })
                    .map(|(name, _)| (descriptor, name))
            })
            .ok_or_else(|| {
                Issue::from(format!(
                    "missing native scalar executable image or target marker: {}",
                    scalar.identity
                ))
            })?;
        let path = descriptor
            .image
            .as_ref()
            .ok_or("missing native scalar executable image contract")?;
        let marker = descriptor
            .markers
            .get(name)
            .ok_or_else(|| Issue::from(format!("missing native scalar target marker: {name}")))?;
        images.push(Image {
            provider: descriptor.provider.clone(),
            fingerprint: descriptor.fingerprint.clone(),
            path: path.clone(),
            scalars: vec![(scalar.clone(), marker.clone())],
            bounds: BTreeMap::new(),
        });
    }
    Ok(images)
}
