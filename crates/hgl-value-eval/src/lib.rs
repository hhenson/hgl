//! Direct ordinary evaluation of checked IR without runtime capabilities.
use hgl_rust_ir::{Kind, Statement, Value};
use hgl_source::{Literal, Ty};
use std::fmt;
mod operators;

/// Failure during ordinary evaluation, separated from an unsupported operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// An admitted operation failed when evaluated.
    Operation(String),
    /// The checked IR cannot be executed by this ordinary evaluator.
    Unsupported(String),
}
impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(message) | Self::Unsupported(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for EvalError {}
fn unsupported(message: &str) -> EvalError {
    EvalError::Unsupported(message.into())
}
fn bounds() -> EvalError {
    EvalError::Operation("ordinary list index out of bounds".into())
}
fn length(size: usize) -> Result<i64, EvalError> {
    i64::try_from(size)
        .map_err(|error| EvalError::Operation(format!("ordinary list length exceeds i64: {error}")))
}
fn index(value: &Value) -> Result<usize, EvalError> {
    let Kind::Literal(Literal::Int(value)) = value.kind else {
        return Err(unsupported("ordinary list index must be i64"));
    };
    usize::try_from(value).map_err(|_out_of_range| bounds())
}

/// Whether an expression is already a closed ordinary constant.
pub fn constant(value: &Value) -> bool {
    match &value.kind {
        Kind::Literal(_) | Kind::Void => true,
        Kind::List(items) => items.iter().all(constant),
        Kind::Construct(fields) => fields.iter().all(|(_, value)| constant(value)),
        Kind::WiringFailure(_)
        | Kind::Index(..)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::ValueCall(..)
        | Kind::Configuration(_)
        | Kind::Field(..)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::ReplaySlot(_)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability => false,
    }
}

/// Lexical owning locals for one ordinary evaluation context.
#[derive(Debug, Default)]
pub struct Evaluator {
    locals: Vec<(usize, Value, bool)>,
}
impl Evaluator {
    /// Preserve a checked unavailable wiring result while later source checks continue.
    pub fn bind_failed(&mut self, id: usize, value: Value, writable: bool) {
        self.locals.push((id, value, writable));
    }
    /// Evaluate a checked expression to a closed, independently owned value.
    pub fn value(&mut self, value: &Value) -> Result<Value, EvalError> {
        let kind = match &value.kind {
            Kind::WiringFailure(message) => return Err(EvalError::Operation(message.clone())),
            Kind::Literal(_) | Kind::Void => return Ok(value.clone()),
            Kind::List(items) => {
                length(items.len())?;
                Kind::List(
                    items
                        .iter()
                        .map(|item| self.value(item))
                        .collect::<Result<_, _>>()?,
                )
            }
            Kind::Construct(fields) => Kind::Construct(
                fields
                    .iter()
                    .map(|(id, item)| Ok((*id, self.value(item)?)))
                    .collect::<Result<_, EvalError>>()?,
            ),
            Kind::Local(id) | Kind::MutableLocal(id) => {
                let value = self
                    .locals
                    .iter()
                    .rev()
                    .find(|(local, _, _)| local == id)
                    .map(|(_, value, _)| value.clone())
                    .ok_or_else(|| unsupported("ordinary local is not initialized"))?;
                if let Kind::WiringFailure(message) = &value.kind {
                    return Err(EvalError::Operation(message.clone()));
                }
                return Ok(value);
            }
            Kind::Field(parent, field) => {
                return field_value(&self.value(parent)?, *field).cloned();
            }
            Kind::Index(parent, offset) => {
                let parent = self.value(parent)?;
                let offset = index(&self.value(offset)?)?;
                return list(&parent)?.get(offset).cloned().ok_or_else(bounds);
            }
            Kind::Length(parent) => {
                Kind::Literal(Literal::Int(length(list(&self.value(parent)?)?.len())?))
            }
            Kind::Push(parent, item) => {
                self.push(parent, item)?;
                Kind::Void
            }
            Kind::ValueCall(args, body) => return self.call(args, body, &value.ty),
            Kind::Binary(op, a, b) => return self.binary(op, a, b),
            Kind::Unary(op, operand) => return operators::unary(op, &self.value(operand)?),
            Kind::Configuration(_)
            | Kind::GlobalGet(_)
            | Kind::BorrowedLocal(..)
            | Kind::GlobalSet(..)
            | Kind::ReplaySlot(_)
            | Kind::IsPresent(_)
            | Kind::Present(_)
            | Kind::Wire(_)
            | Kind::Input(..)
            | Kind::Cache(_)
            | Kind::Native(..)
            | Kind::Query(..)
            | Kind::Output
            | Kind::Capability => {
                return Err(unsupported(
                    "expression requires an unavailable runtime or native context",
                ));
            }
        };
        Ok(Value::new(value.ty.clone(), kind))
    }
    fn binary(&mut self, op: &str, a: &Value, b: &Value) -> Result<Value, EvalError> {
        let a = self.value(a)?;
        if matches!(
            (op, &a.kind),
            ("&&", Kind::Literal(Literal::Bool(false)))
                | ("||", Kind::Literal(Literal::Bool(true)))
        ) {
            return Ok(a);
        }
        operators::binary(op, &a, &self.value(b)?)
    }
    fn call(&mut self, args: &[Value], body: &[Statement], ty: &Ty) -> Result<Value, EvalError> {
        let mut invocation = Self::default();
        for (id, arg) in args.iter().enumerate() {
            invocation.locals.push((id, self.value(arg)?, false));
        }
        match invocation.block(body)? {
            Some(value) if value.ty == *ty => Ok(value),
            None if *ty == Ty::Void => Ok(Value::new(Ty::Void, Kind::Void)),
            Some(_) | None => Err(unsupported("ordinary call has no correctly typed result")),
        }
    }
    /// Execute one statement; a yielded value ends the containing invocation.
    pub fn statement(&mut self, statement: &Statement) -> Result<Option<Value>, EvalError> {
        match statement {
            Statement::Let(id, value) | Statement::Var(id, value) => {
                let value = self.value(value)?;
                self.locals
                    .push((*id, value, matches!(statement, Statement::Var(..))));
            }
            Statement::Assign(target, value) => {
                if matches!(target.kind, Kind::Index(..)) {
                    return Err(unsupported("ordinary indexed replacement is not admitted"));
                }
                let value = self.value(value)?;
                let target = self.place(target)?;
                if target.ty != value.ty {
                    return Err(unsupported("ordinary assignment type mismatch"));
                }
                *target = value;
            }
            Statement::Call(value) => {
                self.value(value)?;
            }
            Statement::Yield(value) => return self.value(value).map(Some),
            Statement::Exit => return Ok(Some(Value::new(Ty::Void, Kind::Void))),
            Statement::If(condition, yes, no) => {
                let Kind::Literal(Literal::Bool(condition)) = self.value(condition)?.kind else {
                    return Err(unsupported("ordinary condition must be bool"));
                };
                return self.block(if condition { yes } else { no });
            }
            Statement::Borrow(..) | Statement::Return(_) | Statement::For(..) => {
                return Err(unsupported("statement requires runtime evaluation"));
            }
        }
        Ok(None)
    }
    fn block(&mut self, body: &[Statement]) -> Result<Option<Value>, EvalError> {
        let start = self.locals.len();
        let result = (|| {
            for statement in body {
                if let Some(value) = self.statement(statement)? {
                    return Ok(Some(value));
                }
            }
            Ok(None)
        })();
        self.locals.truncate(start);
        result
    }
    fn push(&mut self, parent: &Value, item: &Value) -> Result<(), EvalError> {
        let Ty::List(element, None) = &parent.ty else {
            return Err(unsupported("push requires an unbounded ordinary list"));
        };
        let item = self.value(item)?;
        if item.ty != **element {
            return Err(unsupported("ordinary push element type mismatch"));
        }
        let Kind::List(items) = &mut self.place(parent)?.kind else {
            return Err(unsupported("push receiver is not an ordinary list"));
        };
        length(items.len())?
            .checked_add(1)
            .ok_or_else(|| EvalError::Operation("ordinary list length exceeds i64".into()))?;
        items.try_reserve(1).map_err(|error| {
            EvalError::Operation(format!(
                "ordinary list capacity acquisition failed: {error}"
            ))
        })?;
        items.push(item);
        Ok(())
    }
    fn path(&mut self, value: &Value, path: &mut Vec<Projection>) -> Result<usize, EvalError> {
        match &value.kind {
            Kind::WiringFailure(message) => Err(EvalError::Operation(message.clone())),
            Kind::MutableLocal(id) => Ok(*id),
            Kind::Field(parent, field) => {
                let id = self.path(parent, path)?;
                path.push(Projection::Field(*field));
                Ok(id)
            }
            Kind::Index(parent, offset) => {
                let id = self.path(parent, path)?;
                path.push(Projection::Index(index(&self.value(offset)?)?));
                Ok(id)
            }
            Kind::List(_)
            | Kind::Length(_)
            | Kind::Push(..)
            | Kind::ValueCall(..)
            | Kind::Configuration(_)
            | Kind::Construct(_)
            | Kind::GlobalGet(_)
            | Kind::BorrowedLocal(..)
            | Kind::GlobalSet(..)
            | Kind::ReplaySlot(_)
            | Kind::IsPresent(_)
            | Kind::Present(_)
            | Kind::Literal(_)
            | Kind::Wire(_)
            | Kind::Input(..)
            | Kind::Cache(_)
            | Kind::Local(_)
            | Kind::Native(..)
            | Kind::Binary(..)
            | Kind::Unary(..)
            | Kind::Query(..)
            | Kind::Output
            | Kind::Capability
            | Kind::Void => Err(unsupported(
                "ordinary mutation requires writable owner access",
            )),
        }
    }
    fn place(&mut self, value: &Value) -> Result<&mut Value, EvalError> {
        let mut path = Vec::new();
        let id = self.path(value, &mut path)?;
        let (_, mut value, writable) = self
            .locals
            .iter_mut()
            .rev()
            .find(|(local, _, _)| *local == id)
            .map(|(id, value, writable)| (*id, value, *writable))
            .ok_or_else(|| unsupported("ordinary local is not initialized"))?;
        if !writable {
            return Err(unsupported("ordinary local is read-only"));
        }
        if let Kind::WiringFailure(message) = &value.kind {
            return Err(EvalError::Operation(message.clone()));
        }
        for projection in path {
            value = match (projection, &mut value.kind) {
                (Projection::Field(id), Kind::Construct(fields)) => fields
                    .iter_mut()
                    .find(|(field, _)| *field == id)
                    .map(|(_, value)| value)
                    .ok_or_else(|| unsupported("ordinary struct field is absent"))?,
                (Projection::Index(id), Kind::List(items)) => {
                    items.get_mut(id).ok_or_else(bounds)?
                }
                _ => return Err(unsupported("ordinary projection type mismatch")),
            };
        }
        Ok(value)
    }
}
enum Projection {
    Field(usize),
    Index(usize),
}
fn list(value: &Value) -> Result<&[Value], EvalError> {
    if let Kind::List(items) = &value.kind {
        Ok(items)
    } else {
        Err(unsupported("ordinary list required"))
    }
}
fn field_value(value: &Value, id: usize) -> Result<&Value, EvalError> {
    let Kind::Construct(fields) = &value.kind else {
        return Err(unsupported("ordinary struct required"));
    };
    fields
        .iter()
        .find(|(field, _)| *field == id)
        .map(|(_, value)| value)
        .ok_or_else(|| unsupported("ordinary struct field is absent"))
}
