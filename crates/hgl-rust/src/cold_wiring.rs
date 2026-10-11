//! Standalone cold recipes produce data-only private configuration carriers.
use crate::{Kind, Node, Plan};
use hgl_source::Ty;
use std::fmt::Write as _;
fn carrier(ty: &Ty) -> bool {
    ty.scalar_kind().is_some() || matches!(ty, Ty::Native(_))
}
fn header(plan: &Plan, ty: &Ty) -> String {
    let Ty::Native(scalar) = ty else {
        unreachable!("native carrier")
    };
    let image = plan
        .natives
        .iter()
        .filter_map(|n| n.provider.as_ref())
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
        .filter(|(_, v)| carrier(&v.ty))
        .map(|(id, v)| {
            let ty = if matches!(v.ty, Ty::Native(_)) {
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
        } else if carrier(&v.ty) { format!("ports.scalar::<{}>({name:?})?",crate::values::owned_type(&v.ty)) }
        else { format!("(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType{{node:{:?},what:format!(\"{{:?}}: {{}}\",e.code,e.message)}})?",crate::values::condition_code(plan,v),node.name) }
    }).collect::<Vec<_>>().join(",");
    Some(format!("Self::build_prepared(ports,{args})"))
}
pub(crate) fn arguments(plan: &Plan, node: &Node, index: usize) -> String {
    if !active(plan, node) {
        return "&[]".into();
    }
    let args = node.configuration.iter().enumerate().filter(|(_,v)|carrier(&v.ty)).map(|(id, v)| {
        let value = format!("configurations.node{index}_configuration{id}");
        let encoded = if matches!(v.ty, Ty::Native(_)) {
            format!("{{let mut bytes={:?}.as_bytes().to_vec();bytes.extend({value}.into_owned());hgl_types::ScalarValue::Bytes(bytes)}}", header(plan,&v.ty))
        } else { format!("hgl_store::Scalar::into_value({value})") };
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
    let mut code = crate::preparation::native_context(plan);
    code.push_str("fn cold_configurations()->Result<Configurations,String>{let mut context=NativeContext(hgl_source::time_context::RunContext::from_bundled()?);let mut evaluator=hgl_semantics::value_eval::Evaluator::default();let mut values=Vec::new();");
    for (id, value) in plan.wiring.iter().enumerate() {
        let mut recipe = value.clone();
        // Recipe operands capture their wiring versions; helper-local IDs have a separate scope.
        hgl_semantics::ir_visit::value_mut(&mut recipe, &mut |v| {
            if let Kind::Prepared(id) = v.kind {
                v.kind = Kind::Local(id);
            }
            true
        });
        write!(
            code,
            "let value=evaluator.value_in_context(&{},&mut context).map_err(|e|format!(\"{{e:?}}\"))?;evaluator.bind_checked({id},value.clone(),false);values.push(value);",
            crate::checked_data::value(&recipe)
        ).unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code.push_str("prepare_configurations(&values)}");
    code
}
