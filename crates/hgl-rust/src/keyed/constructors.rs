use hgl_semantics::ir::{DeltaEntry, Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
use std::fmt::Write as _;
type Locals = BTreeMap<usize, Value>;
/// Include checked constructor key paths and widths without evaluating payload expressions.
pub fn constructors(
    plan: &Plan,
    root: impl Fn(&Ty) -> Option<String>,
    width: impl Fn(&Ty, usize, usize) -> String,
    emit: impl Fn(&Value) -> String,
) -> String {
    let mut code = String::new();
    for (node, definition) in plan.nodes.iter().enumerate() {
        let mut visit = |value: &Value, locals: &Locals| {
            if let Kind::Query(op, args) = &value.kind
                && matches!(
                    op.as_str(),
                    "collection_insert" | "collection_update" | "collection_upsert"
                )
                && let [receiver, key, payload] = args.as_slice()
                && let Ok(ty) = receiver.ty.clone().delta()
                && let Some(domain) = root(&ty)
            {
                let publication = Value::new(
                    ty,
                    Kind::Delta(vec![DeltaEntry::Keyed(key.clone(), payload.clone())]),
                );
                code += &paths(&publication, locals, node, &domain, &width, &emit);
            }
            if matches!(value.kind, Kind::Delta(_))
                && let Some(domain) = root(&value.ty)
            {
                code += &paths(value, locals, node, &domain, &width, &emit);
            }
        };
        statements(&definition.start, &mut Locals::new(), &mut visit);
        statements(&definition.stop, &mut Locals::new(), &mut visit);
        if let Some(body) = &definition.generator {
            statements(body, &mut Locals::new(), &mut visit);
        }
        for (guard, body) in &definition.handlers {
            if let Some(guard) = guard {
                values(guard, &Locals::new(), &mut visit);
            }
            statements(body, &mut Locals::new(), &mut visit);
        }
    }
    code
}
fn resolve<'a>(value: &'a Value, locals: &'a Locals) -> &'a Value {
    if let Kind::Local(id) = value.kind
        && let Some(origin) = locals.get(&id)
    {
        return resolve(origin, locals);
    }
    value
}
fn key(
    value: &Value,
    locals: &Locals,
    node: usize,
    emit: &impl Fn(&Value) -> String,
) -> Option<String> {
    let value = resolve(value, locals);
    if let Kind::Configuration(id) = value.kind {
        return Some(format!("&configurations.node{node}_configuration{id}"));
    }
    let (_, Some(literal)) = hgl_semantics::static_values::StaticValues::default()
        .key(value.clone())
        .ok()?
    else {
        return None;
    };
    Some(format!("&({})", emit(&literal)))
}
fn paths(
    value: &Value,
    locals: &Locals,
    node: usize,
    domain: &str,
    width: &impl Fn(&Ty, usize, usize) -> String,
    emit: &impl Fn(&Value) -> String,
) -> String {
    let value = resolve(value, locals);
    let (Ty::Delta(origin), Kind::Delta(parts)) = (&value.ty, &value.kind) else {
        return String::new();
    };
    let mut counts = BTreeMap::new();
    let mut code = String::new();
    for part in parts {
        let (position, child) = match part {
            DeltaEntry::Add(k) | DeltaEntry::Remove(k) | DeltaEntry::Keyed(k, _) => {
                let removed = matches!(part, DeltaEntry::Remove(_));
                let field = if removed {
                    if matches!(origin.as_ref(), Ty::Map(..) | Ty::List(_, None)) {
                        2
                    } else {
                        1
                    }
                } else {
                    0
                };
                *counts.entry(field).or_insert(0) += 1;
                if matches!(part, DeltaEntry::Keyed(..)) {
                    *counts.entry(1).or_insert(0) += 1;
                }
                let Some(key) = key(k, locals, node, emit) else {
                    continue;
                };
                let ty = crate::layouts::global_type(&k.ty);
                let position = format!(
                    "<{ty} as hgl_store::Key>::prepare(&mut store.keys,{key}).map_err(|e|e.message)?;let key=<{ty} as hgl_store::Key>::id(&store.keys,{key}).map_err(|e|e.message)?;"
                );
                (
                    position,
                    if let DeltaEntry::Keyed(_, child) = part {
                        Some(child)
                    } else {
                        None
                    },
                )
            }
            DeltaEntry::Child(index, child) => {
                if matches!(origin.as_ref(), Ty::List(..)) {
                    *counts.entry(0).or_insert(0) += 1;
                    *counts.entry(1).or_insert(0) += 1;
                } else if let Ok(field) = usize::try_from(*index) {
                    counts.insert(field, 1);
                }
                (format!("let key={index}_i64;"), Some(child))
            }
        };
        let nested = child.map_or_else(String::new, |child| {
            paths(child, locals, node, "domain", width, emit)
        });
        write!(
            code,
            "{{{position}let domain=({domain}).children.entry(key).or_default();{nested}}}"
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    for (field, count) in counts {
        code += &width(&value.ty, field, count);
    }
    code
}
fn statements(body: &[Statement], locals: &mut Locals, visit: &mut impl FnMut(&Value, &Locals)) {
    for statement in body {
        match statement {
            Statement::Let(id, v) => {
                values(v, locals, visit);
                let origin = resolve(v, locals).clone();
                locals.insert(*id, origin);
            }
            Statement::Var(id, v) | Statement::Borrow(id, v, _) => {
                values(v, locals, visit);
                locals.remove(id);
            }
            Statement::Return(v) | Statement::Yield(v) | Statement::Call(v) => {
                values(v, locals, visit);
            }
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                values(a, locals, visit);
                values(b, locals, visit);
            }
            Statement::ForItems(_, _, _, v, body)
            | Statement::For(_, v, body)
            | Statement::While(v, body) => {
                values(v, locals, visit);
                statements(body, &mut locals.clone(), visit);
            }
            Statement::If(v, a, b) => {
                values(v, locals, visit);
                statements(a, &mut locals.clone(), visit);
                statements(b, &mut locals.clone(), visit);
            }
            Statement::Exit => {}
        }
    }
}
fn values(value: &Value, locals: &Locals, visit: &mut impl FnMut(&Value, &Locals)) {
    visit(value, locals);
    match &value.kind {
        Kind::Delta(parts) => {
            for part in parts {
                for v in part.operands() {
                    values(v, locals, visit);
                }
            }
        }
        Kind::List(items) | Kind::Query(_, items) | Kind::Native(_, items) => {
            for v in items {
                values(v, locals, visit);
            }
        }
        Kind::Construct(items) => {
            for (_, v) in items {
                values(v, locals, visit);
            }
        }
        Kind::ValueCall(args, body) => {
            let mut scope = Locals::new();
            for (id, arg) in args.iter().enumerate() {
                values(arg, locals, visit);
                scope.insert(id, resolve(arg, locals).clone());
            }
            statements(body, &mut scope, visit);
        }
        Kind::Field(v, _)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v)
        | Kind::GlobalSet(_, v) => values(v, locals, visit),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            values(a, locals, visit);
            values(b, locals, visit);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Captured(..)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
