//! Cold conditional rebinding of existing graph connections and ordinary locals.
use crate::ir::{Kind, Plan, Statement, Value};
use crate::local_check::GraphLocals;
use crate::value_access::StaticValues;
use crate::value_eval::{EvalError, Evaluator};
use hgl_source::{Expr, Literal, Stmt, Ty};
use std::collections::{BTreeMap, BTreeSet};
type Env = BTreeMap<String, Value>;

/// Identify conditional writes that need lexical connection assignment semantics.
pub fn handles(statement: &Stmt, env: &Env) -> bool {
    if let Stmt::If(_, yes, no) = statement {
        let ports = env
            .iter()
            .filter(|(_, value)| matches!(value.kind, Kind::Wire(_)))
            .map(|(name, _)| name.clone())
            .collect();
        return writes(yes, &ports) || writes(no, &ports);
    }
    false
}
fn port(expr: &Expr, ports: &BTreeSet<String>) -> bool {
    if let Expr::Name(name) = expr.syntax() {
        return ports.contains(name);
    }
    if let Expr::Binary(_, a, b) = expr.syntax() {
        return port(a, ports) || port(b, ports);
    }
    false
}
fn writes(body: &[Stmt], ports: &BTreeSet<String>) -> bool {
    let mut ports = ports.clone();
    for statement in body {
        if let Stmt::Let(name, _, expr) | Stmt::Var(name, _, expr) = statement {
            let temporal = port(expr, &ports);
            ports.remove(name);
            if temporal {
                ports.insert(name.clone());
            }
        }
        if let Stmt::Assign(Expr::Name(name), _) | Stmt::Add(Expr::Name(name), _) = statement
            && ports.contains(name)
        {
            return true;
        }
        if let Stmt::If(_, yes, no) = statement
            && (writes(yes, &ports) || writes(no, &ports))
        {
            return true;
        }
    }
    false
}

/// Check both cold branches and retain only the selected branch's bindings and nodes.
pub fn conditional(
    statement: &Stmt,
    env: &mut Env,
    ports: &BTreeSet<String>,
    (signals, statics): (&GraphLocals, &StaticValues),
    (plan, evaluator): (&mut Plan, &mut Evaluator),
    next: &mut usize,
) -> Result<(), String> {
    let Stmt::If(expr, yes, no) = statement else {
        return Err("expected wiring conditional".into());
    };
    let mut initial = Scope {
        env: env
            .iter()
            .map(|(name, value)| (name.clone(), statics.resolve(value).clone()))
            .collect(),
        ports: ports.clone(),
        signals: signals.clone(),
        evaluator: evaluator.clone(),
        next: *next,
    };
    let condition = expression(expr, &mut initial, plan)?;
    if matches!(condition.kind, Kind::Wire(_)) {
        return Err("temporal graph conditionals are unsupported by this backend".into());
    }
    if condition.ty != Ty::Bool {
        return Err("condition requires bool".into());
    }
    let Kind::Literal(Literal::Bool(selected)) = closed(condition, &mut initial, plan)?.kind else {
        return Err("wiring condition requires a fixed ordinary bool".into());
    };
    let start = plan.nodes.len();
    let incoming_error = plan.construction_error.clone();
    let mut branches = Vec::new();
    for body in [yes, no] {
        let mut scope = initial.clone();
        let outer = block(body, &mut scope, plan, statics)?;
        branches.push((
            scope,
            outer,
            plan.nodes.split_off(start),
            plan.construction_error.take(),
        ));
        plan.construction_error.clone_from(&incoming_error);
    }
    let (scope, outer, nodes, error) = branches.swap_remove(usize::from(!selected));
    for (name, value) in env.iter_mut() {
        if matches!(value.kind, Kind::Wire(_)) {
            *value = outer.get(name).ok_or("missing outer binding")?.clone();
        }
    }
    *evaluator = scope.evaluator;
    *next = scope.next;
    plan.nodes.extend(nodes);
    plan.construction_error = error;
    Ok(())
}

#[derive(Clone)]
struct Scope {
    env: Env,
    ports: BTreeSet<String>,
    signals: GraphLocals,
    evaluator: Evaluator,
    next: usize,
}
fn block(
    body: &[Stmt],
    scope: &mut Scope,
    plan: &mut Plan,
    statics: &StaticValues,
) -> Result<Env, String> {
    let mut outer = scope.env.clone();
    let mut shadows = BTreeSet::new();
    for statement in body {
        match statement {
            Stmt::Let(name, annotation, expr) | Stmt::Var(name, annotation, expr) => {
                if !shadows.insert(name.clone()) { return Err(format!("duplicate local {name}")); }
                let value = expression(expr, scope, plan)?;
                let annotation = annotation.as_ref().map(|ty| Ty::parse(ty).ok_or("wiring branch local requires a concrete built-in annotation")).transpose()?;
                let value = crate::local_check::initializer(value, annotation.as_ref())?;
                let mutable = matches!(statement, Stmt::Var(..));
                scope.signals.bind(name, expr);
                scope.ports.remove(name);
                let binding = if matches!(value.kind, Kind::Wire(_)) {
                    if mutable { scope.ports.insert(name.clone()); }
                    value
                } else {
                    let id = scope.next;
                    scope.next += 1;
                    let value = closed(value, scope, plan)?;
                    let (binding, statement) = crate::local_check::statement(id, value.clone(), mutable, annotation.is_some())?;
                    if matches!(value.kind, Kind::WiringFailure(_)) {
                        scope.evaluator.bind_failed(id, value, mutable);
                    } else { execute(&statement, scope, plan)?; }
                    binding
                };
                scope.env.insert(name.clone(), binding);
            }
            Stmt::Assign(Expr::Name(name), expr) | Stmt::Add(Expr::Name(name), expr) => {
                scope.signals.replacement(name, expr)?;
                let target = scope.env.get(name).cloned().ok_or("unknown wiring local")?;
                let value = if matches!(statement, Stmt::Add(..)) {
                    expression(&Expr::Binary("+".into(), Box::new(Expr::Name(name.clone())), Box::new(expr.clone())), scope, plan)?
                } else { expression(expr, scope, plan)? };
                let value = crate::local_check::replacement(&target, value, scope.ports.contains(name))?;
                if matches!(target.kind, Kind::Wire(_)) {
                    if !shadows.contains(name) { outer.insert(name.clone(), value.clone()); }
                    scope.env.insert(name.clone(), value);
                } else {
                    execute(&crate::local_check::assignment(target, value)?, scope, plan)?;
                }
            }
            Stmt::If(..) => {
                conditional(statement, &mut scope.env, &scope.ports, (&scope.signals, statics), (plan, &mut scope.evaluator), &mut scope.next)?;
                for (name, value) in &mut outer {
                    if !shadows.contains(name) && matches!(value.kind, Kind::Wire(_)) {
                        *value = scope.env.get(name).ok_or("missing outer binding")?.clone();
                    }
                }
            }
            Stmt::Assign(..) | Stmt::Add(..) | Stmt::Exit | Stmt::Return(_) | Stmt::Call(_) | Stmt::TimedYield(..) | Stmt::While(..) | Stmt::ForItems(..) | Stmt::For(..) => return Err("conditional connection rebinding admits local declarations, name assignments and ordinary if blocks".into()),
        }
    }
    Ok(outer)
}
fn expression(expr: &Expr, scope: &mut Scope, plan: &mut Plan) -> Result<Value, String> {
    match expr.syntax() {
        Expr::Located(..) => unreachable!("syntax strips source origins"),
        Expr::Name(name) => scope.env.get(name).cloned().ok_or_else(|| format!("unknown value {name}")),
        Expr::Literal(value) => Ok(Value::new(value.ty(), Kind::Literal(value.clone()))),
        Expr::Binary(op, a, b) => {
            scope.signals.payload((a, b))?;
            let mut a = expression(a, scope, plan)?;
            let mut b = expression(b, scope, plan)?;
            if matches!(a.kind, Kind::Wire(_)) || matches!(b.kind, Kind::Wire(_)) {
                if !matches!(a.kind, Kind::Wire(_)) { a = closed(a, scope, plan)?; }
                if !matches!(b.kind, Kind::Wire(_)) { b = closed(b, scope, plan)?; }
            }
            crate::local_check::binary(op, [a, b], plan)
        }
        Expr::Unary(op, child) if op == "!" => {
            let value = expression(child, scope, plan)?;
            if value.ty != Ty::Bool || matches!(value.kind, Kind::Wire(_)) {
                return Err("wiring negation requires ordinary bool".into());
            }
            Ok(Value::new(Ty::Bool, Kind::Unary(op.clone(), Box::new(value))))
        }
        Expr::Lambda(..) | Expr::Null | Expr::TemporalLiteral(_) | Expr::Sequence(_) | Expr::Tuple(_) | Expr::Sparse(_) | Expr::Call(..) | Expr::Applied(..) | Expr::Unary(..) | Expr::Property(..) | Expr::Index(..) => Err("conditional connection rebinding requires existing locals, scalar literals and binary expressions; calls are unsupported".into()),
    }
}
fn closed(value: Value, scope: &mut Scope, plan: &mut Plan) -> Result<Value, String> {
    match scope.evaluator.value(&value) {
        Ok(value) => Ok(value),
        Err(EvalError::Operation(message)) => {
            plan.construction_error.get_or_insert(message.clone());
            Ok(Value::new(value.ty, Kind::WiringFailure(message)))
        }
        Err(error) => Err(error.to_string()),
    }
}
fn execute(statement: &Statement, scope: &mut Scope, plan: &mut Plan) -> Result<(), String> {
    match scope.evaluator.statement(statement) {
        Ok(_) => Ok(()),
        Err(EvalError::Operation(message)) => {
            plan.construction_error.get_or_insert(message);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}
