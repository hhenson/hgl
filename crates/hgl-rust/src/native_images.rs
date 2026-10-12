//! Reachable native scalar layouts and helper calls share one checked provider image union.
use crate::ir::Plan;
use hgl_rust_codegen::native_image::image_module;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn emit(plan: &Plan) -> Result<String, hgl_source::Issue> {
    let reached = hgl_semantics::native_images::scalars(plan)?;
    let helper_providers = plan
        .natives
        .iter()
        .filter_map(|native| native.provider.as_ref())
        .map(|image| image.provider.as_str())
        .collect::<BTreeSet<_>>();
    let images = plan.native_images.iter().chain(
        plan.natives
            .iter()
            .filter_map(|native| native.provider.as_ref()),
    );
    let mut providers = BTreeMap::new();
    let mut scalars = BTreeMap::new();
    for image in images {
        if helper_providers.contains(image.provider.as_str())
            || image
                .scalars
                .iter()
                .any(|(scalar, _)| reached.contains_key(&scalar.identity))
        {
            providers.insert(&image.provider, image);
        }
        for (scalar, marker) in &image.scalars {
            if reached.contains_key(&scalar.identity) {
                scalars
                    .entry(&scalar.identity)
                    .or_insert((scalar, marker, image));
            }
        }
    }
    let mut code = Vec::new();
    for image in providers.values() {
        code.push(format!(
            "#[path={:?}] mod {};\n",
            image.path,
            image_module(&image.provider)
        ));
    }
    for (scalar, marker, image) in scalars.values() {
        code.push(format!(
            "#[derive(Debug)] struct NativeDomain{};impl hgl_store::NativeKeyDomain for NativeDomain{} {{type Scalar={}::{marker};const MAPPING:(&'static str,&'static str)=({:?},{marker:?});fn slot()->&'static std::sync::OnceLock<usize>{{static SLOT:std::sync::OnceLock<usize>=std::sync::OnceLock::new();&SLOT}}}} type {}={}::{marker};",
            crate::native_values::domain(scalar),
            crate::native_values::domain(scalar),
            image_module(&image.provider),
            image.path,
            crate::native_values::marker(scalar),
            image_module(&image.provider)
        ));
    }
    code.push("fn validate_native_images()->Result<(),String>{".into());
    for image in providers.values() {
        let module = image_module(&image.provider);
        code.push(format!("if {module}::PROVIDER!={:?} || {module}::FINGERPRINT!={:?} {{return Err(\"native descriptor/provider identity or fingerprint mismatch\".into());}}",image.provider,image.fingerprint));
    }
    for (scalar, marker, image) in scalars.values() {
        let module = image_module(&image.provider);
        let caps = ["copy", "text", "equality", "hash", "order", "serialization"]
            .map(|c| scalar.capabilities.contains(c));
        code.push(format!("<{module}::{marker} as hgl_store::native_values::NativeType>::CONTRACT.validate({:?},{:?},{:?},{caps:?}).map_err(|e|e.message.clone())?;",image.provider,image.fingerprint,scalar.identity));
        code.push(format!("<NativeDomain{} as hgl_store::NativeKeyDomain>::prepare().map_err(|e|e.message.clone())?;",crate::native_values::domain(scalar)));
    }
    code.push("Ok(())}\n".into());
    Ok(code.concat())
}
