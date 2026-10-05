//! Closed ordinary replay data and pre-start publication admission.
use hgl_composite_keys::{Key, key};
use hgl_rust_ir::{DeltaEntry, Kind, Value};
use hgl_source::{Expr, Literal, Ty};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct State {
    members: BTreeSet<Key>,
    children: BTreeMap<Key, Self>,
}
/// Validate sparse publications from a fresh endpoint, preserving prior membership.
pub fn validate(shape: &Ty, slots: &[Option<Value>]) -> Result<(), (usize, String)> {
    let mut state = State::default();
    for (index, value) in slots.iter().enumerate() {
        if let Some(value) = value {
            state.apply(shape, value).map_err(|error| (index, error))?;
        }
    }
    Ok(())
}
impl State {
    fn growing(&mut self, parts: &[DeltaEntry]) -> Result<(), String> {
        let items = parts.iter().filter_map(|part| {
            if let DeltaEntry::Child(index, _) = part {
                Some(*index)
            } else {
                None
            }
        });
        let removed = parts.iter().filter_map(|part| {
            if let DeltaEntry::Remove(Value {
                kind: Kind::Literal(Literal::Int(index)),
                ..
            }) = part
            {
                Some(*index)
            } else {
                None
            }
        });
        hgl_growing_range::validate(self.children.len(), items, removed.clone())
            .map_err(str::to_owned)?;
        for index in removed {
            self.children
                .remove(&hgl_composite_keys::scalar(&Literal::Int(index))?);
        }
        Ok(())
    }
    fn apply(&mut self, shape: &Ty, value: &Value) -> Result<(), String> {
        let Kind::Delta(parts) = &value.kind else {
            return Ok(());
        };
        if parts.is_empty() {
            return Err("empty structural publication".into());
        }
        hgl_delta_check::materialized(parts)?;
        if matches!(shape, Ty::List(_, None)) {
            self.growing(parts)?;
        }
        for part in parts {
            match (shape, part) {
                (Ty::List(_, None), DeltaEntry::Remove(_)) => {}
                (Ty::Set(_), DeltaEntry::Add(member)) => {
                    if !self.members.insert(key(member)?) {
                        return Err("set addition is already present".into());
                    }
                }
                (Ty::Set(_), DeltaEntry::Remove(member)) => {
                    if !self.members.remove(&key(member)?) {
                        return Err("set removal is absent".into());
                    }
                }
                (Ty::Map(..), DeltaEntry::Remove(member)) => {
                    if self.children.remove(&key(member)?).is_none() {
                        return Err("map removal is absent".into());
                    }
                }
                (Ty::Map(_, child), DeltaEntry::Keyed(member, value)) => self
                    .children
                    .entry(key(member)?)
                    .or_default()
                    .apply(child, value)?,
                (Ty::List(child, _), DeltaEntry::Child(index, value)) => self
                    .children
                    .entry(hgl_composite_keys::scalar(&Literal::Int(*index))?)
                    .or_default()
                    .apply(child, value)?,
                (Ty::Tuple(children), DeltaEntry::Child(index, value)) => {
                    let index = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(hgl_composite_keys::scalar(&Literal::Int(
                            i64::try_from(index).map_err(|e| e.to_string())?,
                        ))?)
                        .or_default()
                        .apply(&children[index], value)?;
                }
                (Ty::Struct(_, fields, _), DeltaEntry::Child(index, value)) => {
                    let position = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(hgl_composite_keys::scalar(&Literal::Int(*index))?)
                        .or_default()
                        .apply(&fields[position].1, value)?;
                }
                _ => return Err("delta does not match its checked publication shape".into()),
            }
        }
        Ok(())
    }
}
/// Turn present dense slots into owned ordinary timed entries; absence adds no data.
pub fn timed(entry_type: Ty, slots: &[Option<Value>]) -> Result<Value, String> {
    let mut entries = Vec::new();
    for (index, value) in slots.iter().enumerate() {
        if let Some(value) = value {
            let micros = i64::try_from(index)
                .map_err(|error| format!("eval timestamp overflow: {error}"))?
                .checked_add(1)
                .ok_or("eval timestamp overflow")?;
            entries.push(Value::new(
                entry_type.clone(),
                Kind::Construct(vec![
                    (
                        0,
                        Value::new(Ty::DateTime, Kind::Literal(Literal::DateTime(micros))),
                    ),
                    (1, value.clone()),
                ]),
            ));
        }
    }
    Ok(Value::new(
        Ty::List(Box::new(entry_type), None),
        Kind::List(entries),
    ))
}

/// Contextually check dense input cells without turning silence into a value.
pub fn sequence(
    ticks: &[Option<Expr>],
    mut ty: Option<Ty>,
    mut check: impl FnMut(&Expr, Option<&Ty>) -> Result<Value, String>,
) -> Result<(Ty, Vec<Option<Value>>), String> {
    let mut slots = Vec::new();
    for expr in ticks {
        let value = if let Some(expr) = expr {
            let expected = ty.clone().map(Ty::delta).transpose()?;
            let value = check(expr, expected.as_ref())?;
            ty.get_or_insert_with(|| {
                if let Ty::Delta(origin) = &value.ty {
                    *origin.clone()
                } else {
                    value.ty.clone()
                }
            });
            Some(value)
        } else {
            None
        };
        slots.push(value);
    }
    let ty = ty.ok_or("cannot infer empty generic sequence")?;
    if !ty.publication() {
        return Err("eval input is outside the delta publication profile".into());
    }
    Ok((ty, slots))
}

/// Ordinary test setup, executed once in source order before fresh eval graphs.
#[derive(Debug, Default)]
pub struct Scope {
    bindings: BTreeMap<String, Value>,
    next: usize,
    evaluator: hgl_value_eval::Evaluator,
}
impl Scope {
    /// Check one source statement while retaining the usual lexical write authority.
    pub fn apply(
        &mut self,
        statement: &hgl_source::Stmt,
        mut check: impl FnMut(
            &hgl_source::Stmt,
            &mut BTreeMap<String, Value>,
            &mut usize,
        ) -> Result<hgl_rust_ir::Statement, String>,
    ) -> Result<(), String> {
        if let hgl_source::Stmt::Let(name, _, _) | hgl_source::Stmt::Var(name, _, _) = statement
            && self.bindings.contains_key(name)
        {
            return Err(format!("duplicate test local {name}"));
        }
        let checked = check(statement, &mut self.bindings, &mut self.next)?;
        self.evaluator
            .statement(&checked)
            .map_err(|error| error.to_string())?;
        Ok(())
    }
    /// Independently retain a closed environment for one evaluation or assertion.
    pub fn values(&mut self) -> Result<BTreeMap<String, Value>, String> {
        self.bindings
            .iter()
            .map(|(name, value)| {
                Ok((
                    name.clone(),
                    self.evaluator
                        .value(value)
                        .map_err(|error| error.to_string())?,
                ))
            })
            .collect()
    }
}
/// Parse one ordinary setup statement without consuming subsequent assertions.
pub fn statement(cursor: &mut hgl_source::Cursor<'_>) -> Result<hgl_source::Stmt, String> {
    use hgl_source::Stmt;
    if cursor.at("let") || cursor.at("var") {
        let mutable = cursor.consume()? == "var";
        let name = cursor.name()?;
        let annotation = if cursor.take(":") {
            Some(cursor.type_name()?)
        } else {
            None
        };
        cursor.need("=")?;
        let value = cursor.expr()?;
        return Ok(if mutable {
            Stmt::Var(name, annotation, value)
        } else {
            Stmt::Let(name, annotation, value)
        });
    }
    if cursor.take("if") {
        let condition = cursor.expr()?;
        let yes = cursor.block()?;
        cursor.lines();
        let no = if cursor.take("else") {
            if cursor.at("if") {
                vec![statement(cursor)?]
            } else {
                cursor.block()?
            }
        } else {
            Vec::new()
        };
        return Ok(Stmt::If(condition, yes, no));
    }
    let target = cursor.expr()?;
    Ok(if cursor.take("=") {
        Stmt::Assign(target, cursor.expr()?)
    } else if cursor.take("+=") {
        Stmt::Add(target, cursor.expr()?)
    } else {
        Stmt::Call(target)
    })
}
/// Choose an unused run-owned recording key during graph construction.
pub fn recording_key(plan: &hgl_rust_ir::Plan) -> String {
    let used = plan
        .nodes
        .iter()
        .flat_map(|node| node.globals.iter().map(|(key, _)| key))
        .collect::<BTreeSet<_>>();
    let mut index = 0;
    loop {
        let key = format!("eval.recording.{index}");
        if !used.contains(&key) {
            return key;
        }
        index += 1;
    }
}

/// Parsed eval syntax, before name resolution or checking.
#[derive(Debug)]
pub struct Evaluation {
    /// Named target.
    pub function: String,
    /// Source-ordered arguments.
    pub arguments: Vec<(Option<String>, Expr)>,
    /// Optional dense expected output.
    pub expected: Option<Vec<Option<Expr>>>,
}
/// Extract an eval call or equality assertion without evaluating source data.
pub fn evaluation(expr: Expr, assertion: bool) -> Result<Evaluation, String> {
    let (call, expected) = if assertion {
        let Expr::Binary(op, call, expected) = expr else {
            return Err("expected eval comparison".into());
        };
        if op != "==" {
            return Err("eval assertion requires ==".into());
        }
        let Expr::Sequence(elements) = *expected else {
            return Err("expected a dense sequence".into());
        };
        (*call, Some(elements))
    } else {
        (expr, None)
    };
    let Expr::Call(eval, mut arguments) = call else {
        return Err("expected eval call".into());
    };
    if eval != "eval" || arguments.is_empty() {
        return Err("expected eval(function, ...)".into());
    }
    let (label, Expr::Name(function)) = arguments.remove(0) else {
        return Err("eval requires a named function".into());
    };
    if label.is_some() {
        return Err("eval function must be positional".into());
    }
    Ok(Evaluation {
        function,
        arguments,
        expected,
    })
}

/// Find the declaration parameter for one supplied eval argument.
pub fn parameter<'a>(
    signature: &'a hgl_library::Signature,
    position: usize,
    label: Option<&str>,
) -> Result<&'a hgl_library::Parameter, String> {
    label
        .map_or_else(
            || signature.parameters.get(position),
            |name| signature.parameters.iter().find(|p| p.name == name),
        )
        .ok_or_else(|| "unknown eval argument".into())
}

/// One parsed lexical test operation, without name or type resolution.
#[derive(Debug)]
pub enum TestStep {
    /// Ordinary setup statement.
    Ordinary(hgl_source::Stmt),
    /// Ordinary boolean assertion expression.
    Assert(Expr),
    /// Eval call with optional dense expected values.
    Eval(Evaluation),
    /// Inferred immutable binding of the returned dense logical sequence.
    BindEval(String, Evaluation),
    /// Lexical branches containing setup, assertions and graph runs.
    If(Expr, Vec<Self>, Vec<Self>),
}
/// Parse a test declaration's ordered setup, assertions and eval calls.
pub fn steps(tokens: &[hgl_source::Token]) -> Result<Vec<TestStep>, String> {
    let mut cursor = hgl_source::Cursor::new(tokens);
    cursor.need("test")?;
    cursor.name()?;
    step_block(&mut cursor)
}
fn step_block(cursor: &mut hgl_source::Cursor<'_>) -> Result<Vec<TestStep>, String> {
    cursor.need("{")?;
    cursor.lines();
    let mut steps = Vec::new();
    while !cursor.take("}") {
        steps.push(step(cursor)?);
        cursor.lines();
    }
    Ok(steps)
}
fn step(cursor: &mut hgl_source::Cursor<'_>) -> Result<TestStep, String> {
    if cursor.take("if") {
        let condition = cursor.expr()?;
        let yes = step_block(cursor)?;
        cursor.lines();
        let no = if cursor.take("else") {
            if cursor.at("if") {
                vec![step(cursor)?]
            } else {
                step_block(cursor)?
            }
        } else {
            Vec::new()
        };
        return Ok(TestStep::If(condition, yes, no));
    }
    if !cursor.at("assert") && !cursor.at("eval") {
        let ordinary = statement(cursor)?;
        if let hgl_source::Stmt::Let(name, annotation, expr)
        | hgl_source::Stmt::Var(name, annotation, expr) = &ordinary
            && matches!(expr, Expr::Call(name, _) if name == "eval")
        {
            if annotation.is_some() || matches!(ordinary, hgl_source::Stmt::Var(..)) {
                return Err("eval result requires an inferred immutable let binding".into());
            }
            return Ok(TestStep::BindEval(
                name.clone(),
                evaluation(expr.clone(), false)?,
            ));
        }
        return Ok(TestStep::Ordinary(ordinary));
    }
    let assertion = cursor.take("assert");
    let expr = cursor.expr()?;
    let eval = matches!(&expr,Expr::Binary(_,left,_) if matches!(left.as_ref(),Expr::Call(name,_) if name=="eval"));
    if assertion && !eval {
        Ok(TestStep::Assert(expr))
    } else {
        Ok(TestStep::Eval(evaluation(expr, assertion)?))
    }
}
