//! Ordinary value authority and lexical global-entry effect checking.
use hgl_rust_ir::{Kind, Node, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Whether the checked type admits ordinary owning retention.
pub fn ordinary(ty: &Ty) -> bool {
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
    if let Kind::Field(parent, _) = &value.kind {
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
    if !matches!(value.ty, Ty::Struct(..)) {
        return None;
    }
    if let Kind::BorrowedLocal(_, entry, writable) = value.kind {
        return Some((entry, writable));
    }
    if let Kind::Field(parent, _) = &value.kind {
        return provenance(parent);
    }
    None
}
/// Bind an owning value or preserve an explicitly borrowed initializer.
pub fn binding(id: usize, value: &Value, mutable: bool, annotated: bool) -> Result<Value, String> {
    let kind = if let Kind::GlobalGet(entry) = value.kind
        && matches!(value.ty, Ty::Struct(..))
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
            Statement::Return(value) => {
                helper_argument(value)?;
                expression(value, live)?;
            }
            Statement::Assign(target, value) => {
                if matches!(target.kind, Kind::Cache(_) | Kind::Output) {
                    helper_argument(value)?;
                }
                expression(value, live)?;
            }
            Statement::For(_, value, body) => {
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
            if matches!(value.ty, Ty::Struct(..)) {
                return Err("aggregate get requires a typed let or var binding".into());
            }
        }
        Kind::GlobalSet(entry, value) => {
            expression(value, live)?;
            conflict(*entry, true, live)?;
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
        Kind::Field(value, _)
        | Kind::ReplaySlot(value)
        | Kind::IsPresent(value)
        | Kind::Present(value)
        | Kind::Unary(_, value) => expression(value, live)?,
        Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
    Ok(())
}
