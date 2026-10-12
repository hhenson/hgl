//! Pure cold native bounds and single-call borrowed result emission.
use hgl_semantics::ir::{Kind, Native, Plan, Value};
use hgl_source::{Literal, Ty};
fn bound(binding: &Native) -> Option<String> {
    let image = binding.provider.as_ref()?;
    Some(format!(
        "{}::{}",
        crate::native_image::image_module(&image.provider),
        image.bounds.get(&binding.method)?
    ))
}
fn expression(value: &Value) -> Option<String> {
    match &value.kind {
        Kind::Native(binding, args) => {
            bound(binding)?;
            let image = binding.provider.as_ref()?;
            let args = args
                .iter()
                .map(expression)
                .collect::<Option<Vec<_>>>()?
                .join(",");
            Some(format!(
                "{}::{}({args}){}",
                crate::native_image::image_module(&image.provider),
                binding.method,
                if binding.throws { "?" } else { "" }
            ))
        }
        Kind::Input(id, false) if value.ty == Ty::Str => Some(format!(
            "observation.scalar::<String>(self.input{id}.id())?.as_str()"
        )),
        Kind::Input(id, false) if matches!(value.ty, Ty::Native(_)) => Some(format!(
            "observation.atomic.values().native(observation.atomic.borrow(observation.bindings,self.input{id})?)"
        )),
        Kind::Configuration(id) if value.ty == Ty::Str => {
            Some(format!("self.configuration{id}.as_str()"))
        }
        Kind::Configuration(id) if matches!(value.ty, Ty::Native(_)) => {
            Some(format!("self.configuration{id}.view()"))
        }
        Kind::Local(id) | Kind::MutableLocal(id) if matches!(value.ty, Ty::Native(_)) => {
            Some(format!(
                "globals.values().native({{let view=local{id};if !view.1 {{return Err(hgl_types::NodeError::coded(\"required ordinary observation is unset\",\"value.unset_read\"));}}view.0}})"
            ))
        }
        Kind::GlobalGet(id) if matches!(value.ty, Ty::Native(_)) => Some(format!(
            "globals.values().native(globals.borrow(self.global{id})?)"
        )),
        Kind::Literal(Literal::Str(text)) => Some(format!("{text:?}")),
        Kind::Query(op, args) if op == "delta_value" => expression(args.first()?),
        Kind::Opaque(_)
        | Kind::IterationInput(_)
        | Kind::Captured(..)
        | Kind::Delta(_)
        | Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::List(_)
        | Kind::Index(..)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::ValueCall(..)
        | Kind::Configuration(_)
        | Kind::Construct(_)
        | Kind::Field(..)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => None,
    }
}
/// Only admitted borrowed result trees may select the prepared helper path.
pub fn supported(value: &Value) -> bool {
    matches!(value.kind, Kind::Native(..)) && expression(value).is_some()
}
/// Evaluate a provider helper once and copy its view into independent reserved storage.
pub fn publication(value: &Value) -> Option<String> {
    if !supported(value) {
        return None;
    }
    let method = if matches!(value.ty, Ty::Native(_)) {
        "native_result"
    } else if value.ty == Ty::Str {
        "text_result"
    } else {
        return None;
    };
    Some(format!(
        "_ctx.prepared().{method}(self._output,|observation,globals|{{Ok({})}})?;",
        expression(value)?
    ))
}
fn width(value: &Value, bounds: &impl Fn(&Ty) -> String) -> String {
    if let Kind::Native(binding, args) = &value.kind
        && let Some(callback) = bound(binding)
    {
        let arguments = args
            .iter()
            .map(|value| width(value, bounds))
            .collect::<Vec<_>>()
            .join(",");
        return format!("{callback}(&[{arguments}]).map_err(|e|e.message.clone())?");
    }
    if let Kind::Literal(Literal::Str(text)) = &value.kind {
        return format!("{}usize", text.len());
    }
    if value.ty.owning_scalar() {
        bounds(&value.ty)
    } else {
        "0usize".into()
    }
}
/// Compose pure cold ABI bounds along checked expression and graph dependencies.
pub fn limits(plan: &Plan, bounds: impl Fn(&Ty) -> String) -> String {
    let mut code = String::new();
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |value| {
            if supported(value) && value.ty.owning_scalar() {
                let destination = bounds(&value.ty);
                crate::append(
                    &mut code,
                    format_args!(
                        "{{let width={};{destination}={destination}.max(width);}}",
                        width(value, &bounds)
                    ),
                );
                return false;
            }
            true
        });
    }
    code
}

/// Execute a borrowed native result once before independently copying into a reserved local.
pub fn write(value: &Value, destination: &str) -> Option<String> {
    let method = if matches!(value.ty, Ty::Native(_)) {
        "native_result"
    } else if value.ty == Ty::Str {
        "text_result"
    } else {
        return None;
    };
    let expression = expression(value)?;
    Some(format!(
        "_ctx.with_observations(|observation,globals|globals.{method}(observation,{destination},|observation,globals|{{Ok({expression})}}))?;"
    ))
}
