use crate::index::{Decl, Library, Role, Signature};
use crate::syntax::{Cursor, Expr, Literal, Stmt, Ty};
use hgl_rust::{Kind, Native, Node, Plan, Statement, Value};
use hgl_semantics::body_check::runtime_body;
use hgl_semantics::endpoint_check::{
    capability_function, capability_payload, clock_property, endpoint_call, endpoint_metadata,
    injected_clock, require_payload, set_call,
};
use hgl_semantics::flow_check::{facts, handler_facts, merge_facts, terminates};
use hgl_semantics::static_values::PreparedLexicalScope;
use hgl_semantics::value_bind::{
    bind, bind_prepared, method_arguments, order_arguments, resolve_type,
};
use hgl_semantics::value_check::{ordinary, writable};
use hgl_source::Issue;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Checker {
    library: Library,
    plan: Plan,
    active: BTreeSet<usize>,
    documented: BTreeSet<usize>,
    test_scope: Option<String>,
    harness_values: Env,
    static_values: hgl_semantics::value_access::StaticValues,
    result_hint: Option<Ty>,
    requirements: Requirements,
    phase: Phase,
    types: BTreeMap<String, Ty>,
    runtime_node: bool,
    value_context: ValueContext,
    generator: bool,
    preparing_graph: bool,
    graph_locals: hgl_semantics::local_check::GraphLocals,
    wiring: hgl_semantics::value_eval::Evaluator,
    next_wiring_local: usize,
    global_types: BTreeMap<String, Ty>,
    globals: Vec<(String, Ty)>,
    failed_globals: BTreeSet<usize>,
    facts: BTreeSet<(String, usize)>,
}
#[derive(Default)]
struct Requirements {
    delta: bool,
    result: bool,
    complete: bool,
}
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ValueContext {
    #[default]
    Outside,
    Constant,
    Preparation,
    Runtime,
}
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    #[default]
    Evaluation,
    Stop,
}
type Selection = (usize, Signature, Vec<Value>, BTreeMap<String, Ty>, Ty);
type Env = BTreeMap<String, Value>;
type Arguments = Vec<(Option<String>, Value)>;

fn test_checker(library: Library, module: &str, scope: &PreparedLexicalScope) -> Checker {
    Checker {
        library,
        test_scope: Some(module.into()),
        harness_values: scope.bindings.clone(),
        static_values: scope.origins.clone(),
        facts: scope.facts.clone(),
        value_context: ValueContext::Preparation,
        ..Checker::default()
    }
}
pub(crate) fn prepared_statement(
    library: Library,
    module: &str,
    statement: &Stmt,
    scope: &mut PreparedLexicalScope,
) -> Result<Statement, Issue> {
    let mut checker = test_checker(library, module, scope);
    let result = checker.statement(
        module,
        statement,
        &mut scope.bindings,
        &Ty::Void,
        &mut scope.next,
    );
    scope.origins = checker.static_values;
    result
}
pub(crate) fn prepared_assertion(
    library: Library,
    module: &str,
    expr: &Expr,
    scope: &PreparedLexicalScope,
) -> Result<Value, Issue> {
    let mut checker = test_checker(library, module, scope);
    let value = checker.expression(module, expr, &scope.bindings, false)?;
    if value.ty != Ty::Bool {
        return Err("ordinary assertion requires bool".into());
    }
    if hgl_semantics::value_check::context_free(&value) {
        checker
            .wiring
            .value(&value)
            .map_err(|e| format!("constant evaluation: {e}"))?;
    }
    Ok(value)
}
pub(crate) fn compile(library: Library, module: &str, entry: &str) -> Result<Plan, Issue> {
    let mut checker = Checker {
        library,
        ..Checker::default()
    };
    checker.call(module, entry, &[], false)?;
    Ok(checker.plan)
}
impl Checker {
    fn call(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Value, Issue> {
        let values = self.call_values(args, runtime)?;
        let args = values.as_slice();
        let (module, name) = hgl_semantics::value_types::identity(&self.library, module, name);
        let (mut id, mut signature, values, mut types, result) =
            self.select(&module, &name, args, runtime)?;
        if self.preparing_graph
            && values
                .iter()
                .any(|v| matches!(v.kind, Kind::TemporalLiteral(_)))
        {
            return Err(
                "contextual temporal defaults in composition require unsupported run preparation"
                    .into(),
            );
        }
        let mut decl = self.library.declarations[id].clone();
        self.document(id);
        if decl.role == Role::Operator {
            (id, signature, types) =
                self.implementation(&decl, &signature, &values, &result, runtime)?;
            decl = self.library.declarations[id].clone();
            self.document(id);
        }
        if decl.role == Role::Native {
            return Ok(hgl_semantics::value_bind::native(
                &self.library,
                &decl,
                &signature,
                (values, result),
                runtime,
                &mut self.plan.natives,
            )?);
        }
        let runtime_body = runtime_body(&signature)?;
        if signature.value_function && values.iter().all(|v| !matches!(v.kind, Kind::Wire(_))) {
            return self
                .direct_call(id, &signature, (values, types, result), runtime, args)
                .map_err(|e| e.in_source(&decl.source));
        }
        if runtime {
            return Err("temporal calls in runtime handlers are not supported".into());
        }
        if !self.active.insert(id) {
            return Err(format!("recursive graph construction: {name}").into());
        }
        let previous_scope = self.test_scope.clone();
        self.test_scope = decl.test_only.then(|| module.clone());
        let mut env = hgl_semantics::body_check::parameter_scope(&signature, values);
        let previous_types = std::mem::replace(&mut self.types, types);
        let mut cursor = Cursor::new(&signature.body);
        let value = if runtime_body {
            self.runtime_node = !signature.value_function;
            let context = std::mem::replace(&mut self.value_context, ValueContext::Outside);
            let node = self
                .node(&decl, &signature, &mut cursor, &mut env, &result)
                .map_err(|e| e.in_source(&decl.source))?;
            self.value_context = context;
            let index = self.plan.nodes.len();
            self.plan.nodes.push(node);
            let kind = if result == Ty::Void {
                Kind::Void
            } else {
                Kind::Wire(index)
            };
            Value::new(result.clone(), kind)
        } else {
            let previous = std::mem::replace(&mut self.preparing_graph, true);
            let value = self
                .graph(&module, &mut cursor, &mut env, (&result, &signature))
                .map_err(|e| e.in_source(&decl.source))?;
            self.preparing_graph = previous;
            value
        };
        self.types = previous_types;
        self.active.remove(&id);
        self.test_scope = previous_scope;
        Ok(value)
    }

    fn call_values(
        &mut self,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Arguments, Issue> {
        args.iter().map(|(name, value)| {
            let prepared = self.value_context == ValueContext::Preparation;
            let local = matches!(value.kind, Kind::Local(id) | Kind::MutableLocal(id) if id < self.next_wiring_local);
            let value = if runtime || matches!(value.kind, Kind::Wire(_)) || (prepared && !local) {
                value.clone()
            } else { self.wiring_value(value)? };
            Ok((name.clone(), value))
        }).collect()
    }
    fn constant_size(&mut self, module: &str, source: &str, env: &Env) -> Result<Literal, Issue> {
        let tokens = hgl_source::lex(source)?;
        let mut cursor = Cursor::new(&tokens);
        let expr = cursor.expr()?;
        if !cursor.at("") {
            return Err("invalid list-size expression".into());
        }
        let env = env
            .iter()
            .map(|(name, value)| (name.clone(), self.static_values.resolve(value).clone()))
            .collect();
        let value = self.expression(module, &expr, &env, false)?;
        let value = self
            .wiring
            .value(&value)
            .map_err(|e| format!("constant list size: {e}"))?;
        let Kind::Literal(value) = value.kind else {
            return Err("list size requires a constant scalar".into());
        };
        Ok(value)
    }
    fn type_sizes(&mut self, module: &str, name: &str, env: &Env) -> Result<String, Issue> {
        for (owner, expr) in hgl_semantics::struct_check::schema_sizes(&self.library, module, name)?
        {
            let size = self.constant_size(&owner, &expr, &Env::new())?;
            self.library.type_sizes.insert((owner, expr), size);
        }
        hgl_semantics::type_sizes::normalize_checked(name, &mut |source| {
            self.constant_size(module, source, env)
        })
    }
    fn signature_sizes(
        &mut self,
        module: &str,
        mut signature: Signature,
        args: &[(Option<String>, Value)],
        source: &str,
    ) -> Result<Signature, Issue> {
        let names = signature
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>();
        let supplied = order_arguments(&names, args)?;
        let env = signature
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, p)| p.constant)
            .filter_map(|(id, p)| {
                supplied
                    .get(&id)
                    .cloned()
                    .or_else(|| {
                        p.default
                            .as_ref()?
                            .fixed()
                            .map(|v| Value::new(v.ty(), Kind::Literal(v)))
                    })
                    .map(|v| (p.name.clone(), v))
            })
            .collect::<Env>();
        for parameter in &mut signature.parameters {
            parameter.ty = self.type_sizes(module, &parameter.ty, &env).map_err(|e| {
                hgl_semantics::library::locate_type_issue(e, &parameter.type_tokens, source)
            })?;
        }
        signature.result = self
            .type_sizes(module, &signature.result, &env)
            .map_err(|e| {
                hgl_semantics::library::locate_type_issue(e, &signature.result_tokens, source)
            })?;
        Ok(signature)
    }
    fn direct_call(
        &mut self,
        id: usize,
        signature: &Signature,
        (args, types, result): (Vec<Value>, BTreeMap<String, Ty>, Ty),
        runtime: bool,
        supplied: &[(Option<String>, Value)],
    ) -> Result<Value, Issue> {
        let decl = self.library.declarations[id].clone();
        let (args, positions) = hgl_semantics::value_bind::ordered_call(signature, args, supplied)?;
        if !self.active.insert(id) {
            return Err("recursive ordinary value calls are unsupported".into());
        }
        let mut env = hgl_semantics::body_check::value_scope(
            signature,
            &args,
            &positions,
            self.value_context == ValueContext::Preparation,
        );
        let constants = self.static_values.arguments(signature, &args, &positions);
        let previous_constants = std::mem::replace(&mut self.static_values.locals, constants);
        let previous_types = std::mem::replace(&mut self.types, types);
        let runtime = match self.value_context {
            ValueContext::Outside => runtime,
            ValueContext::Constant | ValueContext::Preparation => false,
            ValueContext::Runtime => true,
        };
        let next_context = if self.value_context == ValueContext::Preparation {
            ValueContext::Preparation
        } else if runtime {
            ValueContext::Runtime
        } else {
            ValueContext::Constant
        };
        let previous_context = std::mem::replace(&mut self.value_context, next_context);
        let previous_runtime = std::mem::replace(&mut self.runtime_node, false);
        let mut cursor = Cursor::new(&signature.body);
        let (services, body) = hgl_semantics::body_check::ordinary_body(&mut cursor)?;
        for service in services {
            if !runtime || !matches!(service.as_str(), "clock" | "logger") {
                return Err(format!(
                    "ordinary helper injectable {service} requires a supported runtime service context"
                ).into());
            }
            if env
                .insert(service.clone(), Value::new(Ty::Void, Kind::Capability))
                .is_some()
            {
                return Err(
                    format!("{service}: injectable conflicts with an existing binding").into(),
                );
            }
        }
        let previous_delta = self.requirements.result;
        self.requirements.result = hgl_source::delta_argument(&signature.result).is_some();
        let body = self
            .statements(&decl.module, &body, &mut env, &result, &mut args.len())
            .map_err(|mut issue| {
                if issue.code == Some("delta.unsupported_shape") && !signature.generics.is_empty() {
                    issue.span = 0..0;
                    issue
                } else {
                    issue.in_source(&decl.source)
                }
            })?;
        self.requirements.result = previous_delta;
        self.types = previous_types;
        self.value_context = previous_context;
        self.static_values.locals = previous_constants;
        self.runtime_node = previous_runtime;
        self.active.remove(&id);
        if result != Ty::Void && !terminates(&body) {
            return Err("ordinary value function requires a return on every path".into());
        }
        Ok(Value::new(result, Kind::ValueCall(args, body)))
    }
    fn select(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Selection, Issue> {
        let mut candidates = Vec::new();
        let mut errors = Vec::new();
        for (id, decl) in self.library.declarations.clone().iter().enumerate() {
            if !hgl_semantics::name_check::visible(
                &self.library,
                decl,
                module,
                name,
                self.test_scope.as_deref(),
            ) {
                continue;
            }
            let signature = match decl
                .signature_checked()
                .and_then(|s| self.signature_sizes(module, s, args, &decl.source))
            {
                Ok(s) => s,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            if decl.role == Role::Native && !signature.body.is_empty() {
                continue;
            }
            let declared = match hgl_semantics::value_bind::signature_types(
                &self.library,
                &decl.module,
                &signature,
                args,
            ) {
                Ok(types) => types,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };
            if signature.body.iter().any(|token| token.text == "yield")
                && resolve_type(&signature.result, &declared).is_some_and(|ty| !ty.publication())
            {
                errors.push("generator requires a declared publication output type".into());
                continue;
            }
            let binder = if self.value_context == ValueContext::Preparation {
                bind_prepared
            } else {
                bind
            };
            match binder(
                &signature,
                args,
                runtime || decl.role == Role::Native,
                self.result_hint.as_ref(),
                &declared,
            ) {
                Ok((values, types, result)) => {
                    candidates.push((id, signature, values, types, result));
                }
                Err(e) => errors.push(e.into()),
            }
        }
        if candidates.len() != 1 {
            return Err(Issue::alternatives(
                errors,
                format!(
                    "{module}::{name}: expected one matching declaration, found {}",
                    candidates.len()
                ),
            ));
        }
        Ok(candidates.remove(0))
    }
    fn implementation(
        &mut self,
        declaration: &Decl,
        signature: &Signature,
        values: &[Value],
        result: &Ty,
        runtime: bool,
    ) -> Result<(usize, Signature, BTreeMap<String, Ty>), Issue> {
        let module = declaration.module.as_str();
        let name = declaration.name.as_str();
        let mut implementations = Vec::new();
        let mut errors = Vec::new();
        for (candidate, implementation) in self.library.declarations.clone().iter().enumerate() {
            if implementation.module != module
                || implementation.name != name
                || implementation.role != Role::Implementation
            {
                continue;
            }
            let positional = values
                .iter()
                .cloned()
                .map(|v| (None, v))
                .collect::<Vec<_>>();
            let sig = match implementation
                .signature_checked()
                .and_then(|s| self.signature_sizes(module, s, &positional, &implementation.source))
            {
                Ok(sig) => sig,
                Err(error) => {
                    errors.push(error.in_source(&implementation.source));
                    continue;
                }
            };
            let Ok(declared) = hgl_semantics::value_bind::signature_types(
                &self.library,
                module,
                &sig,
                &positional,
            ) else {
                continue;
            };
            let binder = if self.value_context == ValueContext::Preparation {
                bind_prepared
            } else {
                bind
            };
            let Ok((_, inferred, output)) =
                binder(&sig, &positional, runtime, Some(result), &declared)
            else {
                continue;
            };
            if !self.requirement(module, &sig, &inferred) {
                continue;
            }
            if output != *result
                || sig
                    .parameters
                    .iter()
                    .zip(&signature.parameters)
                    .any(|(a, b)| a.name != b.name || a.constant != b.constant)
            {
                continue;
            }
            if !sig.generics.is_empty()
                && !hgl_semantics::value_bind::instantiated(
                    &self.library,
                    module,
                    name,
                    &sig.generics,
                    &inferred,
                )
            {
                continue;
            }
            implementations.push((candidate, sig, inferred));
        }
        if implementations.len() != 1 {
            return Err(Issue::alternatives(
                errors,
                format!(
                    "{module}::{name}: expected one instantiated implementation, found {}",
                    implementations.len()
                ),
            ));
        }
        Ok(implementations.remove(0))
    }
    fn requirement(
        &mut self,
        module: &str,
        sig: &Signature,
        inferred: &BTreeMap<String, Ty>,
    ) -> bool {
        if let Some((required, args, target)) = &sig.requirement {
            let args = args
                .iter()
                .map(|ty| {
                    Ty::parse(ty)
                        .or_else(|| inferred.get(ty).cloned())
                        .map(|ty| (None, Value::new(ty, Kind::Literal(Literal::Int(0)))))
                })
                .collect::<Option<Vec<_>>>();
            let Some(args) = args else {
                return false;
            };
            let (owner, item) =
                hgl_semantics::value_types::identity(&self.library, module, required);
            let Ok((id, _, _, _, provided)) = self.select(&owner, &item, &args, true) else {
                return false;
            };
            if self.library.declarations[id].role != Role::Native
                || Ty::parse(target).or_else(|| inferred.get(target).cloned()) != Some(provided)
            {
                return false;
            }
        }
        true
    }
    fn wiring_value(&mut self, value: &Value) -> Result<Value, Issue> {
        match self.wiring.value(value) {
            Ok(value) => Ok(value),
            Err(hgl_semantics::value_eval::EvalError::Operation(message)) => {
                self.plan.construction_error.get_or_insert(message.clone());
                Ok(Value::new(value.ty.clone(), Kind::WiringFailure(message)))
            }
            Err(hgl_semantics::value_eval::EvalError::Unsupported(message)) => Err(message.into()),
            Err(hgl_semantics::value_eval::EvalError::ContextRequired) => {
                Err("contextual value requires run preparation".into())
            }
        }
    }
    fn wiring_statement(&mut self, statement: &Statement) -> Result<(), Issue> {
        match self.wiring.statement(statement) {
            Ok(_) => Ok(()),
            Err(hgl_semantics::value_eval::EvalError::Operation(message)) => {
                self.plan.construction_error.get_or_insert(message);
                Ok(())
            }
            Err(hgl_semantics::value_eval::EvalError::Unsupported(message)) => Err(message.into()),
            Err(hgl_semantics::value_eval::EvalError::ContextRequired) => {
                Err("contextual value requires run preparation".into())
            }
        }
    }
    fn wiring_local(
        &mut self,
        name: &str,
        value: Value,
        mutable: bool,
        annotated: bool,
        env: &mut Env,
    ) -> Result<(), Issue> {
        if env.contains_key(name) {
            return Err(format!("duplicate local {name}").into());
        }
        if matches!(value.kind, Kind::Wire(_))
            || (!mutable && matches!(value.kind, Kind::Prepared(_)))
        {
            env.insert(name.into(), value);
            return Ok(());
        }
        let id = self.next_wiring_local;
        self.next_wiring_local += 1;
        env.insert(
            name.into(),
            hgl_semantics::value_check::binding(id, &value, mutable, annotated)?,
        );
        let value = self.wiring_value(&value)?;
        if matches!(value.kind, Kind::WiringFailure(_)) {
            self.wiring.bind_failed(id, value, mutable);
            return Ok(());
        }
        self.wiring_statement(
            &hgl_semantics::local_check::statement(id, value, mutable, annotated)?.1,
        )
    }
    fn graph(
        &mut self,
        module: &str,
        cursor: &mut Cursor<'_>,
        env: &mut Env,
        (result, signature): (&Ty, &Signature),
    ) -> Result<Value, Issue> {
        let locals = hgl_semantics::local_check::GraphLocals::new(
            signature
                .parameters
                .iter()
                .filter(|p| p.ty == "signal")
                .map(|p| p.name.clone()),
        );
        let previous_locals = std::mem::replace(&mut self.graph_locals, locals);
        let previous_hint = self.result_hint.replace(result.clone());
        let statements = hgl_semantics::body_check::composition_body(cursor)?;
        let mut output = Value::new(Ty::Void, Kind::Void);
        let mut mutable_ports = BTreeSet::new();
        for (position, statement) in statements.iter().enumerate() {
            match statement {
                Stmt::Let(name, annotation, expr) | Stmt::Var(name, annotation, expr) => {
                    let value =
                        self.local_initializer(module, annotation.as_deref(), expr, env, false)?;
                    if matches!(statement, Stmt::Var(..)) && matches!(value.kind, Kind::Wire(_)) {
                        mutable_ports.insert(name.clone());
                    }
                    self.graph_locals.bind(name, expr);
                    self.wiring_local(
                        name,
                        value,
                        matches!(statement, Stmt::Var(..)),
                        annotation.is_some(),
                        env,
                    )?;
                }
                Stmt::Call(expr) | Stmt::Return(expr) => {
                    if matches!(statement, Stmt::Return(_)) && position + 1 != statements.len() {
                        return Err("statements after return".into());
                    }
                    output = self.expression(module, expr, env, false)?;
                    if !matches!(output.kind, Kind::Wire(_) | Kind::Void) {
                        self.wiring_value(&output)?;
                    }
                }
                Stmt::Assign(Expr::Name(name), expr) | Stmt::Add(Expr::Name(name), expr) => {
                    self.graph_assignment(
                        module,
                        (name, expr),
                        env,
                        mutable_ports.contains(name),
                        matches!(statement, Stmt::Add(..)),
                    )?;
                }
                Stmt::If(..) if hgl_semantics::wiring_locals::handles(statement, env) => {
                    hgl_semantics::wiring_locals::conditional(
                        statement,
                        env,
                        &mutable_ports,
                        (&self.graph_locals, &self.static_values),
                        (&mut self.plan, &mut self.wiring),
                        &mut self.next_wiring_local,
                    )?;
                }
                Stmt::Add(..) | Stmt::Assign(..) | Stmt::If(..) => {
                    let mut next = self.next_wiring_local;
                    let checked = self.statement(module, statement, env, &Ty::Void, &mut next)?;
                    self.next_wiring_local = next;
                    self.wiring_statement(&checked)?;
                }
                Stmt::While(..) => {
                    return Err("while requires a runtime body, not composition".into());
                }
                Stmt::TimedYield(..) => return Err("yield requires a generator source".into()),
                Stmt::Exit | Stmt::For(..) | Stmt::ForItems(..) => {
                    return Err("runtime statement in graph".into());
                }
            }
        }
        self.graph_locals = previous_locals;
        self.result_hint = previous_hint;
        Ok(hgl_semantics::local_check::graph_result(output, result)?)
    }
    fn graph_assignment(
        &mut self,
        module: &str,
        (name, expr): (&str, &Expr),
        env: &mut Env,
        mutable_port: bool,
        compound: bool,
    ) -> Result<(), Issue> {
        self.graph_locals.replacement(name, expr)?;
        let target = env
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown value {name}"))?;
        let value = if compound {
            self.binary(module, "+", (&Expr::Name(name.into()), expr), env, false)?
        } else {
            self.expected_expression(module, expr, env, false, Some(&target.ty))?
        };
        let value = hgl_semantics::local_check::replacement(&target, value, mutable_port)?;
        if matches!(target.kind, Kind::Wire(_)) {
            env.insert(name.into(), value);
            Ok(())
        } else {
            self.wiring_statement(&hgl_semantics::local_check::assignment(target, value)?)
        }
    }
    fn document(&mut self, id: usize) {
        if self.documented.insert(id) {
            let decl = &self.library.declarations[id];
            if !decl.doc.is_empty() {
                self.plan.docs.push(format!(
                    "{}::{}\n{}",
                    decl.module,
                    decl.name,
                    crate::documentation::normalize(&decl.doc)
                ));
            }
        }
    }
    fn node(
        &mut self,
        decl: &Decl,
        signature: &Signature,
        c: &mut Cursor<'_>,
        env: &mut Env,
        result: &Ty,
    ) -> Result<Node, Issue> {
        let generator = signature.body.iter().any(|token| token.text == "yield");
        if generator
            && signature
                .parameters
                .iter()
                .any(|parameter| !parameter.constant)
        {
            return Err("generator sources cannot have temporal parameters".into());
        }
        let mut node = hgl_semantics::value_check::prepare_node(
            signature,
            env,
            format!("{}::{}", decl.module, decl.name),
            result.clone(),
        )?;
        self.static_values.configuration = node.configuration.clone();
        if generator {
            return self.generator_body(decl, c, env, node);
        }
        if signature.value_function {
            let body = if c.take("=>") {
                vec![Stmt::Return(c.expr()?)]
            } else {
                c.block()?
            };
            node.handlers.push((
                None,
                self.statements(&decl.module, &body, env, result, &mut 0)?,
            ));
            return Ok(node);
        }
        c.need("{")?;
        c.lines();
        self.initializers(decl, c, env, &mut node)?;
        self.injectables(decl, c, env, &mut node)?;
        let mut next_local = 0;
        self.hooks(decl, c, env, &mut node, &mut next_local)?;
        if node.handlers.is_empty() {
            return Err("expected when handler".into());
        }
        node.globals = std::mem::take(&mut self.globals);
        self.failed_globals.clear();
        hgl_semantics::value_check::validate(&node)?;
        self.facts.clear();
        self.runtime_node = false;
        c.need("}")?;
        c.lines();
        if !c.at("") {
            return Err("unsupported node body suffix".into());
        }
        Ok(node)
    }
    fn generator_body(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &mut Env,
        mut node: Node,
    ) -> Result<Node, Issue> {
        if !node.result.publication() {
            return Err("generator requires a declared publication output type".into());
        }
        let (services, body) = hgl_semantics::body_check::generator_body(c)?;
        for service in services {
            hgl_semantics::endpoint_check::inject_capability(&service, &mut node, env)?;
        }
        self.generator = true;
        self.phase = Phase::Evaluation;
        self.facts.clear();
        node.generator = Some(self.statements(&decl.module, &body, env, &node.result, &mut 0)?);
        self.generator = false;
        self.runtime_node = false;
        self.facts.clear();
        node.alarm = true;
        hgl_semantics::value_check::validate(&node)?;
        Ok(node)
    }
    fn hooks(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &Env,
        node: &mut Node,
        next_local: &mut usize,
    ) -> Result<(), Issue> {
        let mut seen = BTreeSet::new();
        while !c.at("}") {
            if c.at("when") {
                self.phase = Phase::Evaluation;
                self.handlers(decl, c, env, node, next_local)?;
            } else {
                let hook = c.consume()?;
                self.phase = match hook.as_str() {
                    "start" => Phase::Start,
                    "stop" => Phase::Stop,
                    _ => return Err("expected start, when or stop hook".into()),
                };
                if !seen.insert(hook.clone()) {
                    return Err(format!("duplicate {hook} hook").into());
                }
                self.facts.clear();
                let body = self.statements(
                    &decl.module,
                    &c.block()?,
                    &mut env.clone(),
                    &Ty::Void,
                    next_local,
                )?;
                if self.phase == Phase::Start {
                    node.start = body;
                } else {
                    node.stop = body;
                }
                c.lines();
            }
        }
        self.phase = Phase::Evaluation;
        Ok(())
    }
    fn handlers(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &Env,
        node: &mut Node,
        next_local: &mut usize,
    ) -> Result<(), Issue> {
        while c.take("when") {
            self.facts.clear();
            if c.at("scheduled") {
                return Err(
                    "expected { or a supported endpoint guard; scheduled() is not an alarm guard"
                        .into(),
                );
            }
            let guard = if c.at("{") {
                None
            } else {
                let expr = hgl_semantics::flow_check::node_guard(node, c.expr()?);
                let condition = self.expression(&decl.module, &expr, env, true)?;
                if condition.ty != Ty::Bool {
                    return Err("handler guard requires bool".into());
                }
                Some(condition)
            };
            self.facts = handler_facts(guard.as_ref(), node.inputs.len());
            let body = c.block()?;
            node.handlers.push((
                guard,
                self.statements(
                    &decl.module,
                    &body,
                    &mut env.clone(),
                    &node.result,
                    next_local,
                )?,
            ));
            c.lines();
        }
        Ok(())
    }
    fn injectables(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &mut Env,
        node: &mut Node,
    ) -> Result<(), Issue> {
        let result = &node.result.clone();
        while c.take("inject") {
            loop {
                match c.name()?.as_str() {
                    name @ ("alarm" | "clock" | "logger" | "global_state") => {
                        hgl_semantics::endpoint_check::inject_capability(name, node, env)?;
                    }
                    "out" => {
                        env.insert("out".into(), Value::new(result.clone(), Kind::Output));
                    }
                    name => return Err(format!("unsupported injectable {name}").into()),
                }
                if !c.take(",") {
                    break;
                }
            }
            c.lines();
        }
        if node.alarm && !node.inputs.is_empty() {
            return Err("alarm is admitted only on sources".into());
        }
        self.initializers(decl, c, env, node)?;
        Ok(())
    }
    fn initializers(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &mut Env,
        node: &mut Node,
    ) -> Result<(), Issue> {
        while c.take("cache") || c.take("state") {
            let name = c.name()?;
            c.need(":")?;
            let ty = Ty::parse(&c.name()?).ok_or("unsupported state type")?;
            c.need("=")?;
            let value = self.expression(&decl.module, &c.expr()?, env, false)?;
            let Kind::Literal(initial) = value.kind else {
                return Err("cache initializer must be fixed i64".into());
            };
            let index = node.caches.len();
            if initial.ty() != ty {
                return Err("state initializer type mismatch".into());
            }
            node.caches.push(initial);
            if env
                .insert(name, Value::new(ty, Kind::Cache(index)))
                .is_some()
            {
                return Err("duplicate cache name".into());
            }
            c.lines();
        }
        Ok(())
    }
    fn statements(
        &mut self,
        module: &str,
        statements: &[Stmt],
        env: &mut Env,
        result: &Ty,
        next_local: &mut usize,
    ) -> Result<Vec<Statement>, Issue> {
        let mut bindings = BTreeSet::new();
        let mut checked = Vec::new();
        for statement in statements {
            if let Stmt::Let(name, _, _) | Stmt::Var(name, _, _) = statement
                && !bindings.insert(name)
            {
                return Err(format!("duplicate local {name}").into());
            }
            checked.push(self.statement(module, statement, env, result, next_local)?);
        }
        Ok(checked)
    }
    fn local_initializer(
        &mut self,
        module: &str,
        annotation: Option<&str>,
        expr: &Expr,
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        let mut ty = annotation
            .map(|name| {
                let name = self.type_sizes(module, name, env)?;
                resolve_type(&name, &self.types).map_or_else(
                    || {
                        hgl_semantics::value_types::substitute(
                            &self.library,
                            module,
                            &name,
                            &self.types,
                            &mut BTreeSet::new(),
                        )
                    },
                    Ok,
                )
            })
            .transpose()?;
        let hint = ty.as_ref().map(|ty| {
            if matches!(ty, Ty::Atomic(_)) {
                ty.clone()
            } else {
                hgl_semantics::value_access::project(ty)
            }
        });
        let previous = self.requirements.delta;
        self.requirements.delta =
            annotation.is_some_and(|name| hgl_source::delta_argument(name).is_some());
        let mut value = self.expected_expression(module, expr, env, runtime, hint.as_ref())?;
        value.delta_required = self.requirements.delta;
        self.requirements.delta = previous;
        if !matches!(value.kind, Kind::Wire(_)) {
            ty = hint;
        }
        Ok(hgl_semantics::local_check::initializer(value, ty.as_ref())?)
    }
    fn struct_declaration(&self, module: &str, name: &str) -> Result<Option<&Decl>, Issue> {
        Ok(hgl_semantics::value_types::declaration(
            &self.library,
            module,
            name,
        )?)
    }
    fn global_operation(&mut self, operation: &str, args: &Arguments) -> Result<Value, Issue> {
        if operation == "get" {
            return Err("global_state: get requires a concrete scalar expected type or a typed ordinary struct binding".into());
        }
        if operation != "set" {
            return Err(format!("global_state: unknown capability operation {operation}").into());
        }
        let ordered = order_arguments(&["key", "value"], args)?;
        let key = ordered.get(&0).ok_or("global_state: missing key")?;
        let value = ordered.get(&1).ok_or("global_state: missing value")?;
        if key.ty != Ty::Str || !ordinary(&value.ty) || matches!(value.kind, Kind::Input(_, true)) {
            return Err("global_state: set requires a str key and an ordinary scalar value or required-field struct".into());
        }
        let index = self.global_entry(key, &value.ty)?;
        Ok(Value::new(
            Ty::Void,
            Kind::GlobalSet(index, Box::new(value.clone())),
        ))
    }
    fn global_entry(&mut self, key: &Value, ty: &Ty) -> Result<usize, Issue> {
        let resolved = self.static_values.resolve(key).clone();
        let key = &resolved;
        if matches!(key.kind, Kind::WiringFailure(_)) {
            let index = self.globals.len();
            self.globals
                .push((format!("<failed configuration {index}>"), ty.clone()));
            self.failed_globals.insert(index);
            return Ok(index);
        }
        let Kind::Literal(Literal::Str(key)) = &key.kind else {
            return Err("global_state: key must resolve to a literal str or const str parameter; general const key expressions and runtime keys are unsupported".into());
        };
        if let Some(previous) = self.global_types.get(key) {
            if previous != ty {
                return Err(format!("global_state: type conflict for key {key:?}").into());
            }
        } else {
            self.global_types.insert(key.clone(), ty.clone());
        }
        if let Some(index) = self
            .globals
            .iter()
            .enumerate()
            .position(|(index, (name, _))| name == key && !self.failed_globals.contains(&index))
        {
            return Ok(index);
        }
        let index = self.globals.len();
        self.globals.push((key.clone(), ty.clone()));
        Ok(index)
    }
    pub(crate) fn harness_expression(
        &mut self,
        module: &str,
        expr: &Expr,
        expected: &Ty,
    ) -> Result<Value, Issue> {
        if let Expr::Tuple(cells) = expr.syntax()
            && matches!(expected, Ty::Delta(_))
        {
            return hgl_semantics::delta_check::tuple_shorthand(cells, expected, |expr, ty| {
                self.harness_expression(module, expr, ty)
            });
        }
        self.expected_expression(
            module,
            expr,
            &self.harness_values.clone(),
            false,
            Some(expected),
        )
    }

    fn expected_expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, Issue> {
        let value = self
            .expected_inner(module, expr, env, runtime, expected)
            .map_err(|mut issue| {
                if self.requirements.delta && issue.code.is_none() && issue.category == "type" {
                    issue.code = Some("delta.type_mismatch");
                }
                issue.at(expr.span())
            })?;
        let value = hgl_semantics::family_values::coerce(expected, value).map_err(|message| {
            if self.requirements.delta {
                Issue::coded("type", "delta.type_mismatch", expr.span(), message)
            } else {
                message.into()
            }
        })?;
        if self.requirements.delta
            && expected
                .is_some_and(|ty| value.ty != *ty && !(ty == &Ty::F64 && value.ty == Ty::I64))
        {
            return Err(Issue::coded(
                "type",
                "delta.type_mismatch",
                expr.span(),
                "delta required expression type mismatch",
            ));
        }
        Ok(value)
    }
    fn expected_inner(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, Issue> {
        if let Expr::Applied(name, args) | Expr::Call(name, args) = expr.syntax()
            && (matches!(expr.syntax(), Expr::Applied(..))
                || self.struct_declaration(module, name)?.is_some())
        {
            return self.constructor(module, (name, args), env, runtime, expected);
        }
        if matches!(expr.syntax(), Expr::Sequence(_) | Expr::Tuple(_)) {
            return hgl_semantics::value_check::aggregate_checked(
                expr,
                expected,
                matches!(
                    self.value_context,
                    ValueContext::Constant | ValueContext::Preparation
                ) || !runtime
                    || self.requirements.complete,
                |expr, expected| self.expected_expression(module, expr, env, runtime, expected),
            );
        }
        if let Expr::Call(name, args) = expr.syntax()
            && name == "get"
            && args.first().is_some_and(|(name, expression)| name.is_none() && matches!(expression.syntax(), Expr::Name(receiver) if receiver == "global_state" && env.get(receiver).is_some_and(|v| matches!(v.kind, Kind::Capability))))
        {
            if !runtime {
                return Err("global_state: requires a runtime hook".into());
            }
            capability_payload("global_state", env)?;
            let ty = expected
                .filter(|ty| ordinary(ty))
                .ok_or("global_state: get requires a concrete scalar expected type or a typed ordinary struct binding")?;
            let values = args[1..]
                .iter()
                .map(|(name, expr)| {
                    Ok((name.clone(), self.expression(module, expr, env, runtime)?))
                })
                .collect::<Result<Vec<_>, Issue>>()?;
            let values = method_arguments("get(global_state)", &[("key", Ty::Str)], &values)?;
            let index = self.global_entry(&values[0], ty)?;
            return Ok(Value::new(ty.clone(), Kind::GlobalGet(index)));
        }
        self.expression(module, expr, env, runtime)
    }
    fn statement(
        &mut self,
        module: &str,
        statement: &Stmt,
        env: &mut Env,
        result: &Ty,
        next_local: &mut usize,
    ) -> Result<Statement, Issue> {
        Ok(match statement {
            Stmt::Let(name, annotation, expr) | Stmt::Var(name, annotation, expr) => {
                let value =
                    self.local_initializer(module, annotation.as_deref(), expr, env, true)?;
                let mutable = matches!(statement, Stmt::Var(..));
                let id = *next_local;
                *next_local += 1;
                self.static_values.bind(id, &value, mutable);
                let (binding, statement) = hgl_semantics::local_check::statement(
                    id,
                    value,
                    mutable,
                    annotation.is_some(),
                )?;
                env.insert(name.clone(), binding);
                statement
            }
            Stmt::Exit => {
                if self.phase != Phase::Evaluation || !self.runtime_node {
                    return Err("bare return requires runtime evaluation".into());
                }
                Statement::Exit
            }
            Stmt::Return(expr) => self.return_statement(module, expr, env, result)?,
            Stmt::Call(expr) => {
                let value = self.expression(module, expr, env, true)?;
                require_payload(&value)?;
                Statement::Call(value)
            }
            Stmt::TimedYield(time, payload, span) => {
                self.timed_yield(module, (time, payload, span.clone()), env, result)?
            }
            Stmt::While(expr, body) => {
                self.while_statement(module, (expr, body), env, result, next_local)?
            }
            Stmt::ForItems(name, child_name, collection, body) => {
                let (key_id, child_id, modified, collection, mut scope) =
                    hgl_semantics::collection_check::items_scope(
                        collection,
                        (name, child_name),
                        env,
                        next_local,
                        |expr| self.expression(module, expr, env, true),
                    )?;
                let body = self.statements(module, body, &mut scope, result, next_local)?;
                Statement::ForItems(key_id, child_id, modified, collection, body)
            }
            Stmt::For(name, collection, body) => {
                if !matches!(collection.syntax(), Expr::Call(name, _) if name == "elements") {
                    return Err("for currently requires elements(input, added)".into());
                }
                let collection = self.expression(module, collection, env, true)?;
                let Ty::Set(element) = &collection.ty else {
                    return Err("elements requires a set".into());
                };
                let id = *next_local;
                *next_local += 1;
                let mut scope = env.clone();
                scope.insert(name.clone(), Value::new(*element.clone(), Kind::Local(id)));
                let previous = self.facts.clone();
                let body = self.statements(module, body, &mut scope, result, next_local)?;
                self.facts = previous;
                Statement::For(id, collection, body)
            }
            Stmt::Assign(target, expr) => self.assignment(module, target, expr, env)?,
            Stmt::Add(target, expr) => self.increment(module, target, expr, env)?,
            Stmt::If(expr, yes, no) => {
                self.conditional(module, (expr, yes, no), env, result, next_local)?
            }
        })
    }
    fn return_statement(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        result: &Ty,
    ) -> Result<Statement, Issue> {
        if self.generator && self.value_context == ValueContext::Outside {
            return Err("generator return cannot carry a value; use yield".into());
        }
        let expected =
            if self.value_context == ValueContext::Outside && !matches!(result, Ty::Ref(_)) {
                result.clone().delta()?
            } else {
                result.clone()
            };
        let previous_complete = self.requirements.complete;
        self.requirements.complete = matches!(result, Ty::Atomic(_));
        let previous = self.requirements.delta;
        self.requirements.delta = self.requirements.result || matches!(expected, Ty::Delta(_));
        let v = self.expected_expression(module, expr, env, true, Some(&expected))?;
        self.requirements.delta = previous;
        self.requirements.complete = previous_complete;
        if v.ty != expected || *result == Ty::Void {
            return Err("node return type mismatch".into());
        }
        require_payload(&v)?;
        Ok(if self.value_context == ValueContext::Outside {
            Statement::Return(v)
        } else {
            Statement::Yield(v)
        })
    }
    fn timed_yield(
        &mut self,
        module: &str,
        (time, payload, span): (&Expr, &Expr, std::ops::Range<usize>),
        env: &Env,
        result: &Ty,
    ) -> Result<Statement, Issue> {
        if !self.generator || self.value_context != ValueContext::Outside {
            return Err("yield requires a generator source".into());
        }
        let time = self.expression(module, time, env, true)?;
        hgl_semantics::source_check::yield_time(&time.ty, span)?;
        let expected = result.clone().delta()?;
        let payload = self.expected_expression(module, payload, env, true, Some(&expected))?;
        require_payload(&payload)?;
        if payload.ty != expected {
            return Err("yield payload type mismatch".into());
        }
        Ok(Statement::TimedYield(time, payload))
    }
    fn while_statement(
        &mut self,
        module: &str,
        (expr, body): (&Expr, &[Stmt]),
        env: &Env,
        result: &Ty,
        next_local: &mut usize,
    ) -> Result<Statement, Issue> {
        if !self.runtime_node || self.value_context != ValueContext::Outside {
            return Err(
                "while requires a runtime body, not composition or const evaluation".into(),
            );
        }
        let condition = self.expression(module, expr, env, true)?;
        if condition.ty != Ty::Bool {
            return Err("while condition requires bool".into());
        }
        require_payload(&condition)?;
        let previous = self.facts.clone();
        self.facts = facts(&condition, true, &self.facts);
        let body = self.statements(module, body, &mut env.clone(), result, next_local)?;
        self.facts = previous;
        Ok(Statement::While(condition, body))
    }
    fn assignment_target(
        &mut self,
        module: &str,
        target: &Expr,
        env: &Env,
    ) -> Result<Value, Issue> {
        if let Expr::Property(parent, _) = target.syntax()
            && injected_clock(parent, env)
        {
            return Err("clock properties are read-only".into());
        }
        if !matches!(target.syntax(), Expr::Name(_) | Expr::Property(..)) {
            return Err("assignment requires a writable variable or struct field; indexed replacement is not admitted".into());
        }
        self.expression(module, target, env, true)
    }
    fn increment(
        &mut self,
        module: &str,
        target: &Expr,
        expr: &Expr,
        env: &Env,
    ) -> Result<Statement, Issue> {
        let binding = self.assignment_target(module, target, env)?;
        if matches!(binding.kind, Kind::Cache(_)) {
            let value = self.expression(module, expr, env, true)?;
            if binding.ty != Ty::I64 || value.ty != Ty::I64 {
                return Err("cache increment requires i64 target and value".into());
            }
            let sum = Value::new(
                Ty::I64,
                Kind::Binary("+".into(), Box::new(binding.clone()), Box::new(value)),
            );
            return Ok(Statement::Assign(binding, sum));
        }
        if !writable(&binding) {
            return Err("increment requires a cache variable or writable var".into());
        }
        let sum = Expr::Binary("+".into(), Box::new(target.clone()), Box::new(expr.clone()));
        self.assignment(module, target, &sum, env)
    }
    fn assignment(
        &mut self,
        module: &str,
        target: &Expr,
        expr: &Expr,
        env: &Env,
    ) -> Result<Statement, Issue> {
        let target = self.assignment_target(module, target, env)?;
        let expected =
            hgl_semantics::local_check::assignment_type(&target, self.phase == Phase::Evaluation)?;
        let previous = self.requirements.delta;
        self.requirements.delta = hgl_semantics::struct_check::delta_place(&self.library, &target);
        let value = self.expected_expression(module, expr, env, true, Some(&expected))?;
        self.requirements.delta = previous;
        Ok(hgl_semantics::local_check::assignment(target, value)?)
    }
    fn conditional(
        &mut self,
        module: &str,
        (expr, yes, no): (&Expr, &[Stmt], &[Stmt]),
        env: &Env,
        result: &Ty,
        next_local: &mut usize,
    ) -> Result<Statement, Issue> {
        let condition = self.expected_expression(module, expr, env, true, Some(&Ty::Bool))?;
        if condition.ty != Ty::Bool {
            return Err("condition requires bool".into());
        }
        if matches!(condition.kind, Kind::Wire(_)) {
            return Err("temporal graph conditionals are unsupported by this backend".into());
        }
        require_payload(&condition)?;
        let previous = self.facts.clone();
        self.facts = facts(&condition, true, &self.facts);
        let yes = self.statements(module, yes, &mut env.clone(), result, next_local)?;
        let yes_facts = self.facts.clone();
        self.facts = previous;
        self.facts = facts(&condition, false, &self.facts);
        let no = self.statements(module, no, &mut env.clone(), result, next_local)?;
        self.facts = match (terminates(&yes), terminates(&no)) {
            (true, _) => self.facts.clone(),
            (false, true) => yes_facts,
            (false, false) => merge_facts(&yes_facts, &self.facts),
        };
        Ok(Statement::If(condition, yes, no))
    }
    fn expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        match expr.syntax() {
        Expr::Located(..) => unreachable!("syntax strips source origins"),
            Expr::Lambda(parameters, result, body) => {
                let mut env = env.clone();
                for (index, (name, ty)) in parameters.iter().enumerate() { env.insert(name.clone(), Value::new(hgl_semantics::value_types::concrete(&self.library, module, ty, &self.types)?, Kind::Local(index))); }
                self.requirements.delta = hgl_source::delta_argument(result).is_some();
                let ty = hgl_semantics::value_types::concrete(&self.library, module, result, &self.types)?;
                self.expected_expression(module, body, &env, false, Some(&ty))?;
                Err("ordinary callable lowering is outside this backend profile".into())
            }
            Expr::Tuple(_) => self.expected_expression(module, expr, env, runtime, None),
            Expr::Sparse(_) => Err("sparse entries require a delta constructor context".into()),
            Expr::Null => Err("null requires a contextual nullable comparison".into()),
            Expr::Property(receiver, name) => {
                if injected_clock(receiver, env) {
                    return Ok(clock_property(receiver, name, env, runtime)?);
                }
                let parent = self.expression(module, receiver, env, runtime)?;
                Ok(hgl_semantics::struct_check::field(&self.library, parent, name)?)
            }
            Expr::Index(receiver, index) => {
                let parent = self.expression(module, receiver, env, runtime)?;
                let index = self.expression(module, index, env, runtime)?;
                Ok(hgl_semantics::value_check::indexed(parent, index)?)
            }
            Expr::Sequence(elements) => Ok(hgl_semantics::value_check::list_literal(elements, None)?),
            Expr::Literal(l) => Ok(Value::new(l.ty(), Kind::Literal(l.clone()))),
            Expr::TemporalLiteral(l) if self.value_context == ValueContext::Preparation && !self.preparing_graph => Ok(Value::new(l.ty(), Kind::TemporalLiteral(l.clone()))),
            Expr::TemporalLiteral(_) => Err("contextual temporal construction requires run preparation; node-hook construction is unsupported".into()),
            Expr::Name(name) => {
                if !env.contains_key(name) && let Some(value) = hgl_semantics::enums::member(&self.library, module, name)? { return Ok(Value::new(value.ty(), Kind::Literal(value))); }
                let value = env
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("unknown value {name}"))?;
                if matches!(value.kind, Kind::Void | Kind::Capability) {
                    return Err(format!("{name}: injectable cannot escape as a value").into());
                }
                if self.phase != Phase::Evaluation
                    && matches!(value.kind, Kind::Input(..) | Kind::Output)
                {
                    let phase = if self.phase == Phase::Stop {
                        "stop"
                    } else {
                        "start"
                    };
                    return Err(format!("{phase} cannot access temporal endpoints").into());
                }
                if let (Ty::Nullable(payload), Kind::Local(id)) = (&value.ty, &value.kind)
                    && self.facts.contains(&("present".into(), *id))
                {
                    return Ok(Value::new(*payload.clone(), Kind::Present(Box::new(value))));
                }
                Ok(value)
            }
            Expr::Unary(op, expr) => {
                let value = self.expression(module, expr, env, runtime)?;
                require_payload(&value)?;
                if (op == "!" && value.ty != Ty::Bool)
                    || (op == "-" && !matches!(value.ty, Ty::I64 | Ty::F64 | Ty::Duration))
                {
                    return Err("unary type mismatch".into());
                }
                Ok(Value::new(
                    value.ty.clone(),
                    Kind::Unary(op.clone(), Box::new(value)),
                ))
            }
            Expr::Applied(name, args) => self.constructor(module, (name, args), env, runtime, None),
            Expr::Call(name, args) => self.call_expression(module, name, args, env, runtime),
            Expr::Binary(op, a, b) => self.binary(module, op, (a, b), env, runtime),
        }
    }
    fn call_expression(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        hgl_semantics::enums::check_call(&self.library, module, name, args)?;
        if let Some((label, expression)) = args.first()
            && let Expr::Name(receiver) = expression.syntax()
            && env
                .get(receiver)
                .is_some_and(|v| matches!(v.kind, Kind::Capability))
        {
            if label.is_some() || !runtime {
                return Err(format!(
                    "{receiver}: capability requires a first positional runtime operand"
                )
                .into());
            }
            let values = hgl_semantics::endpoint_check::arguments(&args[1..], |expr| {
                self.expression(module, expr, env, runtime)
            })?;
            if receiver == "global_state" {
                return self.global_operation(name, &values);
            }
            if receiver == "logger" && name == "info" {
                return self.log_info(&values);
            }
            return Ok(capability_function(
                receiver,
                name,
                &values,
                env,
                self.phase == Phase::Stop,
            )?);
        }
        if matches!(
            name,
            "insert" | "update" | "remove" | "invalidate" | "push" | "pop" | "contains"
        ) && let Some((_, first)) = args.first()
        {
            let receiver = self.expression(module, first, env, runtime)?;
            if matches!(receiver.ty, Ty::Map(..) | Ty::List(..))
                && (matches!(receiver.kind, Kind::Output) || name == "contains")
            {
                if !runtime
                    || !self.runtime_node
                    || self.phase != Phase::Evaluation
                    || args.iter().any(|(label, _)| label.is_some())
                {
                    return Err(
                        "collection operation requires positional evaluation operands".into(),
                    );
                }
                let mut values = vec![receiver];
                for (_, expr) in &args[1..] {
                    values.push(self.expression(module, expr, env, runtime)?);
                }
                return Ok(hgl_semantics::collection_check::operation(name, values)?);
            }
        }
        self.value_expression(module, name, args, env, runtime)
    }
    fn value_expression(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        if name == "delta_value" {
            return self.delta_value(module, args, env, runtime);
        }
        if name == "elements" {
            if args.len() != 2 || !matches!(args[1].1.syntax(),Expr::Name(n) if n=="added") {
                return Err("elements currently requires added".into());
            }
            let collection = self.expression(module, &args[0].1, env, runtime)?;
            if !matches!(collection.ty, Ty::Set(_)) || !matches!(collection.kind, Kind::Input(..)) {
                return Err("elements requires a set input".into());
            }
            return Ok(collection);
        }
        if matches!(name, "len" | "push") && !args.is_empty() {
            let receiver = self.expression(module, &args[0].1, env, runtime)?;
            if let Ty::List(element, _) = &receiver.ty {
                if args.iter().any(|(label, _)| label.is_some()) {
                    return Err("ordinary list operations require positional arguments".into());
                }
                let mut values = vec![receiver.clone()];
                for (_, expr) in &args[1..] {
                    values.push(self.expected_expression(
                        module,
                        expr,
                        env,
                        runtime,
                        Some(element),
                    )?);
                }
                return Ok(hgl_semantics::value_check::list_operation(name, &values)?);
            }
        }
        if self.struct_declaration(module, name)?.is_some() {
            return self.constructor(module, (name, args), env, runtime, None);
        }
        let args = args
            .iter()
            .enumerate()
            .map(|(index, (n, v))| {
                let hint = hgl_semantics::source_check::argument_hint(
                    &self.library,
                    module,
                    name,
                    index,
                    n.as_deref(),
                );
                let previous = self.requirements.delta;
                self.requirements.delta = hint.as_ref().is_some_and(|(_, delta)| *delta);
                let value = self.expected_expression(
                    module,
                    v,
                    env,
                    runtime,
                    hint.as_ref().map(|(ty, _)| ty),
                )?;
                self.requirements.delta = previous;
                if !endpoint_metadata(name) {
                    require_payload(&value)?;
                }
                Ok((n.clone(), value))
            })
            .collect::<Result<Vec<_>, Issue>>()?;
        for (_, value) in &args {
            hgl_semantics::value_check::helper_argument(value)?;
        }
        self.value_call(module, name, args, runtime)
    }
    fn delta_value(
        &mut self,
        module: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        if !runtime
            || !self.runtime_node
            || self.phase != Phase::Evaluation
            || args.len() != 1
            || args[0].0.is_some()
        {
            return Err("delta_value requires one runtime input endpoint in evaluation".into());
        }
        let value = self.expression(module, &args[0].1, env, runtime)?;
        Ok(hgl_semantics::delta_check::delta_value(value, &self.facts)?)
    }

    fn constructor(
        &mut self,
        module: &str,
        (name, args): (&str, &[(Option<String>, Expr)]),
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, Issue> {
        let name = self.type_sizes(module, name, env)?;
        if let Some(origin) = name
            .strip_prefix("delta<")
            .and_then(|name| name.strip_suffix('>'))
        {
            return self.delta_constructor(module, (origin, args), env, runtime, expected);
        }
        if hgl_source::application(&name).is_some_and(|(base, _)| matches!(base, "set" | "map")) {
            let ty =
                hgl_semantics::value_types::concrete(&self.library, module, &name, &self.types)?;
            return hgl_semantics::collection_check::constructor_checked(&ty, args, |expr, ty| {
                self.expected_expression(module, expr, env, runtime, Some(ty))
            });
        }
        let mut check = hgl_semantics::struct_check::Constructor::new(
            &self.library,
            module,
            &name,
            args,
            expected,
            &self.types,
        )?;
        while let Some((index, hint)) = check.next(&self.library, args)? {
            let previous = self.requirements.delta;
            self.requirements.delta |= check.delta_required(index);
            let value =
                self.expected_expression(module, &args[index].1, env, runtime, hint.as_ref())?;
            self.requirements.delta = previous;
            require_payload(&value)?;
            check.checked(&self.library, index, value)?;
        }
        let value = check.finish(&self.library)?;
        if (self.value_context != ValueContext::Preparation || self.preparing_graph)
            && let Kind::Construct(fields) = &value.kind
            && fields
                .iter()
                .any(|(_, v)| matches!(v.kind, Kind::TemporalLiteral(_)))
        {
            return Err("contextual temporal default requires run preparation; node-hook construction is unsupported".into());
        }
        Ok(value)
    }
    fn delta_constructor(
        &mut self,
        module: &str,
        (origin, args): (&str, &[(Option<String>, Expr)]),
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, Issue> {
        let origin =
            hgl_semantics::value_types::concrete(&self.library, module, origin, &self.types)?;
        let ty = origin.clone().delta()?;
        let parts = hgl_semantics::delta_check::constructor(&origin, args, |expr, ty| {
            let previous = self.requirements.delta;
            self.requirements.delta = false;
            let value = self.expected_expression(module, expr, env, false, Some(ty))?;
            self.requirements.delta = previous;
            if value.ty != *ty {
                return Err(Issue::coded(
                    "type",
                    "delta.entry_type",
                    expr.span(),
                    "delta entry requires exact type",
                ));
            }
            self.static_values.key(value).map_err(|message| {
                Issue::coded("type", "delta.entry_constant", expr.span(), message)
            })
        })?;
        let entries = hgl_semantics::delta_check::values(parts, |ty, expr| {
            let previous = self.requirements.delta;
            self.requirements.delta = true;
            let value = self.expected_expression(module, expr, env, runtime, Some(ty))?;
            self.requirements.delta = previous;
            require_payload(&value)?;
            Ok(value)
        })?;
        if expected.is_some_and(|expected| *expected != ty) {
            return Err(Issue::coded(
                "type",
                "delta.type_mismatch",
                0..0,
                "delta originating shape mismatch",
            ));
        }
        Ok(Value::new(ty, Kind::Delta(entries)))
    }
    fn value_call(
        &mut self,
        module: &str,
        name: &str,
        args: Vec<(Option<String>, Value)>,
        runtime: bool,
    ) -> Result<Value, Issue> {
        let (owner, item) = hgl_semantics::value_types::identity(&self.library, module, name);
        if matches!(name, "upsert" | "discard")
            || (name == "contains"
                && args
                    .first()
                    .is_some_and(|(_, v)| matches!(v.ty, Ty::Set(_))))
            || (owner == "hgraph.native"
                && matches!(item.as_str(), "bound" | "len")
                && args
                    .first()
                    .is_some_and(|(_, v)| matches!(v.ty, Ty::Set(_))))
        {
            return Ok(set_call(name, &owner, &item, args)?);
        }
        if matches!(
            name,
            "valid" | "modified" | "all_valid" | "last_modified" | "passivate" | "activate"
        ) {
            return Ok(endpoint_call(name, args, runtime)?);
        }
        self.call(module, name, &args, runtime)
    }
    fn log_info(&mut self, args: &Arguments) -> Result<Value, Issue> {
        let values = method_arguments("info(logger)", &[("value", Ty::Str)], args)?;
        let id = self
            .plan
            .natives
            .iter()
            .position(|n| n.method == "log_info_str")
            .unwrap_or(self.plan.natives.len());
        if id == self.plan.natives.len() {
            self.plan.natives.push(Native {
                name: "info(logger)".into(),
                method: "log_info_str".into(),
                args: vec![Ty::Str],
                result: Ty::Void,
                throws: false,
            });
        }
        Ok(Value::new(Ty::Void, Kind::Native(id, values)))
    }
    fn binary(
        &mut self,
        module: &str,
        op: &str,
        operands: (&Expr, &Expr),
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        if matches!(operands.0.syntax(), Expr::Null) || matches!(operands.1.syntax(), Expr::Null) {
            if !matches!(op, "==" | "!=") {
                return Err("null only supports nullable presence comparisons".into());
            }
            let expr = if matches!(operands.0.syntax(), Expr::Null) {
                operands.1
            } else {
                operands.0
            };
            let value = if let Expr::Name(name) = expr.syntax() {
                env.get(name)
                    .cloned()
                    .ok_or_else(|| format!("unknown value {name}"))?
            } else {
                self.expression(module, expr, env, runtime)?
            };
            if !matches!(value.ty, Ty::Nullable(_)) {
                return Err("null comparison requires a nullable replay result".into());
            }
            let present = Value::new(Ty::Bool, Kind::IsPresent(Box::new(value)));
            return Ok(if op == "!=" {
                present
            } else {
                Value::new(Ty::Bool, Kind::Unary("!".into(), Box::new(present)))
            });
        }
        if !runtime {
            self.graph_locals.payload(operands)?;
        }
        let mut a = self.expression(module, operands.0, env, runtime)?;
        let previous = self.facts.clone();
        if matches!(op, "&&" | "||") {
            self.facts = facts(&a, op == "&&", &self.facts);
        }
        let mut b = self.expression(module, operands.1, env, runtime)?;
        self.facts = previous;
        if !runtime && (matches!(a.kind, Kind::Wire(_)) || matches!(b.kind, Kind::Wire(_))) {
            if !matches!(a.kind, Kind::Wire(_)) {
                a = self.wiring_value(&a)?;
            }
            if !matches!(b.kind, Kind::Wire(_)) {
                b = self.wiring_value(&b)?;
            }
        }
        Ok(hgl_semantics::local_check::binary(
            op,
            [a, b],
            &mut self.plan,
        )?)
    }
}

fn select_eval(
    checker: &Checker,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
) -> Result<(Checker, EvalInputs, Arguments), Issue> {
    let (owner, item) = hgl_semantics::value_types::identity(&checker.library, module, name);
    let helpers =
        checker.library.declarations.iter().any(|d| {
            d.module == owner && d.name == item && d.test_only && d.role == Role::Function
        });
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for decl in checker.library.declarations.clone() {
        if decl.module != owner
            || decl.name != item
            || matches!(
                decl.role,
                Role::Implementation | Role::Test | Role::Native | Role::Struct | Role::Enum
            )
            || (decl.test_only && owner != module)
            || (helpers && owner == module && !decl.test_only)
        {
            continue;
        }
        let signature = decl.signature()?;
        let mut candidate = test_checker(
            checker.library.clone(),
            module,
            &PreparedLexicalScope {
                bindings: checker.harness_values.clone(),
                origins: checker.static_values.clone(),
                facts: checker.facts.clone(),
                ..PreparedLexicalScope::default()
            },
        );
        match eval_arguments(
            &mut candidate,
            module,
            &decl.module,
            &signature,
            args,
            &decl.source,
        ) {
            Ok((plan, values)) => candidates.push((candidate, plan, values)),
            Err(error) => errors.push(error),
        }
    }
    if candidates.len() != 1 {
        return Err(Issue::alternatives(
            errors,
            format!(
                "{owner}::{item}: expected one eval signature, found {}",
                candidates.len()
            ),
        ));
    }
    Ok(candidates.remove(0))
}

pub(crate) fn prepare_evaluation(
    library: Library,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
    scope: &PreparedLexicalScope,
) -> Result<(Plan, Vec<hgl_semantics::harness_ir::Argument>), Issue> {
    let checker = test_checker(library, module, scope);
    let (mut checker, plan, mut values) = select_eval(&checker, module, name, args)?;
    let mut prepared = Vec::new();
    checker.static_values.prepared = values.iter().map(|(_, value)| value.clone()).collect();
    for (binding, (name, value)) in values.iter_mut().enumerate() {
        if let Kind::Wire(input) = value.kind {
            let shape = value.ty.clone();
            let entry_type = hgl_semantics::source_check::replay_type(&checker.library, &shape)?;
            let data = Value::new(
                Ty::List(Box::new(entry_type.clone()), None),
                Kind::Prepared(binding),
            );
            prepared.push(hgl_semantics::harness_ir::Argument::Dense {
                parameter: name.clone().ok_or("missing checked eval parameter")?,
                binding,
                shape,
                entry_type,
                slots: plan[input].0.clone(),
                sequence: plan[input].1.clone().map(Box::new),
            });
            *value = checker.call("hgraph.std", "replay", &[(None, data)], false)?;
        } else {
            prepared.push(hgl_semantics::harness_ir::Argument::Constant {
                binding,
                value: value.clone(),
            });
            *value = Value::new(value.ty.clone(), Kind::Prepared(binding));
        }
    }
    checker.result_hint = None;
    let output = checker.call(module, name, &values, false)?;
    if matches!(output.kind, Kind::Wire(_)) {
        if !output.ty.publication() {
            return Err("eval output is outside the delta publication profile".into());
        }
        let ty = output.ty.clone();
        let key = hgl_semantics::eval_data::recording_key(&checker.plan);
        checker.call(
            "hgraph.std",
            "record",
            &[
                (None, output),
                (
                    None,
                    Value::new(Ty::Str, Kind::Literal(Literal::Str(key.clone()))),
                ),
            ],
            false,
        )?;
        let record = checker.plan.nodes.len() - 1;
        let recording = checker.plan.nodes[record]
            .globals
            .iter()
            .find(|(name, _)| name == &key)
            .ok_or("record must prepare its ordinary recording entry")?
            .1
            .clone();
        checker.plan.recording = Some((key, recording));
        checker.plan.output = Some((record, ty));
    }
    Ok((checker.plan, prepared))
}

type EvalInputs = Vec<(Vec<Option<Value>>, Option<Value>)>;

fn checked_eval(
    checker: &mut Checker,
    module: &str,
    expr: &Expr,
    expected: Option<&Ty>,
) -> Result<Value, Issue> {
    let mut value = if let Some(expected) = expected {
        checker.harness_expression(module, expr, expected)?
    } else {
        checker.expression(module, expr, &checker.harness_values.clone(), false)?
    };
    if expected == Some(&Ty::F64) && value.ty == Ty::I64 {
        value = Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(value)));
    }
    if let Some(ty) = expected
        && value.ty != *ty
    {
        return Err(Issue::coded(
            "type",
            "delta.type_mismatch",
            expr.span(),
            format!("expected {}, got {}", ty.name(), value.ty.name()),
        ));
    }
    Ok(value)
}

pub(crate) fn prepared_expected(
    library: Library,
    module: &str,
    ty: &Ty,
    slots: &[Option<Expr>],
    scope: &PreparedLexicalScope,
) -> Result<Vec<Option<Value>>, Issue> {
    let mut checker = test_checker(library, module, scope);
    hgl_semantics::eval_data::sequence_checked(slots, Some(ty.clone()), |expr, expected| {
        checked_eval(&mut checker, module, expr, expected)
    })
    .map(|(_, slots)| slots)
}

fn eval_configuration(
    checker: &mut Checker,
    module: &str,
    owner: &str,
    signature: &Signature,
    args: &[(Option<String>, Expr)],
) -> Result<Arguments, Issue> {
    let mut values = Vec::new();
    for (index, (label, expr)) in args.iter().enumerate() {
        let parameter = hgl_semantics::eval_data::parameter(signature, index, label.as_deref())?;
        if !parameter.constant {
            continue;
        }
        let name = checker.type_sizes(owner, &parameter.ty, &Env::new())?;
        let hint = hgl_semantics::value_types::resolve(
            &checker.library,
            owner,
            &name,
            &mut BTreeSet::new(),
        )
        .ok()
        .map(|ty| hgl_semantics::value_access::project(&ty));
        values.push((
            Some(parameter.name.clone()),
            checked_eval(checker, module, expr, hint.as_ref())?,
        ));
    }
    Ok(values)
}

fn eval_arguments(
    checker: &mut Checker,
    module: &str,
    owner: &str,
    signature: &Signature,
    args: &[(Option<String>, Expr)],
    source: &str,
) -> Result<(EvalInputs, Arguments), Issue> {
    let configuration = eval_configuration(checker, module, owner, signature, args)?;
    let signature = checker.signature_sizes(owner, signature.clone(), &configuration, source)?;
    let mut plan = Vec::new();
    let mut values = Vec::new();
    for (position, (label, expr)) in args.iter().enumerate() {
        let parameter =
            hgl_semantics::eval_data::parameter(&signature, position, label.as_deref())?;
        let type_name = checker.type_sizes(owner, &parameter.ty, &Env::new())?;
        let hint = hgl_semantics::value_types::resolve(
            &checker.library,
            owner,
            &type_name,
            &mut BTreeSet::new(),
        )
        .ok()
        .map(|ty| if let Ty::Ref(child) = ty { *child } else { ty });
        let value = if let Expr::Sequence(ticks) = expr.syntax() {
            if parameter.constant {
                return Err("const parameter requires a fixed value".into());
            }
            let (ty, slots) =
                hgl_semantics::eval_data::sequence_checked(ticks, hint, |expr, expected| {
                    checked_eval(checker, module, expr, expected)
                })?;
            let index = plan.len();
            plan.push((slots, None));
            Value::new(ty, Kind::Wire(index))
        } else {
            if !parameter.constant {
                let value = checked_eval(checker, module, expr, None)?;
                let Ty::List(child, _) = &value.ty else {
                    return Err("temporal eval argument requires a sequence".into());
                };
                let ty =
                    hint.ok_or("ordinary publication sequence requires a concrete parameter")?;
                if child.as_ref() != &ty.clone().delta()? {
                    return Err(Issue::coded(
                        "type",
                        "delta.type_mismatch",
                        expr.span(),
                        "ordinary publication sequence child mismatch",
                    ));
                }
                let input = plan.len();
                plan.push((Vec::new(), Some(value)));
                values.push((
                    Some(parameter.name.clone()),
                    Value::new(ty, Kind::Wire(input)),
                ));
                continue;
            }
            configuration
                .iter()
                .find(|(name, _)| name.as_deref() == Some(parameter.name.as_str()))
                .map(|(_, value)| value.clone())
                .ok_or("missing evaluated configuration")?
        };
        values.push((Some(parameter.name.clone()), value));
    }
    let types =
        hgl_semantics::value_bind::signature_types(&checker.library, owner, &signature, &values)?;
    let (bound, _, _) = bind_prepared(&signature, &values, false, None, &types)?;
    for (parameter, value) in signature.parameters.iter().zip(bound) {
        if parameter.constant
            && !values
                .iter()
                .any(|(name, _)| name.as_ref() == Some(&parameter.name))
        {
            values.push((Some(parameter.name.clone()), value));
        }
    }
    Ok((plan, values))
}

#[cfg(test)]
#[path = "../tests/support/nullable.rs"]
mod nullable_tests;

#[cfg(test)]
#[path = "../tests/support/eval_composition.rs"]
mod eval_composition_tests;

pub(crate) struct TestChecker {
    pub library: Library,
    pub module: String,
}
impl hgl_semantics::harness_check::Check for TestChecker {
    fn statement(
        &mut self,
        statement: &Stmt,
        scope: &mut PreparedLexicalScope,
    ) -> Result<Statement, Issue> {
        prepared_statement(self.library.clone(), &self.module, statement, scope)
    }
    fn boolean(&mut self, expr: &Expr, scope: &PreparedLexicalScope) -> Result<Value, Issue> {
        prepared_assertion(self.library.clone(), &self.module, expr, scope)
    }
    fn evaluation(
        &mut self,
        call: &hgl_semantics::eval_data::Evaluation,
        scope: &PreparedLexicalScope,
    ) -> Result<(Plan, Vec<hgl_semantics::harness_ir::Argument>), Issue> {
        prepare_evaluation(
            self.library.clone(),
            &self.module,
            &call.function,
            &call.arguments,
            scope,
        )
    }
    fn expected(
        &mut self,
        ty: &Ty,
        slots: &[Option<Expr>],
        scope: &PreparedLexicalScope,
    ) -> Result<Vec<Option<Value>>, Issue> {
        prepared_expected(self.library.clone(), &self.module, ty, slots, scope)
    }
}

pub(crate) fn validate_declaration(library: Library, decl: &Decl) -> Result<(), Issue> {
    if decl.role == Role::Test {
        return hgl_semantics::harness_check::block(
            hgl_semantics::eval_data::steps_checked(&decl.tokens)?,
            &mut PreparedLexicalScope::default(),
            &mut Vec::new(),
            &mut TestChecker {
                library,
                module: decl.module.clone(),
            },
        )
        .map(|_| ());
    }
    let Some(signature) = hgl_semantics::source_check::concrete_signature(decl)? else {
        return Ok(());
    };
    let mut checker = Checker {
        library,
        test_scope: decl.test_only.then(|| decl.module.clone()),
        ..Checker::default()
    };
    let signature = checker.signature_sizes(&decl.module, signature, &[], &decl.source)?;
    for (name, tokens) in signature
        .parameters
        .iter()
        .map(|parameter| (&parameter.ty, &parameter.type_tokens))
        .chain(std::iter::once((
            &signature.result,
            &signature.result_tokens,
        )))
    {
        if name.contains("delta<") {
            hgl_semantics::value_types::concrete(
                &checker.library,
                &decl.module,
                name,
                &BTreeMap::new(),
            )
            .map_err(|issue| {
                hgl_semantics::library::locate_type_issue(issue, tokens, &decl.source)
            })?;
        }
    }
    if decl.role == Role::Function && signature.parameters.is_empty() && !signature.body.is_empty()
    {
        checker.call(&decl.module, &decl.name, &[], false)?;
    }
    Ok(())
}
