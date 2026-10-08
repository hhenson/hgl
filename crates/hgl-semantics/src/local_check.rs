//! Local value/category checking and scalar expression lifting during composition.
use crate::endpoint_check::require_payload;
use crate::ir::{Kind, Node, Plan, Statement, Value};
use crate::value_check::{ordinary, writable};
use hgl_source::{Expr, Ty};
use std::collections::BTreeSet;

/// Apply ordinary widening without turning a value into a connection.
fn widen(value: Value, expected: &Ty) -> Value {
    if *expected == Ty::F64 && value.ty == Ty::I64 && !matches!(value.kind, Kind::Wire(_)) {
        Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(value)))
    } else {
        value
    }
}

/// Check a resolved local annotation in the initializer's own category.
pub fn initializer(value: Value, annotation: Option<&Ty>) -> Result<Value, String> {
    let value = crate::tuple_values::retain(value, false);
    if value.ty == Ty::Void {
        return Err("statement operation has no initializer value".into());
    }
    if !matches!(value.ty, Ty::Nullable(_)) {
        owning_payload(&value)?;
    }
    let value = if let Some(ty) = annotation {
        widen(value, ty)
    } else {
        value
    };
    if annotation.is_some_and(|ty| *ty != value.ty) {
        return Err(
            "local initializer type mismatch; annotations do not create temporal endpoints".into(),
        );
    }
    Ok(value)
}

/// Bind an ordinary lexical local while preserving ownership and observations.
pub fn statement(
    id: usize,
    value: Value,
    mutable: bool,
    annotated: bool,
) -> Result<(Value, Statement), String> {
    if mutable {
        if value.snapshot {
            return Err("mutable retained tuple locals are outside this backend profile".into());
        }
        owning_payload(&value)?;
        if !ordinary(&value.ty) {
            return Err("mutable locals currently require an ordinary scalar or struct".into());
        }
    }
    let binding = crate::value_check::binding(id, &value, mutable, annotated)?;
    let statement = if matches!(binding.kind, Kind::BorrowedLocal(..)) {
        Statement::Borrow(id, value, mutable)
    } else if mutable {
        Statement::Var(id, value)
    } else {
        Statement::Let(id, value)
    };
    Ok((binding, statement))
}

/// Check assignment authority and resolve the existing publication context.
pub fn assignment_type(target: &Value, evaluation: bool) -> Result<Ty, String> {
    if (!evaluation && matches!(target.kind, Kind::Output))
        || !(matches!(target.kind, Kind::Output | Kind::Cache(_)) || writable(target))
    {
        return Err("assignment requires writable var, state, cache or out".into());
    }
    if matches!(target.kind, Kind::Output) {
        Ok(target.ty.clone().delta()?)
    } else {
        Ok(target.ty.clone())
    }
}

/// Ordinary mutation never changes a binding into a temporal connection.
pub fn assignment(target: Value, value: Value) -> Result<Statement, String> {
    let expected = assignment_type(&target, true)?;
    if matches!(value.kind, Kind::Wire(_)) {
        return Err("local assignment category mismatch: ordinary value required".into());
    }
    let value = if writable(&target) {
        widen(value, &expected)
    } else {
        value
    };
    if expected != value.ty || !ordinary(&expected) {
        return Err("assignment type mismatch".into());
    }
    owning_payload(&value)?;
    Ok(Statement::Assign(target, value))
}

/// Validate a graph local's authority and the replacement's fixed category/type.
pub fn replacement(target: &Value, value: Value, mutable_port: bool) -> Result<Value, String> {
    if !mutable_port && !writable(target) {
        return Err("assignment requires writable var".into());
    }
    if matches!(target.kind, Kind::Wire(_)) != matches!(value.kind, Kind::Wire(_)) {
        return Err("local assignment category mismatch".into());
    }
    let value = widen(value, &target.ty);
    if target.ty != value.ty {
        return Err("local assignment type mismatch".into());
    }
    Ok(value)
}

/// Build ordinary binary IR or a temporal scalar node from checked operands.
pub fn binary(op: &str, mut values: [Value; 2], plan: &mut Plan) -> Result<Value, String> {
    if values.iter().any(|value| {
        matches!(value.ty, Ty::Set(_) | Ty::Map(..))
            && matches!(
                value.kind,
                Kind::Wire(_) | Kind::IterationInput(_) | Kind::Input(..) | Kind::Output
            )
    }) {
        return Err(format!(
            "unsupported binary operation {op} on structural collection endpoints"
        ));
    }
    if values
        .iter()
        .any(|v| v.snapshot && matches!(v.ty, Ty::Tuple(_) | Ty::List(..) | Ty::Map(..)))
    {
        return Err("retained aggregate comparisons are outside this backend profile".into());
    }
    let temporal = values.iter().any(|v| matches!(v.kind, Kind::Wire(_)));
    let mut node = Node {
        name: format!("binary_{op}"),
        inputs: Vec::new(),
        result: Ty::Void,
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
    if temporal {
        for (index, value) in values.iter_mut().enumerate() {
            if let Kind::Wire(wire) = value.kind {
                let input = node.inputs.len();
                node.inputs
                    .push((format!("operand{index}"), wire, value.ty.clone()));
                value.kind = Kind::Input(input, false);
            } else if matches!(value.kind, Kind::Prepared(_)) {
                let index = node.configuration.len();
                node.configuration.push(value.clone());
                value.kind = Kind::Configuration(index);
            }
        }
    }
    let [mut a, mut b] = values;
    require_payload(&a)?;
    require_payload(&b)?;
    if a.ty == Ty::I64 && b.ty == Ty::F64 {
        a = widen(a, &Ty::F64);
    }
    if a.ty == Ty::F64 && b.ty == Ty::I64 {
        b = widen(b, &Ty::F64);
    }
    let ty = crate::value_check::binary_type(op, &a.ty, &b.ty)?;
    let value = Value::new(
        ty.clone(),
        Kind::Binary(op.into(), Box::new(a), Box::new(b)),
    );
    if !temporal {
        return Ok(value);
    }
    node.result = ty.clone();
    node.handlers.push((None, vec![Statement::Return(value)]));
    let index = plan.nodes.len();
    plan.nodes.push(node);
    Ok(Value::new(ty, Kind::Wire(index)))
}

/// Lexical signal restrictions follow aliases, independently of shared port identity.
#[derive(Debug, Default, Clone)]
pub struct GraphLocals {
    signals: BTreeSet<String>,
}
impl GraphLocals {
    /// Seed the current callable's formal signal names.
    pub fn new(signals: impl Iterator<Item = String>) -> Self {
        Self {
            signals: signals.collect(),
        }
    }
    /// Retain a signal initializer's access restriction across local aliases.
    pub fn bind(&mut self, name: &str, expr: &Expr) {
        if self.signal(expr) {
            self.signals.insert(name.into());
        } else {
            self.signals.remove(name);
        }
    }
    fn signal(&self, expr: &Expr) -> bool {
        matches!(expr.syntax(), Expr::Name(name) if self.signals.contains(name))
    }
    /// Reject payload arithmetic on a formal signal, including its lexical aliases.
    pub fn payload(&self, (a, b): (&Expr, &Expr)) -> Result<(), String> {
        if self.signal(a) || self.signal(b) {
            return Err("signal has no ordinary scalar value".into());
        }
        Ok(())
    }
    /// Signal payload access cannot be gained by rebinding a typed local.
    pub fn replacement(&self, name: &str, expr: &Expr) -> Result<(), String> {
        if self.signals.contains(name) || self.signal(expr) {
            return Err("signal connection rebinding is unsupported by this backend".into());
        }
        Ok(())
    }
}

/// Preserve the existing temporal result boundary after checking local mutations.
pub fn graph_result(mut output: Value, result: &Ty) -> Result<Value, String> {
    if *result == Ty::Void {
        return Ok(Value::new(Ty::Void, Kind::Void));
    }
    if (output.ty != *result && !matches!(&output.ty,Ty::Ref(child) if child.as_ref()==result))
        || !matches!(output.kind, Kind::Wire(_))
    {
        return Err("graph return type mismatch".into());
    }
    output.ty = result.clone();
    Ok(output)
}

fn owning_payload(value: &Value) -> Result<(), String> {
    if matches!(value.ty, Ty::Set(_)) && matches!(value.kind, Kind::Input(..) | Kind::Output) {
        return Err("structural endpoint payload requires delta_value observation".into());
    }
    require_payload(value)
}
