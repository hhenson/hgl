//! Contextual tagged data and checked variant facts share ordinary control flow.
use super::{Checker, Env, Expr, Issue, Kind, Literal, Phase, Ty, Value, ValueContext};
fn issue(code: &'static str, message: &str) -> Issue {
    Issue::coded(
        if code == "result.propagation_phase" {
            "phase"
        } else {
            "type"
        },
        code,
        0..0,
        message,
    )
}
fn context_type() -> Ty {
    Ty::Struct(
        "\0error.context".into(),
        vec![
            ("node".into(), Ty::Str),
            ("phase".into(), Ty::Str),
            ("evaluation_time".into(), Ty::DateTime),
        ],
        vec![],
    )
}
impl Checker {
    pub(super) fn lifted_outcome_body(
        &mut self,
        decl: &super::Decl,
        signature: &super::Signature,
        body: &[super::Stmt],
        env: &Env,
        result: &Ty,
    ) -> Result<Value, Issue> {
        let args = signature
            .parameters
            .iter()
            .map(|parameter| {
                env.get(&parameter.name)
                    .cloned()
                    .ok_or_else(|| Issue::from("missing lifted argument"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut scope = env.clone();
        for (id, parameter) in signature.parameters.iter().enumerate() {
            scope.insert(
                parameter.name.clone(),
                Value::new(args[id].ty.clone(), Kind::Local(id)),
            );
        }
        let context = std::mem::replace(&mut self.value_context, ValueContext::Runtime);
        let previous = self.propagation_result.replace(result.clone());
        let facts = std::mem::take(&mut self.facts);
        let body = self.statements(&decl.module, body, &mut scope, result, &mut args.len());
        self.value_context = context;
        self.propagation_result = previous;
        self.facts = facts;
        Ok(Value::new(result.clone(), Kind::ValueCall(args, body?)))
    }

    pub(super) fn outcome_expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        mode: (bool, Option<&Ty>),
    ) -> Result<Option<Value>, Issue> {
        if let Expr::Call(name, args) = expr.syntax() {
            self.outcome_constructor(module, name, args, env, mode)
        } else {
            Ok(None)
        }
    }
    pub(super) fn outcome_constructor(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        (runtime, expected): (bool, Option<&Ty>),
    ) -> Result<Option<Value>, Issue> {
        if matches!(name, "ok" | "err") {
            let Some(target @ Ty::Result(ok, err)) = expected else {
                return Err(issue(
                    "result.context_type",
                    "ok/err require an expected complete result type",
                ));
            };
            if !hgl_semantics::value_check::ordinary(ok)
                || !hgl_semantics::value_check::ordinary(err)
            {
                return Err(issue(
                    "result.context_type",
                    "result payloads require ordinary types",
                ));
            }
            let [(label, expr)] = args else {
                return Err("ok/err require one positional payload".into());
            };
            if label.is_some() {
                return Err("ok/err require one positional payload".into());
            }
            let success = name == "ok";
            let payload = if success { ok.as_ref() } else { err.as_ref() };
            let value = self.expected_expression(module, expr, env, runtime, Some(payload))?;
            if value.ty != *payload {
                return Err(issue(
                    "result.context_type",
                    "result selected payload requires exact type",
                ));
            }
            return Ok(Some(Value::new(
                target.clone(),
                Kind::Construct(vec![
                    (
                        0,
                        Value::new(Ty::Bool, Kind::Literal(Literal::Bool(success))),
                    ),
                    (if success { 1 } else { 2 }, value),
                ]),
            )));
        }
        if name != "error" {
            return Ok(None);
        }
        let mut fields = Vec::new();
        for (position, (label, expr)) in args.iter().enumerate() {
            let index = match label.as_deref() {
                Some("message") => 0,
                Some("code") => 1,
                None if position == 0 => 0,
                _ => return Err("unknown error constructor argument".into()),
            };
            if fields.iter().any(|(i, _)| *i == index) {
                return Err("duplicate error constructor argument".into());
            }
            if index == 1 && matches!(expr.syntax(), Expr::Null) {
                continue;
            }
            let value = self.expected_expression(module, expr, env, runtime, Some(&Ty::Str))?;
            if value.ty != Ty::Str {
                return Err("error message/code require str".into());
            }
            fields.push((index, value));
        }
        if !fields.iter().any(|(i, _)| *i == 0) {
            return Err("error requires a message".into());
        }
        Ok(Some(Value::new(Ty::Error, Kind::Construct(fields))))
    }
    pub(super) fn outcome_predicate(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        let [(None, expr)] = args else {
            return Err("result predicate requires one positional operand".into());
        };
        let value = self.expression(module, expr, env, runtime)?;
        self.outcome_predicate_values(name, vec![(None, value)])
    }
    pub(super) fn outcome_predicate_values(
        &mut self,
        name: &str,
        args: Vec<(Option<String>, Value)>,
    ) -> Result<Value, Issue> {
        let [(name_arg, mut value)]: [(Option<String>, Value); 1] =
            args.try_into()
                .map_err(|args: Vec<(Option<String>, Value)>| {
                    Issue::from(format!(
                        "result predicate requires one positional operand, found {}",
                        args.len()
                    ))
                })?;
        if name_arg.is_some() {
            return Err("result predicate requires one positional operand".into());
        }
        if !matches!(value.ty, Ty::Result(..)) {
            return Err("result predicate requires result<T,E>".into());
        }
        if let Kind::Wire(source) = value.kind {
            let ty = value.ty.clone();
            value.kind = Kind::Input(0, false);
            let id = self.plan.nodes.len();
            self.plan.nodes.push(hgl_semantics::ir::Node {
                name: format!("hgl.{name}"),
                inputs: vec![("value".into(), source, ty)],
                result: Ty::Bool,
                alarm: false,
                generator: None,
                start: vec![],
                global_state: false,
                globals: vec![],
                configuration: vec![],
                stop: vec![],
                caches: vec![],
                handlers: vec![(
                    None,
                    vec![hgl_semantics::ir::Statement::Return(Value::new(
                        Ty::Bool,
                        Kind::Query(name.into(), vec![value]),
                    ))],
                )],
            });
            return Ok(Value::new(Ty::Bool, Kind::Wire(id)));
        }
        Ok(Value::new(Ty::Bool, Kind::Query(name.into(), vec![value])))
    }
    pub(super) fn outcome_property(
        &self,
        parent: &Value,
        name: &str,
    ) -> Result<Option<Value>, Issue> {
        let (ty, op) = if let Ty::Result(ok, err) = &parent.ty {
            let (fact, ty, op) = match name {
                "value" => ("ok", ok.as_ref(), "\0result_value"),
                "error" => ("err", err.as_ref(), "\0result_error"),
                _ => return Err("unknown result property".into()),
            };
            if !matches!(parent.kind,Kind::Local(id) if self.facts.contains(&(fact.into(),id))) {
                return Err(issue(
                    "result.required_variant",
                    "result payload requires a checked immutable binding variant",
                ));
            }
            (ty.clone(), op)
        } else if parent.ty == Ty::Error {
            match name {
                "message" => (Ty::Str, "\0error_message"),
                "code" => (Ty::Nullable(Box::new(Ty::Str)), "\0error_code"),
                "context" => (Ty::Nullable(Box::new(context_type())), "\0error_context"),
                _ => return Err("unknown error property".into()),
            }
        } else if parent.ty == context_type() {
            match name {
                "node" => (Ty::Str, "\0context_node"),
                "phase" => (Ty::Str, "\0context_phase"),
                "evaluation_time" => (Ty::DateTime, "\0context_time"),
                _ => return Err("unknown error context property".into()),
            }
        } else {
            return Ok(None);
        };
        Ok(Some(Value::new(
            ty,
            Kind::Query(op.into(), vec![parent.clone()]),
        )))
    }
    pub(super) fn propagation(&self, value: Value) -> Result<Value, Issue> {
        if (self.value_context == ValueContext::Outside
            && (self.generator || self.phase != Phase::Evaluation || !self.runtime_node))
            || (self.value_context != ValueContext::Outside && self.propagation_result.is_none())
        {
            return Err(issue(
                "result.propagation_phase",
                "propagation requires an ordinary helper or node when",
            ));
        }
        let Ty::Result(ok, err) = &value.ty else {
            return Err(issue(
                "result.propagation_type",
                "propagation requires result<T,E>",
            ));
        };
        let ty = *ok.clone();
        if self.value_context == ValueContext::Outside {
            if **err != Ty::Error {
                return Err(issue(
                    "result.propagation_type",
                    "node propagation requires canonical error",
                ));
            }
            return Ok(Value::new(
                ty,
                Kind::Query("\0propagate_node".into(), vec![value]),
            ));
        }
        let Some(target @ Ty::Result(_, target_error)) = &self.propagation_result else {
            return Err(issue(
                "result.propagation_type",
                "helper propagation requires a result return type",
            ));
        };
        if target_error != err {
            return Err(issue(
                "result.propagation_type",
                "propagated error type must match the helper return type",
            ));
        }
        Ok(Value::new(
            ty,
            Kind::Query(
                "\0propagate_helper".into(),
                vec![value, Value::new(target.clone(), Kind::Void)],
            ),
        ))
    }
}
