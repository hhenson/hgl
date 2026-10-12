//! Standalone cold recipes produce data-only private configuration carriers.
use crate::{Node, Plan};
use hgl_source::Ty;
use std::fmt::Write as _;
fn header(plan: &Plan, ty: &Ty) -> String {
    let mut images = plan
        .native_images
        .iter()
        .chain(plan.natives.iter().filter_map(|n| n.provider.as_ref()));
    let Ty::Native(scalar) = ty else {
        let providers = images
            .map(|p| (&p.provider, &p.fingerprint))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        return format!("HGL-COLD-2:{:?}", (ty.source_name(), providers));
    };
    let image = images
        .find(|p| p.scalars.iter().any(|(s, _)| s.identity == scalar.identity))
        .unwrap_or_else(|| unreachable!("checked native image"));
    format!(
        "HGL-COLD-1:{:?}",
        (&scalar.identity, &image.provider, &image.fingerprint)
    )
}
fn active(plan: &Plan, node: &Node) -> bool {
    !plan.wiring.is_empty() && !node.configuration.is_empty()
}
pub(crate) fn schema(plan: &Plan, node: &Node) -> String {
    if !active(plan, node) {
        return String::new();
    }
    let entries = node
        .configuration
        .iter()
        .enumerate()
        .map(|(id, v)| {
            let ty = if v.ty.scalar_kind().is_none() {
                "Bytes".into()
            } else {
                crate::values::scalar_type(&v.ty)
            };
            format!("(\"\\0cold:{id}\",hgl_types::ScalarType::{ty}),")
        })
        .collect::<Vec<_>>()
        .concat();
    format!("scalars:vec![{entries}],")
}
pub(crate) fn build(plan: &Plan, node: &Node) -> Option<String> {
    if !active(plan, node) {
        return None;
    }
    let args = node.configuration.iter().enumerate().map(|(id, v)| {
        let name = format!("\0cold:{id}");
        if let Ty::Native(scalar) = &v.ty {
            let marker = crate::native_values::marker(scalar);
            let header = header(plan, &v.ty);
            format!("{{let bytes=ports.scalar::<Vec<u8>>({name:?})?;let payload=bytes.strip_prefix({header:?}.as_bytes()).ok_or_else(||hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:\"native cold carrier identity mismatch\".into()}})?;hgl_store::native_values::NativeView::<{marker}>::from_provider(payload).retain().map_err(|e|hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:format!(\"{{:?}}: {{}}\",e.code,e.message)}})?}}",node.name,node.name)
        } else if v.ty.scalar_kind().is_some() { format!("ports.scalar::<{}>({name:?})?",crate::values::owned_type(&v.ty)) }
        else {
            let header=header(plan,&v.ty);
            let bytes=format!("ports.scalar::<Vec<u8>>({name:?})?");
            let decoded=crate::cold_transport::decode(&v.ty,"payload","ports.registry()");
            format!("{{let bytes={bytes};let payload=bytes.strip_prefix({header:?}.as_bytes()).ok_or_else(||hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:\"cold carrier identity mismatch\".into()}})?;(||->hgl_types::NodeResult<_>{{Ok({decoded})}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:format!(\"{{:?}}: {{}}\",e.code,e.message)}})?}}",node.name,node.name)
        }
    }).collect::<Vec<_>>().join(",");
    Some(format!("Self::build_prepared(ports,{args})"))
}
pub(crate) fn arguments(plan: &Plan, node: &Node, index: usize) -> String {
    if !active(plan, node) {
        return "&[]".into();
    }
    let args = node.configuration.iter().enumerate().map(|(id, v)| {
        let value = format!("configurations.node{index}_configuration{id}");
        let encoded = if matches!(v.ty, Ty::Native(_)) {
            format!("{{let mut bytes={:?}.as_bytes().to_vec();bytes.extend({value}.into_owned());hgl_types::ScalarValue::Bytes(bytes)}}", header(plan,&v.ty))
        } else if v.ty.scalar_kind().is_some() { format!("hgl_store::Scalar::into_value({value})") }
        else {
            let encoded=crate::cold_transport::encode(&v.ty,&value);
            format!("{{let payload=(||->hgl_types::NodeResult<_>{{Ok({encoded})}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:format!(\"{{:?}}: {{}}\",e.code,e.message)}})?;let mut bytes={:?}.as_bytes().to_vec();bytes.extend(payload);hgl_types::ScalarValue::Bytes(bytes)}}",node.name,header(plan,&v.ty))
        };
        format!("({:?},{encoded})",format!("\0cold:{id}"))
    }).collect::<Vec<_>>().join(",");
    format!("&[{args}]")
}
pub(crate) fn execute(plan: &Plan) -> String {
    if plan.wiring.is_empty() {
        return String::new();
    }
    "let configurations=cold_configurations().map_err(|what|hgl_describe::BuildError::InvalidNodeType{node:\"hgl.program\",what})?;".into()
}
pub(crate) fn recipes(plan: &Plan) -> String {
    if plan.wiring.is_empty() {
        return String::new();
    }
    let mut code = crate::cold_transport::support();
    code.push_str(
        "fn cold_configurations()->Result<Configurations,String>{(||->hgl_types::NodeResult<_>{",
    );
    for (id, value) in plan.wiring.iter().enumerate() {
        write!(
            code,
            "let prepared{id}={};",
            crate::values::condition_code(plan, value)
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code.push_str("Ok(Configurations {");
    for (node, n) in plan.nodes.iter().enumerate() {
        for (id, value) in n.configuration.iter().enumerate() {
            write!(
                code,
                "node{node}_configuration{id}:{},",
                crate::values::condition_code(plan, value)
            )
            .unwrap_or_else(|_| unreachable!("String formatting"));
        }
    }
    code.push_str("})})().map_err(|e|format!(\"{:?}: {}\",e.code,e.message))}");
    code
}
