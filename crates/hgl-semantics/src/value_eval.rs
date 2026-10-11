//! Direct ordinary evaluation of checked IR without runtime capabilities.
use crate::ir::{DeltaEntry, Kind, Statement, Value};
use crate::value_operations as operators;
pub use crate::value_operations::EvalError;
use hgl_source::{Literal, TemporalLiteral, Ty};
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

/// Whether an expression is closed independently owned ordinary data.
pub fn constant(value: &Value) -> bool {
    value.closed()
}

/// Lexical owning locals for one ordinary evaluation context.
#[derive(Debug, Default, Clone)]
pub struct Evaluator {
    locals: Vec<(usize, Value, bool)>,
}
type Materialize<'a> = dyn FnMut(&TemporalLiteral) -> Result<Literal, EvalError> + 'a;
impl Evaluator {
    /// Execute a lexical scope, releasing its bindings while retaining outer writes.
    pub fn scoped<T, E>(&mut self, body: impl FnOnce(&mut Self) -> Result<T, E>) -> Result<T, E> {
        let start = self.locals.len();
        let result = body(self);
        self.locals.truncate(start);
        result
    }
    /// Preserve a checked unavailable wiring result while later source checks continue.
    pub fn bind_failed(&mut self, id: usize, value: Value, writable: bool) {
        self.locals.push((id, value, writable));
    }
    /// Evaluate a checked expression to a closed, independently owned value.
    pub fn value(&mut self, value: &Value) -> Result<Value, EvalError> {
        self.value_with(value, &mut |_| Err(EvalError::ContextRequired))
    }
    /// Evaluate with the host construction context, preserving expression order.
    pub fn value_with(
        &mut self,
        value: &Value,
        materialize: &mut Materialize<'_>,
    ) -> Result<Value, EvalError> {
        Execution {
            locals: &mut self.locals,
            materialize,
        }
        .value(value)
    }
    /// Execute one statement without a provider context.
    pub fn statement(&mut self, statement: &Statement) -> Result<Option<Value>, EvalError> {
        self.statement_with(statement, &mut |_| Err(EvalError::ContextRequired))
    }
    /// Execute a statement with provider-dependent literal materialization.
    pub fn statement_with(
        &mut self,
        statement: &Statement,
        materialize: &mut Materialize<'_>,
    ) -> Result<Option<Value>, EvalError> {
        Execution {
            locals: &mut self.locals,
            materialize,
        }
        .statement(statement)
    }
}
struct Execution<'a, 'b> {
    locals: &'a mut Vec<(usize, Value, bool)>,
    materialize: &'a mut Materialize<'b>,
}
impl Execution<'_, '_> {
    fn value(&mut self, value: &Value) -> Result<Value, EvalError> {
        let kind = match &value.kind {
            Kind::TemporalLiteral(recipe) => Kind::Literal((self.materialize)(recipe)?),
            Kind::Delta(parts) => self.delta(parts)?,
            Kind::WiringFailure(message) => return Err(EvalError::Operation(message.clone())),
            Kind::Captured(..) | Kind::Literal(_) | Kind::Void => return Ok(value.clone()),
            Kind::List(items) => self.collection(&value.ty, items)?,
            Kind::Construct(fields) => Kind::Construct(
                fields
                    .iter()
                    .map(|(id, item)| Ok((*id, self.value(item)?)))
                    .collect::<Result<_, EvalError>>()?,
            ),
            Kind::Local(id) | Kind::MutableLocal(id) => {
                let value = self.binding(*id)?.1.clone();
                if let Kind::WiringFailure(message) = &value.kind {
                    return Err(EvalError::Operation(message.clone()));
                }
                return Ok(value);
            }
            Kind::Field(parent, field) => {
                return field_value(&self.value(parent)?, *field).cloned();
            }
            Kind::Index(parent, offset) => return self.indexed(parent, offset, &value.ty),
            Kind::Length(parent) => Kind::Literal(Literal::Int(length(sequence_length(
                &self.value(parent)?,
            )?)?)),
            Kind::Push(parent, item) => {
                self.push(parent, item)?;
                Kind::Void
            }
            Kind::ValueCall(args, body) => return self.call(args, body, &value.ty),
            Kind::Binary(op, a, b) => return self.binary(op, a, b),
            Kind::Unary(op, operand) => {
                let operand = self.value(operand)?;
                if op == "family" {
                    return crate::family_values::retain(&value.ty, operand)
                        .map_err(|message| unsupported(&message));
                }
                return operators::unary(op, &operand);
            }
            Kind::IsPresent(operand) => Kind::Literal(Literal::Bool(!matches!(
                self.value(operand)?.kind,
                Kind::Void
            ))),
            Kind::Present(operand) => {
                let payload = self.value(operand)?;
                if matches!(payload.kind, Kind::Void) {
                    return Err(unsupported("absent nullable payload"));
                }
                return Ok(payload);
            }
            Kind::Prepared(_)
            | Kind::Configuration(_)
            | Kind::GlobalGet(_)
            | Kind::BorrowedLocal(..)
            | Kind::GlobalSet(..)
            | Kind::ObservedLocal(_)
            | Kind::GeneratorLocal(_)
            | Kind::Wire(_)
            | Kind::IterationInput(_)
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
    fn collection(&mut self, ty: &Ty, items: &[Value]) -> Result<Kind, EvalError> {
        length(items.len())?;
        Ok(Kind::List(if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
            crate::collection_values::evaluate(ty, items, |item| self.value(item))?
        } else {
            items
                .iter()
                .map(|item| self.value(item))
                .collect::<Result<_, _>>()?
        }))
    }
    fn indexed(&mut self, parent: &Value, offset: &Value, ty: &Ty) -> Result<Value, EvalError> {
        let parent = self.value(parent)?;
        let offset = index(&self.value(offset)?)?;
        if let Kind::Captured(size, slots) = &parent.kind {
            if offset >= *size {
                return Err(bounds());
            }
            return Ok(slots
                .binary_search_by_key(&offset, |(i, _)| *i)
                .ok()
                .map_or_else(
                    || Value::new(ty.clone(), Kind::Void),
                    |i| slots[i].1.clone(),
                ));
        }
        list(&parent)?.get(offset).cloned().ok_or_else(bounds)
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
        let mut locals = args
            .iter()
            .enumerate()
            .map(|(id, arg)| Ok((id, self.value(arg)?, false)))
            .collect::<Result<Vec<_>, EvalError>>()?;
        let mut invocation = Execution {
            locals: &mut locals,
            materialize: self.materialize,
        };
        match invocation.block(body)? {
            Some(value) if value.ty == *ty => Ok(value),
            None if *ty == Ty::Void => Ok(Value::new(Ty::Void, Kind::Void)),
            Some(_) | None => Err(unsupported("ordinary call has no correctly typed result")),
        }
    }
    fn statement(&mut self, statement: &Statement) -> Result<Option<Value>, EvalError> {
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
            Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::ForItems(..)
            | Statement::For(..)
            | Statement::While(..)
            | Statement::TimedYield(..) => {
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
    fn delta(&mut self, parts: &[DeltaEntry]) -> Result<Kind, EvalError> {
        let parts = parts
            .iter()
            .map(|part| part.try_map(|value| self.value(value)))
            .collect::<Result<Vec<_>, _>>()?;
        crate::delta_check::materialized(&parts).map_err(EvalError::Operation)?;
        Ok(Kind::Delta(parts))
    }
    fn path(&mut self, value: &Value, path: &mut Vec<Projection>) -> Result<usize, EvalError> {
        if let Kind::WiringFailure(message) = &value.kind {
            return Err(EvalError::Operation(message.clone()));
        }
        if let Kind::MutableLocal(id) = value.kind {
            return Ok(id);
        }
        if let Kind::Field(parent, field) = &value.kind {
            let id = self.path(parent, path)?;
            path.push(Projection::Field(*field));
            return Ok(id);
        }
        if let Kind::Index(parent, offset) = &value.kind {
            let id = self.path(parent, path)?;
            path.push(Projection::Index(index(&self.value(offset)?)?));
            return Ok(id);
        }
        Err(unsupported(
            "ordinary mutation requires writable owner access",
        ))
    }
    fn binding(&mut self, id: usize) -> Result<&mut (usize, Value, bool), EvalError> {
        self.locals
            .iter_mut()
            .rev()
            .find(|(local, _, _)| *local == id)
            .ok_or_else(|| unsupported("ordinary local is not initialized"))
    }
    fn place(&mut self, value: &Value) -> Result<&mut Value, EvalError> {
        let mut path = Vec::new();
        let id = self.path(value, &mut path)?;
        let (_, value, writable) = self.binding(id)?;
        let mut value = value;
        if !*writable {
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
    let id = if let Ty::Family(family) = &value.ty {
        let name = &family.members()[0]
            .1
            .structure()
            .map_err(EvalError::Unsupported)?
            .1[id]
            .0;
        crate::family_values::concrete(value)
            .ty
            .structure()
            .map_err(EvalError::Unsupported)?
            .1
            .iter()
            .position(|(field, _)| field == name)
            .ok_or_else(|| unsupported("missing common family field"))?
    } else {
        id
    };
    let value = crate::family_values::concrete(value);
    let Kind::Construct(fields) = &value.kind else {
        return Err(unsupported("ordinary struct required"));
    };
    fields
        .iter()
        .find(|(field, _)| *field == id)
        .map(|(_, value)| value)
        .ok_or_else(|| unsupported("ordinary struct field is absent"))
}

fn sequence_length(value: &Value) -> Result<usize, EvalError> {
    if let Kind::Literal(Literal::Bytes(bytes)) = &value.kind {
        Ok(bytes.len())
    } else if let Kind::Captured(size, _) = value.kind {
        Ok(size)
    } else {
        Ok(list(value)?.len())
    }
}

/// Check an ordinary assertion while leaving admitted coded failures in its execution phase.
pub fn check_assertion(value: &Value) -> Result<(), String> {
    if value.ty != Ty::Bool {
        return Err("ordinary assertion requires bool".into());
    }
    if crate::value_constant::context_free(value) {
        match Evaluator::default().value(value) {
            Ok(_) | Err(EvalError::Coded(..)) => {}
            Err(error) => return Err(format!("constant evaluation: {error}")),
        }
    }
    Ok(())
}

/// A readable complete atomic input supplies an ordinary helper's exact payload.
pub fn atomic_argument(value: Value, expected: Option<&Ty>) -> Value {
    if let Ty::Atomic(payload) = &value.ty
        && matches!(value.kind, Kind::Input(_, false))
        && expected.is_some_and(|expected| expected == payload.as_ref())
    {
        let ty = *payload.clone();
        let kind = Kind::Unary("atomic_value".into(), Box::new(value));
        return Value::new(ty, kind);
    }
    value
}
