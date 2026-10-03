//! Ordinary value authority and lexical global-entry effect checking.
use hgl_rust_ir::{Kind, Node, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;

pub use hgl_value_access::{
    binding, field, helper_argument, observed, ordinary, provenance, writable,
};
/// Validate hook-local entry lifetimes after configured keys have been unified.
pub fn validate(node: &Node) -> Result<(), String> {
    if let Some(body) = &node.generator {
        block(body, &mut BTreeMap::new())?;
    }
    block(&node.start, &mut BTreeMap::new())?;
    for (guard, body) in &node.handlers {
        if let Some(guard) = guard {
            expression(guard, &BTreeMap::new())?;
        }
        block(body, &mut BTreeMap::new())?;
    }
    block(&node.stop, &mut BTreeMap::new())
}
fn conflict(entry: usize, writable: bool, live: &BTreeMap<usize, bool>) -> Result<(), String> {
    if live.get(&entry).is_some_and(|old| writable || *old) {
        return Err("global_state: conflicting access overlaps a lexical aggregate borrow".into());
    }
    Ok(())
}
fn block(body: &[Statement], live: &mut BTreeMap<usize, bool>) -> Result<(), String> {
    for statement in body {
        match statement {
            Statement::Borrow(_, value, writable) => {
                if let Kind::GlobalGet(entry) = value.kind {
                    conflict(entry, *writable, live)?;
                    live.insert(entry, *writable);
                } else {
                    expression(value, live)?;
                }
            }
            Statement::Let(_, value) | Statement::Var(_, value) | Statement::Call(value) => {
                expression(value, live)?;
            }
            Statement::Return(value) | Statement::Yield(value) => {
                if !matches!(statement, Statement::Return(_)) || !matches!(value.ty, Ty::Delta(_)) {
                    helper_argument(value)?;
                }
                expression(value, live)?;
            }
            Statement::Assign(target, value) => {
                if matches!(target.kind, Kind::Cache(_))
                    || (matches!(target.kind, Kind::Output) && !matches!(value.ty, Ty::Delta(_)))
                {
                    helper_argument(value)?;
                }
                expression(value, live)?;
            }
            Statement::TimedYield(time, payload) => {
                expression(time, live)?;
                if !matches!(payload.ty, Ty::Delta(_)) {
                    helper_argument(payload)?;
                }
                expression(payload, live)?;
            }
            Statement::For(_, value, body) | Statement::While(value, body) => {
                expression(value, live)?;
                block(body, &mut live.clone())?;
            }
            Statement::If(value, yes, no) => {
                expression(value, live)?;
                block(yes, &mut live.clone())?;
                block(no, &mut live.clone())?;
            }
            Statement::Exit => {}
        }
    }
    Ok(())
}
fn expression(value: &Value, live: &BTreeMap<usize, bool>) -> Result<(), String> {
    match &value.kind {
        Kind::GlobalGet(entry) => {
            conflict(*entry, false, live)?;
            if matches!(value.ty, Ty::Struct(..) | Ty::List(..) | Ty::Delta(_)) {
                return Err("aggregate get requires a typed let or var binding".into());
            }
        }
        Kind::GlobalSet(entry, value) => {
            expression(value, live)?;
            conflict(*entry, true, live)?;
        }
        Kind::Delta(parts) => {
            for part in parts {
                if let hgl_rust_ir::DeltaEntry::Child(_, value) = part {
                    expression(value, live)?;
                }
            }
        }
        Kind::List(values) => {
            for value in values {
                expression(value, live)?;
            }
        }
        Kind::ValueCall(args, body) => {
            for value in args {
                helper_argument(value)?;
                expression(value, live)?;
            }
            block(body, &mut live.clone())?;
        }
        Kind::Index(parent, index) | Kind::Push(parent, index) => {
            expression(parent, live)?;
            expression(index, live)?;
        }
        Kind::Construct(fields) => {
            for (_, value) in fields {
                expression(value, live)?;
            }
        }
        Kind::Native(_, args) | Kind::Query(_, args) => {
            for value in args {
                helper_argument(value)?;
                expression(value, live)?;
            }
        }
        Kind::Binary(_, a, b) => {
            expression(a, live)?;
            expression(b, live)?;
        }
        Kind::Length(value)
        | Kind::Field(value, _)
        | Kind::IsPresent(value)
        | Kind::Present(value)
        | Kind::Unary(_, value) => expression(value, live)?,
        Kind::WiringFailure(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Configuration(_)
        | Kind::Cache(_)
        | Kind::ObservedLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
        Kind::GeneratorLocal(_) => {
            return Err("backend-only generator local in frontend checking".into());
        }
    }
    Ok(())
}

/// Check a constant ordinary list literal under its exact expected type.
pub fn list_literal(
    elements: &[Option<hgl_source::Expr>],
    expected: Option<&Ty>,
) -> Result<Value, String> {
    let context = if let Some(Ty::List(element, size)) = expected {
        Some((element.as_ref(), *size))
    } else {
        None
    };
    if elements.is_empty() && context.is_none() {
        return Err("empty list requires an expected concrete list type".into());
    }
    let mut values = Vec::new();
    for expr in elements {
        let expr = expr
            .as_ref()
            .ok_or("ordinary lists cannot contain absent elements")?;
        let value = if let hgl_source::Expr::Sequence(child) = expr {
            list_literal(child, context.map(|(element, _)| element))?
        } else {
            let fixed = expr
                .fixed()
                .ok_or("ordinary nonempty list literals require constant elements")?;
            Value::new(fixed.ty(), Kind::Literal(fixed))
        };
        values.push(value);
    }
    let (element, size) = context.ok_or("uncontextualized nonempty ordinary list literal inference is unsupported; expected concrete list type required")?;
    let element = element.clone();
    if size.is_some_and(|size| size != values.len()) {
        return Err("fixed list size mismatch".into());
    }
    if !ordinary(&element) || values.iter().any(|v| v.ty != element) {
        return Err("ordinary list element type mismatch".into());
    }
    Ok(Value::new(
        Ty::List(Box::new(element), size),
        Kind::List(values),
    ))
}
/// Check an ordinary indexed read without introducing copy or write authority.
pub fn indexed(parent: Value, index: Value) -> Result<Value, String> {
    if matches!(parent.kind, Kind::Input(..) | Kind::Wire(_) | Kind::Output) {
        return Err(
            "temporal child indexing is outside the admitted publication-delta profile".into(),
        );
    }
    let Ty::List(element, _) = &parent.ty else {
        return Err("indexing requires an ordinary list".into());
    };
    if index.ty != Ty::I64 {
        return Err("ordinary list index requires i64".into());
    }
    Ok(Value::new(
        *element.clone(),
        Kind::Index(Box::new(parent), Box::new(index)),
    ))
}
/// Check receiver-first list observation or growth.
pub fn list_operation(name: &str, args: &[Value]) -> Result<Value, String> {
    let receiver = args
        .first()
        .ok_or("ordinary list operation requires a receiver")?;
    if matches!(
        receiver.kind,
        Kind::Input(..) | Kind::Wire(_) | Kind::Output
    ) {
        return Err("ordinary list operation requires ordinary value access".into());
    }
    let Ty::List(element, size) = &receiver.ty else {
        return Err("ordinary list receiver required".into());
    };
    if name == "len" && args.len() == 1 {
        return Ok(Value::new(
            Ty::I64,
            Kind::Length(Box::new(receiver.clone())),
        ));
    }
    if name != "push" || args.len() != 2 {
        return Err("ordinary list operation argument mismatch".into());
    }
    if size.is_some() {
        return Err("push requires an unbounded ordinary list".into());
    }
    if !writable(receiver) {
        return Err("push requires writable ordinary list access".into());
    }
    if args[1].ty != **element {
        return Err("push ordinary list element type mismatch".into());
    }
    Ok(Value::new(
        Ty::Void,
        Kind::Push(Box::new(receiver.clone()), Box::new(args[1].clone())),
    ))
}

/// Prepare endpoint bindings and retained ordinary node configurations.
pub fn prepare_node(
    signature: &hgl_library::Signature,
    env: &mut BTreeMap<String, Value>,
    name: String,
    result: Ty,
) -> Result<Node, String> {
    let mut node = Node {
        name,
        inputs: Vec::new(),
        result,
        alarm: false,
        generator: None,
        start: Vec::new(),
        stop: Vec::new(),
        global_state: false,
        globals: Vec::new(),
        configuration: Vec::new(),
        caches: Vec::new(),
        handlers: Vec::new(),
    };

    for p in &signature.parameters {
        if p.constant {
            let value = env.get_mut(&p.name).ok_or("missing configuration")?;
            if matches!(value.ty, Ty::List(..) | Ty::Struct(..) | Ty::Delta(_)) {
                let id = node.configuration.len();
                node.configuration.push(value.clone());
                value.kind = Kind::Configuration(id);
            }
        } else {
            let value = env.get_mut(&p.name).ok_or("missing parameter")?;
            let Kind::Wire(wire) = value.kind else {
                return Err("temporal parameter requires a port".into());
            };
            if value.ty == Ty::Void {
                return Err("unsupported temporal input type".into());
            }
            let input = node.inputs.len();
            node.inputs.push((p.name.clone(), wire, value.ty.clone()));
            value.kind = Kind::Input(input, p.ty == "signal");
        }
    }
    Ok(node)
}

/// Resolve the admitted binary table after any numeric operand widening.
pub fn binary_type(op: &str, a: &Ty, b: &Ty) -> Result<Ty, String> {
    match (op, a, b) {
        ("+" | "-", Ty::DateTime, Ty::Duration) | ("+", Ty::Duration, Ty::DateTime) => {
            return Ok(Ty::DateTime);
        }
        ("-", Ty::DateTime, Ty::DateTime) | ("+" | "-", Ty::Duration, Ty::Duration) => {
            return Ok(Ty::Duration);
        }
        _ => {}
    }
    if *a != *b {
        return Err("binary operand type mismatch".into());
    }
    let ty = match op {
        "+" if matches!(*a, Ty::Str | Ty::I64 | Ty::F64) => a.clone(),
        "/" if matches!(*a, Ty::I64 | Ty::F64) => Ty::F64,
        "-" | "*" | "%" if matches!(*a, Ty::I64 | Ty::F64) => a.clone(),
        ">" | "<" | ">=" | "<="
            if matches!(
                *a,
                Ty::I64 | Ty::F64 | Ty::Date | Ty::DateTime | Ty::Time | Ty::Duration
            ) =>
        {
            Ty::Bool
        }
        "==" | "!="
            if !matches!(
                *a,
                Ty::Void
                    | Ty::Set(_)
                    | Ty::Struct(..)
                    | Ty::List(..)
                    | Ty::Delta(_)
                    | Ty::Map(..)
                    | Ty::Tuple(_)
            ) =>
        {
            Ty::Bool
        }
        "&&" | "||" if *a == Ty::Bool => Ty::Bool,
        _ => return Err(format!("unsupported binary operation {op}")),
    };
    Ok(ty)
}
