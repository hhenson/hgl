//! Ordinary value authority and lexical global-entry effect checking.
use hgl_rust_ir::{Kind, Node, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Whether the checked type admits ordinary owning retention.
pub fn ordinary(ty: &Ty) -> bool {
    if let Ty::List(element, _) = ty {
        return ordinary(element);
    }
    matches!(
        ty,
        Ty::Bool
            | Ty::I64
            | Ty::F64
            | Ty::Str
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::Duration
            | Ty::Struct(..)
    )
}
/// Whether a checked place carries recursive write authority.
pub fn writable(value: &Value) -> bool {
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return writable(parent);
    }
    matches!(
        value.kind,
        Kind::MutableLocal(_) | Kind::BorrowedLocal(_, _, true)
    )
}
/// Resolve a declared ordinary field without changing its parent's authority.
pub fn field(parent: Value, name: &str) -> Result<Value, String> {
    let Ty::Struct(_, fields) = &parent.ty else {
        return Err("field access requires an ordinary struct or direct injected clock".into());
    };
    let (index, (_, ty)) = fields
        .iter()
        .enumerate()
        .find(|(_, (field, _))| field == name)
        .ok_or_else(|| format!("unknown struct field {name}"))?;
    Ok(Value::new(ty.clone(), Kind::Field(Box::new(parent), index)))
}
/// Return the entry and access mode carried by an aggregate view.
pub fn provenance(value: &Value) -> Option<(usize, bool)> {
    if !matches!(value.ty, Ty::Struct(..) | Ty::List(..)) {
        return None;
    }
    if let Kind::BorrowedLocal(_, entry, writable) = value.kind {
        return Some((entry, writable));
    }
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return provenance(parent);
    }
    None
}
/// Bind an owning value or preserve an explicitly borrowed initializer.
pub fn binding(id: usize, value: &Value, mutable: bool, annotated: bool) -> Result<Value, String> {
    let kind = if let Kind::GlobalGet(entry) = value.kind
        && matches!(value.ty, Ty::Struct(..) | Ty::List(..))
    {
        if !annotated {
            return Err("aggregate get requires a typed let or var binding".into());
        }
        Kind::BorrowedLocal(id, entry, mutable)
    } else if let Some((entry, source_mutable)) = provenance(value) {
        if source_mutable {
            return Err("cannot alias an exclusive writable global borrow".into());
        }
        if mutable {
            return Err("cannot upgrade a read-only global borrow to writable access".into());
        }
        Kind::BorrowedLocal(id, entry, false)
    } else if mutable {
        Kind::MutableLocal(id)
    } else {
        Kind::Local(id)
    };
    Ok(Value::new(value.ty.clone(), kind))
}
/// Reject passing a borrowed aggregate through an ordinary helper boundary.
pub fn helper_argument(value: &Value) -> Result<(), String> {
    if provenance(value).is_some() {
        return Err(
            "borrowed global aggregate cannot escape through an ordinary helper call".into(),
        );
    }
    Ok(())
}
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
                helper_argument(value)?;
                expression(value, live)?;
            }
            Statement::Assign(target, value) => {
                if matches!(target.kind, Kind::Cache(_) | Kind::Output) {
                    helper_argument(value)?;
                }
                expression(value, live)?;
            }
            Statement::TimedYield(time, payload) => {
                expression(time, live)?;
                helper_argument(payload)?;
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
            if matches!(value.ty, Ty::Struct(..) | Ty::List(..)) {
                return Err("aggregate get requires a typed let or var binding".into());
            }
        }
        Kind::GlobalSet(entry, value) => {
            expression(value, live)?;
            conflict(*entry, true, live)?;
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
            if matches!(value.ty, Ty::List(..) | Ty::Struct(..)) {
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
        "==" | "!=" if !matches!(*a, Ty::Void | Ty::Set(_) | Ty::Struct(..) | Ty::List(..)) => {
            Ty::Bool
        }
        "&&" | "||" if *a == Ty::Bool => Ty::Bool,
        _ => return Err(format!("unsupported binary operation {op}")),
    };
    Ok(ty)
}
