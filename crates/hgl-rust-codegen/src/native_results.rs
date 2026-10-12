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
    if let Some(expression) = borrowed_expression(value) {
        return Some(expression);
    }
    match &value.kind {
        Kind::Native(binding, args) => {
            if value.ty.owning_scalar() {
                bound(binding)?;
            }
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
        | Kind::Void => primitive_expression(value),
    }
}
fn primitive_expression(value: &Value) -> Option<String> {
    if value.ty.scalar_kind().is_none() || value.ty.owning_scalar() {
        return None;
    }
    if let Kind::Input(id, false) = value.kind {
        return Some(format!(
            "*observation.scalar::<{}>(self.input{id}.id())?",
            crate::layouts::rust_type(&value.ty)
        ));
    }
    if let Kind::Configuration(id) = value.kind {
        return Some(format!("self.configuration{id}"));
    }
    if let Kind::Local(id) | Kind::MutableLocal(id) = value.kind {
        return Some(format!("local{id}"));
    }
    if let Kind::GlobalGet(id) = value.kind {
        return Some(format!("globals.get(self.global{id})?"));
    }
    if let Kind::Literal(literal) = &value.kind {
        return Some(crate::scalars::literal(literal));
    }
    if let Kind::Binary(op, left, right) = &value.kind {
        if matches!(left.ty, Ty::Native(_) | Ty::Any) {
            return None;
        }
        return Some(crate::scalars::binary(
            &value.ty,
            op,
            &expression(left)?,
            &expression(right)?,
        ));
    }
    if let Kind::Unary(op, operand) = &value.kind {
        return Some(crate::scalars::unary(
            &operand.ty,
            op,
            &expression(operand)?,
        ));
    }
    scalar_query(value).or_else(|| primitive_projection(value))
}
fn primitive_projection(value: &Value) -> Option<String> {
    let (columns, slot) = borrowed_projection(value)?;
    Some(format!("({slot}).read({columns})?"))
}
fn borrowed_expression(value: &Value) -> Option<String> {
    if !matches!(
        value.kind,
        Kind::Field(..) | Kind::Index(..) | Kind::BorrowedLocal(..)
    ) {
        return None;
    }
    let (columns, slot) = borrowed_projection(value)?;
    if matches!(value.ty, Ty::Native(_)) {
        return Some(format!("({columns}).native({slot})"));
    }
    if value.ty == Ty::Str {
        return Some(format!(
            "({columns}).scalar::<String>(({slot}).fields()).as_str()"
        ));
    }
    None
}
fn borrowed_projection(value: &Value) -> Option<(&'static str, String)> {
    if let Kind::Configuration(id) = value.kind {
        return Some((
            "&self.configuration_columns",
            format!("self.configuration_slot{id}"),
        ));
    }
    if let Kind::GlobalGet(id) = value.kind {
        return Some((
            "globals.values()",
            format!("globals.borrow(self.global{id})?"),
        ));
    }
    if let Kind::BorrowedLocal(id, _, _) = value.kind {
        return Some(("globals.values()", format!("local{id}")));
    }
    if let Kind::Field(parent, index) = &value.kind {
        if matches!(parent.ty, Ty::Family(_)) {
            return None;
        }
        let (columns, parent) = borrowed_projection(parent)?;
        return Some((columns, format!("({parent}).fields().{index}")));
    }
    if let Kind::Index(parent, index) = &value.kind {
        let (columns, parent) = borrowed_projection(parent)?;
        return Some((
            columns,
            format!(
                "{{let parent={parent};let index={};hgl_store::list::global_index({columns},parent,index)?}}",
                expression(index)?
            ),
        ));
    }
    None
}
fn scalar_query(value: &Value) -> Option<String> {
    let Kind::Query(op, args) = &value.kind else {
        return None;
    };
    if !matches!(
        op.as_str(),
        "valid" | "all_valid" | "last_modified" | "modified"
    ) {
        return None;
    }
    let args = args
        .iter()
        .map(|value| {
            let Kind::Input(id, _) = value.kind else {
                return None;
            };
            let input = if matches!(value.ty, Ty::Ref(_)) {
                format!("self.input{id}")
            } else {
                format!("self.input{id}.id()")
            };
            Some(format!(
                "observation.bindings.{op}({input}{})",
                if op == "modified" { ",now" } else { "" }
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(format!(
        "({})",
        args.join(if op == "modified" { " || " } else { " && " })
    ))
}
/// Only admitted borrowed result trees may select the prepared helper path.
pub fn supported(value: &Value) -> bool {
    matches!(value.kind, Kind::Native(..)) && expression(value).is_some()
}
/// Reject missing runtime target transport before a checked plan can execute.
pub fn validate(plan: &Plan) -> Result<(), String> {
    let mut unsupported = None;
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |value| {
            if value.cold_call {
                return false;
            }
            if let Kind::Native(binding, _) = &value.kind
                && binding.provider.is_some()
                && value.ty.owning_scalar()
                && !value.snapshot
                && !supported(value)
            {
                unsupported.get_or_insert_with(|| format!("Rust native target does not support prepared owning result transport for {}", binding.name));
            }
            true
        });
    }
    unsupported.map_or(Ok(()), Err)
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
        "{{let now=_ctx.evaluation_time();_ctx.prepared().{method}(self._output,|observation,globals|{{Ok({})}})?;}}",
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
    } else if value.ty == Ty::Str && supported(value) {
        "text_result"
    } else {
        return None;
    };
    let expression = expression(value)?;
    Some(format!(
        "{{let now=_ctx.evaluation_time();_ctx.with_observations(|observation,globals|globals.{method}(observation,{destination},|observation,globals|{{Ok({expression})}}))?;}}"
    ))
}
