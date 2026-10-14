//! Private canonical constructor ownership metadata; admission and evaluation phases are unchanged.
use crate::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
fn references(value: &mut Value, owners: &BTreeMap<usize, usize>) -> bool {
    if let Kind::Local(id) | Kind::MutableLocal(id) = value.kind
        && let Some(entry) = owners.get(&id)
    {
        value.kind = Kind::BorrowedLocal(id, *entry, matches!(value.kind, Kind::MutableLocal(_)));
        value.snapshot = false;
    }
    if let Kind::ValueCall(args, _) = &mut value.kind {
        for argument in args {
            crate::ir_visit::value_mut(argument, &mut |v| references(v, owners));
        }
        return false;
    }
    !matches!(value.kind, Kind::Captured(..))
}
/// Owning diagnostics can acquire failure text and origin metadata after startup.
pub fn failure_data_type(ty: &Ty) -> bool {
    match ty {
        Ty::Any | Ty::Error | Ty::Result(..) => true,
        Ty::Ref(child)
        | Ty::Atomic(child)
        | Ty::Nullable(child)
        | Ty::List(child, _)
        | Ty::Set(child)
        | Ty::Rolling(child, _)
        | Ty::Delta(child) => failure_data_type(child),
        Ty::Map(key, child) => failure_data_type(key) || failure_data_type(child),
        Ty::Struct(_, fields, _) => fields.iter().any(|(_, ty)| failure_data_type(ty)),
        Ty::Tuple(children) => children.iter().any(failure_data_type),
        Ty::Family(family) => family.members().iter().any(|(_, ty)| failure_data_type(ty)),
        Ty::Recursive(batch) => batch.definitions().iter().any(|definition| {
            definition
                .fields()
                .iter()
                .any(|(_, ty)| failure_data_type(ty))
        }),
        Ty::Enum(_)
        | Ty::Native(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => false,
    }
}
/// Nodes downstream of captured diagnostics lack a cold bound on failure text and provenance.
/// This selects ordinary owning transport without changing checked source admission.
pub fn unbounded_failure_data(plan: &Plan) -> std::collections::BTreeSet<usize> {
    let mut unknown = plan
        .captures
        .iter()
        .map(|capture| capture.owner)
        .collect::<std::collections::BTreeSet<_>>();
    for (id, node) in plan.nodes.iter().enumerate() {
        crate::ir_visit::hooks(node, &mut |value| {
            if matches!(value.kind, Kind::Native(..))
                && matches!(value.ty, Ty::Result(..) | Ty::Error)
            {
                unknown.insert(id);
            }
            true
        });
    }
    loop {
        let before = unknown.len();
        for (id, node) in plan.nodes.iter().enumerate() {
            if node
                .inputs
                .iter()
                .any(|(_, source, _)| unknown.contains(source))
            {
                unknown.insert(id);
            }
        }
        if unknown.len() == before {
            return unknown;
        }
    }
}
/// Retain existing admitted native constructor locals without evaluating any operand early.
pub fn prepare(
    plan: &Plan,
    owning: impl Fn(&Ty) -> bool,
    convert: impl Fn(&Value) -> Option<Value>,
    tuple_staging: impl Fn(&Value, &Ty, &BTreeMap<usize, Value>) -> bool,
) -> Plan {
    let unknown = unbounded_failure_data(plan);
    let mut plan = plan.clone();
    for (node_id, node) in plan.nodes.iter_mut().enumerate() {
        let growing = crate::constructor_growth::locals(node);
        let mut owners = BTreeMap::new();
        let mut origins = BTreeMap::<usize, Value>::new();
        let mut locals = BTreeMap::new();
        for body in std::iter::once(&mut node.start)
            .chain(std::iter::once(&mut node.stop))
            .chain(node.handlers.iter_mut().map(|(_, body)| body))
            .chain(node.generator.iter_mut())
        {
            crate::ir_visit::nodes_mut(body, &mut |s| {
                if let Statement::Let(id, value) | Statement::Var(id, value) = s {
                    locals.insert(*id, value.clone());
                }
                if let Statement::Assign(target, value) = s
                    && matches!(target.kind,Kind::MutableLocal(id) if owners.contains_key(&id))
                {
                    crate::value_constant::retain_closed(value, &mut node.configuration, &owning);
                }
                if let Statement::Let(id, value) | Statement::Var(id, value) = s
                    && canonical_type(&value.ty)
                    && !(unknown.contains(&node_id) && failure_data_type(&value.ty))
                    && (outcome(&value.ty)
                        || matches!(value.kind, Kind::Construct(_))
                        || matches!(&value.kind, Kind::Unary(op, _) if op=="family")
                        || matches!(value.kind,Kind::Local(id)|Kind::MutableLocal(id) if origins.contains_key(&id)))
                    && !growing.contains(id)
                    && owning(&value.ty)
                {
                    let origin =
                        origin(value, &origins, &mut node.configuration, &owning, &convert);
                    origins.insert(*id, origin.clone());
                    let key = format!("\0constructor:{node_id}:{id}");
                    let entry = node.globals.len();
                    node.globals.push((key.clone(), value.ty.clone()));
                    owners.insert(*id, entry);
                    *value = Value::new(
                        value.ty.clone(),
                        Kind::Query(key, vec![value.clone(), origin]),
                    );
                }
            });
        }
        if !owners.is_empty() {
            node.global_state = true;
        }
        crate::ir_visit::hooks_mut(node, &mut |v| references(v, &owners));
        let result = node.result.clone();
        for (_, body) in &mut node.handlers {
            crate::ir_visit::statements_mut(body, &mut |v| {
                if matches!(&v.kind, Kind::Query(op, _) if op.starts_with("\0constructor:")) {
                    return false;
                }
                if matches!(v.kind, Kind::List(_)) && v.closed() {
                    return false;
                }
                if matches!(v.kind, Kind::Construct(_))
                    && !(unknown.contains(&node_id) && failure_data_type(&v.ty))
                    && (matches!(
                        v.ty,
                        Ty::Struct(..) | Ty::Recursive(_) | Ty::Result(..) | Ty::Error
                    ) || (matches!(v.ty, Ty::Tuple(_)) && tuple_staging(v, &result, &locals)))
                    && owning(&v.ty)
                {
                    let key = format!("\0constructor:{node_id}:expression:{}", node.globals.len());
                    node.globals.push((key.clone(), v.ty.clone()));
                    crate::value_constant::retain_closed(v, &mut node.configuration, &owning);
                    *v = Value::new(v.ty.clone(), Kind::Query(key, vec![v.clone()]));
                    return false;
                }
                !matches!(v.kind, Kind::Captured(..) | Kind::ValueCall(..))
            });
        }
        node.global_state |= !node.globals.is_empty();
    }
    plan
}

fn origin(
    value: &mut Value,
    origins: &BTreeMap<usize, Value>,
    configuration: &mut Vec<Value>,
    owning: &impl Fn(&Ty) -> bool,
    convert: &impl Fn(&Value) -> Option<Value>,
) -> Value {
    if let Kind::Local(source) | Kind::MutableLocal(source) = value.kind {
        return origins[&source].clone();
    }
    *value = convert(value).unwrap_or_else(|| value.clone());
    if matches!(value.ty, Ty::Family(_)) {
        if let Kind::Construct(fields) = &mut value.kind {
            for (_, child) in fields {
                crate::value_constant::retain_closed(child, configuration, owning);
            }
        }
    } else {
        crate::value_constant::retain_closed(value, configuration, owning);
    }
    value.clone()
}

fn outcome(ty: &Ty) -> bool {
    matches!(ty, Ty::Result(..) | Ty::Error)
}

fn canonical_type(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Struct(..) | Ty::Recursive(_) | Ty::Family(_) | Ty::Result(..) | Ty::Error
    )
}
