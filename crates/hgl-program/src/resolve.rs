use crate::index::{Decl, Library, Role, Signature};
use crate::syntax::{Cursor, Expr, Literal, Stmt, Ty};
use hgl_body_check::runtime_body;
use hgl_endpoint_check::{
    capability_function, capability_payload, clock_property, endpoint_call, endpoint_metadata,
    injected_clock, require_payload, set_call,
};
use hgl_flow_check::{facts, handler_facts, merge_facts, terminates};
use hgl_rust::{Kind, Native, Node, Plan, Statement, Value};
use hgl_value_bind::{bind, bind_prepared, method_arguments, order_arguments, resolve_type};
use hgl_value_check::{field, ordinary, writable};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Checker {
    library: Library,
    plan: Plan,
    active: BTreeSet<usize>,
    documented: BTreeSet<usize>,
    test_scope: Option<String>,
    harness_values: Env,
    static_values: hgl_value_access::StaticValues,
    result_hint: Option<Ty>,
    phase: Phase,
    types: BTreeMap<String, Ty>,
    runtime_node: bool,
    value_context: ValueContext,
    generator: bool,
    preparing_graph: bool,
    graph_locals: hgl_local_check::GraphLocals,
    wiring: hgl_value_eval::Evaluator,
    next_wiring_local: usize,
    global_types: BTreeMap<String, Ty>,
    globals: Vec<(String, Ty)>,
    failed_globals: BTreeSet<usize>,
    facts: BTreeSet<(String, usize)>,
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

fn test_checker(library: Library, module: &str, values: Env) -> Checker {
    Checker {
        library,
        test_scope: Some(module.into()),
        harness_values: values,
        value_context: ValueContext::Preparation,
        ..Checker::default()
    }
}
pub(crate) fn prepared_statement(
    library: Library,
    module: &str,
    statement: &Stmt,
    env: &mut Env,
    next: &mut usize,
) -> Result<Statement, String> {
    let mut checker = test_checker(library, module, Env::new());
    checker.statement(module, statement, env, &Ty::Void, next)
}
pub(crate) fn prepared_assertion(
    library: Library,
    module: &str,
    expr: &Expr,
    env: &Env,
) -> Result<Value, String> {
    let mut checker = test_checker(library, module, Env::new());
    let value = checker.expression(module, expr, env, false)?;
    if value.ty != Ty::Bool {
        return Err("ordinary assertion requires bool".into());
    }
    if hgl_value_check::context_free(&value) {
        checker
            .wiring
            .value(&value)
            .map_err(|e| format!("constant evaluation: {e}"))?;
    }
    Ok(value)
}
pub(crate) fn compile(library: Library, module: &str, entry: &str) -> Result<Plan, String> {
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
    ) -> Result<Value, String> {
        let values = self.call_values(args, runtime)?;
        let args = values.as_slice();
        let (module, name) = hgl_value_types::identity(&self.library, module, name);
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
            return hgl_value_bind::native(
                &self.library,
                &decl,
                &signature,
                (values, result),
                runtime,
                &mut self.plan.natives,
            );
        }
        let runtime_body = runtime_body(&signature)?;
        if signature.value_function && values.iter().all(|v| !matches!(v.kind, Kind::Wire(_))) {
            return self.direct_call(id, &signature, (values, types, result), runtime, args);
        }
        if runtime {
            return Err("temporal calls in runtime handlers are not supported".into());
        }
        if !self.active.insert(id) {
            return Err(format!("recursive graph construction: {name}"));
        }
        let previous_scope = self.test_scope.clone();
        self.test_scope = decl.test_only.then(|| module.clone());
        let mut env: Env = signature
            .parameters
            .iter()
            .zip(&values)
            .map(|(p, v)| (p.name.clone(), v.clone()))
            .collect();
        let previous_types = std::mem::replace(&mut self.types, types);
        let mut cursor = Cursor::new(&signature.body);
        let value = if runtime_body {
            self.runtime_node = !signature.value_function;
            let context = std::mem::replace(&mut self.value_context, ValueContext::Outside);
            let node = self.node(&decl, &signature, &mut cursor, &mut env, &result)?;
            self.value_context = context;
            let index = self.plan.nodes.len();
            self.plan.nodes.push(node);
            Value {
                ty: result.clone(),
                kind: if result == Ty::Void {
                    Kind::Void
                } else {
                    Kind::Wire(index)
                },
            }
        } else {
            let previous = std::mem::replace(&mut self.preparing_graph, true);
            let value = self.graph(&module, &mut cursor, &mut env, (&result, &signature))?;
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
    ) -> Result<Arguments, String> {
        args.iter().map(|(name, value)| {
            let prepared = self.value_context == ValueContext::Preparation;
            let local = matches!(value.kind, Kind::Local(id) | Kind::MutableLocal(id) if id < self.next_wiring_local);
            let value = if runtime || matches!(value.kind, Kind::Wire(_)) || (prepared && !local) {
                value.clone()
            } else { self.wiring_value(value)? };
            Ok((name.clone(), value))
        }).collect()
    }
    fn constant_size(&mut self, module: &str, source: &str, env: &Env) -> Result<Literal, String> {
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
    fn type_sizes(&mut self, module: &str, name: &str, env: &Env) -> Result<String, String> {
        for (owner, expr) in hgl_struct_check::schema_sizes(&self.library, module, name)? {
            let Literal::Int(size) = self.constant_size(&owner, &expr, &Env::new())? else {
                return Err("list size requires a constant i64".into());
            };
            if size < 0 {
                return Err("list size must be nonnegative or unbounded".into());
            }
            self.library.type_sizes.insert((owner, expr), size);
        }
        hgl_type_sizes::normalize(name, &mut |source| self.constant_size(module, source, env))
    }
    fn signature_sizes(
        &mut self,
        module: &str,
        mut signature: Signature,
        args: &[(Option<String>, Value)],
    ) -> Result<Signature, String> {
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
            parameter.ty = self.type_sizes(module, &parameter.ty, &env)?;
        }
        signature.result = self.type_sizes(module, &signature.result, &env)?;
        Ok(signature)
    }
    fn direct_call(
        &mut self,
        id: usize,
        signature: &Signature,
        (args, types, result): (Vec<Value>, BTreeMap<String, Ty>, Ty),
        runtime: bool,
        supplied: &[(Option<String>, Value)],
    ) -> Result<Value, String> {
        let decl = self.library.declarations[id].clone();
        let (args, positions) = hgl_value_bind::ordered_call(signature, args, supplied)?;
        if !self.active.insert(id) {
            return Err("recursive ordinary value calls are unsupported".into());
        }
        let mut env: Env = signature
            .parameters
            .iter()
            .zip(&positions)
            .map(|(p, id)| {
                let v = &args[*id];
                (
                    p.name.clone(),
                    if p.constant && self.value_context != ValueContext::Preparation {
                        v.clone()
                    } else {
                        Value::new(v.ty.clone(), Kind::Local(*id))
                    },
                )
            })
            .collect();
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
        let (services, body) = hgl_body_check::ordinary_body(&mut cursor)?;
        for service in services {
            if !runtime || !matches!(service.as_str(), "clock" | "logger") {
                return Err(format!(
                    "ordinary helper injectable {service} requires a supported runtime service context"
                ));
            }
            if env
                .insert(service.clone(), Value::new(Ty::Void, Kind::Capability))
                .is_some()
            {
                return Err(format!(
                    "{service}: injectable conflicts with an existing binding"
                ));
            }
        }
        let body = self.statements(&decl.module, &body, &mut env, &result, &mut args.len())?;
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
    fn argument_hint(
        &self,
        module: &str,
        name: &str,
        index: usize,
        label: Option<&str>,
    ) -> Option<Ty> {
        let (owner, item) = hgl_value_types::identity(&self.library, module, name);
        let mut declarations = self.library.declarations.iter().filter(|d| {
            d.module == owner
                && d.name == item
                && matches!(d.role, Role::Function | Role::Native | Role::Operator)
        });
        let signature = declarations.next()?.signature().ok()?;
        if declarations.next().is_some() {
            return None;
        }
        let parameter = if let Some(label) = label {
            signature.parameters.iter().find(|p| p.name == label)?
        } else {
            signature.parameters.get(index)?
        };
        hgl_value_types::resolve_ordinary(&self.library, &owner, &parameter.ty).ok()
    }
    fn select(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Selection, String> {
        let mut candidates = Vec::new();
        let mut errors = Vec::new();
        for (id, decl) in self.library.declarations.clone().iter().enumerate() {
            if decl.module != module
                || decl.name != name
                || matches!(decl.role, Role::Implementation | Role::Test | Role::Struct)
            {
                continue;
            }
            if decl.test_only && self.test_scope.as_deref() != Some(module) {
                continue;
            }
            if !decl.test_only
                && self.test_scope.as_deref() == Some(module)
                && self.library.declarations.iter().any(|d| {
                    d.test_only && d.module == module && d.name == name && d.role == Role::Function
                })
            {
                continue;
            }
            let signature = match decl
                .signature()
                .and_then(|s| self.signature_sizes(module, s, args))
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
            let declared = match hgl_value_bind::signature_types(
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
                Err(e) => errors.push(e),
            }
        }
        if candidates.len() != 1 {
            return Err(format!(
                "{module}::{name}: expected one matching declaration, found {}; {}",
                candidates.len(),
                errors.join("; ")
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
    ) -> Result<(usize, Signature, BTreeMap<String, Ty>), String> {
        let module = declaration.module.as_str();
        let name = declaration.name.as_str();
        let mut implementations = Vec::new();
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
            let Ok(sig) = implementation
                .signature()
                .and_then(|s| self.signature_sizes(module, s, &positional))
            else {
                continue;
            };
            let Ok(declared) =
                hgl_value_bind::signature_types(&self.library, module, &sig, &positional)
            else {
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
                && !hgl_value_bind::instantiated(
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
            return Err(format!(
                "{module}::{name}: expected one instantiated implementation, found {}",
                implementations.len()
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
            let (owner, item) = hgl_value_types::identity(&self.library, module, required);
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
    fn wiring_value(&mut self, value: &Value) -> Result<Value, String> {
        match self.wiring.value(value) {
            Ok(value) => Ok(value),
            Err(hgl_value_eval::EvalError::Operation(message)) => {
                self.plan.construction_error.get_or_insert(message.clone());
                Ok(Value::new(value.ty.clone(), Kind::WiringFailure(message)))
            }
            Err(hgl_value_eval::EvalError::Unsupported(message)) => Err(message),
            Err(hgl_value_eval::EvalError::ContextRequired) => {
                Err("contextual value requires run preparation".into())
            }
        }
    }
    fn wiring_statement(&mut self, statement: &Statement) -> Result<(), String> {
        match self.wiring.statement(statement) {
            Ok(_) => Ok(()),
            Err(hgl_value_eval::EvalError::Operation(message)) => {
                self.plan.construction_error.get_or_insert(message);
                Ok(())
            }
            Err(hgl_value_eval::EvalError::Unsupported(message)) => Err(message),
            Err(hgl_value_eval::EvalError::ContextRequired) => {
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
    ) -> Result<(), String> {
        if env.contains_key(name) {
            return Err(format!("duplicate local {name}"));
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
            hgl_value_check::binding(id, &value, mutable, annotated)?,
        );
        let value = self.wiring_value(&value)?;
        if matches!(value.kind, Kind::WiringFailure(_)) {
            self.wiring.bind_failed(id, value, mutable);
            return Ok(());
        }
        self.wiring_statement(&hgl_local_check::statement(id, value, mutable, annotated)?.1)
    }
    fn graph(
        &mut self,
        module: &str,
        cursor: &mut Cursor<'_>,
        env: &mut Env,
        (result, signature): (&Ty, &Signature),
    ) -> Result<Value, String> {
        let locals = hgl_local_check::GraphLocals::new(
            signature
                .parameters
                .iter()
                .filter(|p| p.ty == "signal")
                .map(|p| p.name.clone()),
        );
        let previous_locals = std::mem::replace(&mut self.graph_locals, locals);
        let previous_hint = self.result_hint.replace(result.clone());
        let statements = hgl_body_check::composition_body(cursor)?;
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
                Stmt::Call(expr) => {
                    output = self.expression(module, expr, env, false)?;
                    if !matches!(output.kind, Kind::Wire(_) | Kind::Void) {
                        self.wiring_value(&output)?;
                    }
                }
                Stmt::Return(expr) => {
                    if position + 1 != statements.len() {
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
                Stmt::If(..) if hgl_wiring_locals::handles(statement, env) => {
                    hgl_wiring_locals::conditional(
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
                Stmt::Exit | Stmt::For(..) => return Err("runtime statement in graph".into()),
            }
        }
        self.graph_locals = previous_locals;
        self.result_hint = previous_hint;
        hgl_local_check::graph_result(output, result)
    }
    fn graph_assignment(
        &mut self,
        module: &str,
        (name, expr): (&str, &Expr),
        env: &mut Env,
        mutable_port: bool,
        compound: bool,
    ) -> Result<(), String> {
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
        let value = hgl_local_check::replacement(&target, value, mutable_port)?;
        if matches!(target.kind, Kind::Wire(_)) {
            env.insert(name.into(), value);
            Ok(())
        } else {
            self.wiring_statement(&hgl_local_check::assignment(target, value)?)
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
                    hgl_documentation::normalize(&decl.doc)
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
    ) -> Result<Node, String> {
        let generator = signature.body.iter().any(|token| token.text == "yield");
        if generator
            && signature
                .parameters
                .iter()
                .any(|parameter| !parameter.constant)
        {
            return Err("generator sources cannot have temporal parameters".into());
        }
        let mut node = hgl_value_check::prepare_node(
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
        hgl_value_check::validate(&node)?;
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
    ) -> Result<Node, String> {
        if !node.result.publication() {
            return Err("generator requires a declared publication output type".into());
        }
        let (services, body) = hgl_body_check::generator_body(c)?;
        for service in services {
            inject_capability(&service, &mut node, env)?;
        }
        self.generator = true;
        self.phase = Phase::Evaluation;
        self.facts.clear();
        node.generator = Some(self.statements(&decl.module, &body, env, &node.result, &mut 0)?);
        self.generator = false;
        self.runtime_node = false;
        self.facts.clear();
        node.alarm = true;
        hgl_value_check::validate(&node)?;
        Ok(node)
    }
    fn hooks(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &Env,
        node: &mut Node,
        next_local: &mut usize,
    ) -> Result<(), String> {
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
                    return Err(format!("duplicate {hook} hook"));
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
    ) -> Result<(), String> {
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
                let expr = node_guard(node, c.expr()?);
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
    ) -> Result<(), String> {
        let result = &node.result.clone();
        while c.take("inject") {
            loop {
                match c.name()?.as_str() {
                    name @ ("alarm" | "clock" | "logger" | "global_state") => {
                        inject_capability(name, node, env)?;
                    }
                    "out" => {
                        env.insert("out".into(), Value::new(result.clone(), Kind::Output));
                    }
                    name => return Err(format!("unsupported injectable {name}")),
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
    ) -> Result<(), String> {
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
    ) -> Result<Vec<Statement>, String> {
        let mut bindings = BTreeSet::new();
        let mut checked = Vec::new();
        for statement in statements {
            if let Stmt::Let(name, _, _) | Stmt::Var(name, _, _) = statement
                && !bindings.insert(name)
            {
                return Err(format!("duplicate local {name}"));
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
    ) -> Result<Value, String> {
        let mut ty = annotation
            .map(|name| {
                let name = self.type_sizes(module, name, env)?;
                resolve_type(&name, &self.types).map_or_else(
                    || {
                        hgl_value_types::substitute(
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
                hgl_value_access::project(ty)
            }
        });
        let value = self.expected_expression(module, expr, env, runtime, hint.as_ref())?;
        if !matches!(value.kind, Kind::Wire(_)) {
            ty = hint;
        }
        hgl_local_check::initializer(value, ty.as_ref())
    }
    fn struct_declaration(&self, module: &str, name: &str) -> Result<Option<&Decl>, String> {
        hgl_value_types::declaration(&self.library, module, name)
    }
    fn global_operation(&mut self, operation: &str, args: &Arguments) -> Result<Value, String> {
        if operation == "get" {
            return Err("global_state: get requires a concrete scalar expected type or a typed ordinary struct binding".into());
        }
        if operation != "set" {
            return Err(format!(
                "global_state: unknown capability operation {operation}"
            ));
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
    fn global_entry(&mut self, key: &Value, ty: &Ty) -> Result<usize, String> {
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
                return Err(format!("global_state: type conflict for key {key:?}"));
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
    ) -> Result<Value, String> {
        let Expr::Tuple(cells) = expr else {
            return self.expected_expression(
                module,
                expr,
                &self.harness_values.clone(),
                false,
                Some(expected),
            );
        };
        let Ty::Delta(origin) = expected else {
            return self.expected_expression(
                module,
                expr,
                &self.harness_values.clone(),
                false,
                Some(expected),
            );
        };
        let Ty::Tuple(children) = origin.as_ref() else {
            return Err("tuple shorthand requires an exact tuple publication shape".into());
        };
        if children.len() != cells.len() {
            return Err("tuple shorthand arity mismatch".into());
        }
        let mut entries = Vec::new();
        for (index, (child, expr)) in children.iter().zip(cells).enumerate() {
            if let Some(expr) = expr {
                let ty = child.clone().delta()?;
                let mut value = self.harness_expression(module, expr, &ty)?;
                if ty == Ty::F64 && value.ty == Ty::I64 {
                    value = Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(value)));
                }
                if value.ty != ty {
                    return Err("tuple shorthand child type mismatch".into());
                }
                entries.push(hgl_rust::DeltaEntry::Child(
                    i64::try_from(index).map_err(|e| e.to_string())?,
                    value,
                ));
            }
        }
        Ok(Value::new(expected.clone(), Kind::Delta(entries)))
    }
    fn expected_expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, String> {
        if let Expr::Applied(name, args) = expr {
            return self.constructor(module, (name, args), env, runtime, expected);
        }
        if let Expr::Call(name, args) = expr
            && self.struct_declaration(module, name)?.is_some()
        {
            return self.constructor(module, (name, args), env, runtime, expected);
        }
        if matches!(expr, Expr::Sequence(_) | Expr::Tuple(_)) {
            return hgl_value_check::aggregate(
                expr,
                expected,
                matches!(
                    self.value_context,
                    ValueContext::Constant | ValueContext::Preparation
                ) || !runtime,
                |expr, expected| self.expected_expression(module, expr, env, runtime, expected),
            );
        }
        if let Expr::Call(name, args) = expr
            && name == "get"
            && matches!(args.first(), Some((None, Expr::Name(receiver))) if receiver == "global_state" && env.get(receiver).is_some_and(|v| matches!(v.kind, Kind::Capability)))
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
                .collect::<Result<Vec<_>, String>>()?;
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
    ) -> Result<Statement, String> {
        Ok(match statement {
            Stmt::Let(name, annotation, expr) | Stmt::Var(name, annotation, expr) => {
                let value =
                    self.local_initializer(module, annotation.as_deref(), expr, env, true)?;
                let mutable = matches!(statement, Stmt::Var(..));
                let id = *next_local;
                *next_local += 1;
                let (binding, statement) =
                    hgl_local_check::statement(id, value, mutable, annotation.is_some())?;
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
            Stmt::TimedYield(time, payload) => {
                self.timed_yield(module, (time, payload), env, result)?
            }
            Stmt::While(expr, body) => {
                self.while_statement(module, (expr, body), env, result, next_local)?
            }
            Stmt::For(name, collection, body) => {
                if !matches!(collection, Expr::Call(name, _) if name == "elements") {
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
    ) -> Result<Statement, String> {
        if self.generator && self.value_context == ValueContext::Outside {
            return Err("generator return cannot carry a value; use yield".into());
        }
        let expected =
            if self.value_context == ValueContext::Outside && !matches!(result, Ty::Ref(_)) {
                result.clone().delta()?
            } else {
                result.clone()
            };
        let v = self.expected_expression(module, expr, env, true, Some(&expected))?;
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
        (time, payload): (&Expr, &Expr),
        env: &Env,
        result: &Ty,
    ) -> Result<Statement, String> {
        if !self.generator || self.value_context != ValueContext::Outside {
            return Err("yield requires a generator source".into());
        }
        let time = self.expression(module, time, env, true)?;
        if !matches!(time.ty, Ty::Duration | Ty::DateTime) {
            return Err("yield time requires datetime or duration".into());
        }
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
    ) -> Result<Statement, String> {
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
    ) -> Result<Value, String> {
        if let Expr::Property(parent, _) = target
            && injected_clock(parent, env)
        {
            return Err("clock properties are read-only".into());
        }
        if !matches!(target, Expr::Name(_) | Expr::Property(..)) {
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
    ) -> Result<Statement, String> {
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
    ) -> Result<Statement, String> {
        let target = self.assignment_target(module, target, env)?;
        let expected = hgl_local_check::assignment_type(&target, self.phase == Phase::Evaluation)?;
        let value = self.expected_expression(module, expr, env, true, Some(&expected))?;
        hgl_local_check::assignment(target, value)
    }
    fn conditional(
        &mut self,
        module: &str,
        (expr, yes, no): (&Expr, &[Stmt], &[Stmt]),
        env: &Env,
        result: &Ty,
        next_local: &mut usize,
    ) -> Result<Statement, String> {
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
    ) -> Result<Value, String> {
        match expr {
            Expr::Tuple(_) => self.expected_expression(module, expr, env, runtime, None),
            Expr::Sparse(_) => Err("sparse entries require a delta constructor context".into()),
            Expr::Null => Err("null requires a contextual nullable comparison".into()),
            Expr::Property(receiver, name) => {
                if injected_clock(receiver, env) {
                    return clock_property(receiver, name, env, runtime);
                }
                let parent = self.expression(module, receiver, env, runtime)?;
                field(parent, name)
            }
            Expr::Index(receiver, index) => {
                let parent = self.expression(module, receiver, env, runtime)?;
                let index = self.expression(module, index, env, runtime)?;
                hgl_value_check::indexed(parent, index)
            }
            Expr::Sequence(elements) => hgl_value_check::list_literal(elements, None),
            Expr::Literal(l) => Ok(Value::new(l.ty(), Kind::Literal(l.clone()))),
            Expr::TemporalLiteral(l) if self.value_context == ValueContext::Preparation && !self.preparing_graph => Ok(Value::new(l.ty(), Kind::TemporalLiteral(l.clone()))),
            Expr::TemporalLiteral(_) => Err("contextual temporal construction requires run preparation; node-hook construction is unsupported".into()),
            Expr::Name(name) => {
                if !env.contains_key(name) && let Some(value) = hgl_enums::member(&self.library, module, name)? { return Ok(Value::new(value.ty(), Kind::Literal(value))); }
                let value = env
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("unknown value {name}"))?;
                if matches!(value.kind, Kind::Void | Kind::Capability) {
                    return Err(format!("{name}: injectable cannot escape as a value"));
                }
                if self.phase != Phase::Evaluation
                    && matches!(value.kind, Kind::Input(..) | Kind::Output)
                {
                    let phase = if self.phase == Phase::Stop {
                        "stop"
                    } else {
                        "start"
                    };
                    return Err(format!("{phase} cannot access temporal endpoints"));
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
    ) -> Result<Value, String> {
        hgl_enums::check_call(&self.library, module, name, args)?;
        if let Some((label, Expr::Name(receiver))) = args.first()
            && env
                .get(receiver)
                .is_some_and(|v| matches!(v.kind, Kind::Capability))
        {
            if label.is_some() || !runtime {
                return Err(format!(
                    "{receiver}: capability requires a first positional runtime operand"
                ));
            }
            let values = args[1..]
                .iter()
                .map(|(n, e)| {
                    let value = self.expression(module, e, env, runtime)?;
                    require_payload(&value)?;
                    Ok((n.clone(), value))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if receiver == "global_state" {
                return self.global_operation(name, &values);
            }
            if receiver == "logger" && name == "info" {
                return self.log_info(&values);
            }
            return capability_function(receiver, name, &values, env, self.phase == Phase::Stop);
        }
        if name == "delta_value" {
            return self.delta_value(module, args, env, runtime);
        }
        if name == "elements" {
            if args.len() != 2 || !matches!(&args[1].1,Expr::Name(n) if n=="added") {
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
                return hgl_value_check::list_operation(name, &values);
            }
        }
        if self.struct_declaration(module, name)?.is_some() {
            return self.constructor(module, (name, args), env, runtime, None);
        }
        let args = args
            .iter()
            .enumerate()
            .map(|(index, (n, v))| {
                let hint = self.argument_hint(module, name, index, n.as_deref());
                let value = self.expected_expression(module, v, env, runtime, hint.as_ref())?;
                if !endpoint_metadata(name) {
                    require_payload(&value)?;
                }
                Ok((n.clone(), value))
            })
            .collect::<Result<Vec<_>, String>>()?;
        for (_, value) in &args {
            hgl_value_check::helper_argument(value)?;
        }
        self.value_call(module, name, args, runtime)
    }
    fn delta_value(
        &mut self,
        module: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, String> {
        if !runtime
            || !self.runtime_node
            || self.phase != Phase::Evaluation
            || args.len() != 1
            || args[0].0.is_some()
        {
            return Err("delta_value requires one runtime input endpoint in evaluation".into());
        }
        let value = self.expression(module, &args[0].1, env, runtime)?;
        let Kind::Input(id, false) = value.kind else {
            return Err(
                "delta_value requires a temporal input endpoint; signal is not admitted".into(),
            );
        };
        if !value.ty.publication() {
            return Err("delta_value: unsupported publication shape".into());
        }
        if !["valid", "modified"]
            .iter()
            .all(|q| self.facts.contains(&(q.to_string(), id)))
        {
            return Err(
                "delta_value requires proof that its endpoint is valid and modified".into(),
            );
        }
        Ok(Value::new(
            value.ty.clone().delta()?,
            Kind::Query("delta_value".into(), vec![value]),
        ))
    }
    fn constructor(
        &mut self,
        module: &str,
        (name, args): (&str, &[(Option<String>, Expr)]),
        env: &Env,
        runtime: bool,
        expected: Option<&Ty>,
    ) -> Result<Value, String> {
        let name = self.type_sizes(module, name, env)?;
        if let Some(origin) = name
            .strip_prefix("delta<")
            .and_then(|name| name.strip_suffix('>'))
        {
            return self.delta_constructor(module, (origin, args), env, runtime, expected);
        }
        let mut check = hgl_struct_check::Constructor::new(
            &self.library,
            module,
            &name,
            args,
            expected,
            &self.types,
        )?;
        while let Some((index, hint)) = check.next(&self.library, args)? {
            let value =
                self.expected_expression(module, &args[index].1, env, runtime, hint.as_ref())?;
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
    ) -> Result<Value, String> {
        let origin = hgl_value_types::substitute(
            &self.library,
            module,
            origin,
            &self.types,
            &mut BTreeSet::new(),
        )?;
        let ty = origin.clone().delta()?;
        if expected.is_some_and(|expected| *expected != ty) {
            return Err("delta originating shape mismatch".into());
        }
        let parts = hgl_delta_check::constructor(&origin, args, |expr| {
            let value = self.expression(module, expr, env, false)?;
            let value = self
                .wiring
                .value(&value)
                .map_err(|e| format!("delta position requires a constant: {e}"))?;
            let Kind::Literal(value) = value.kind else {
                return Err("delta position requires a constant scalar".into());
            };
            Ok(value)
        })?;
        let mut entries = Vec::new();
        for part in parts {
            entries.push(match part {
                hgl_delta_check::Part::Added(value) => hgl_rust::DeltaEntry::Add(value),
                hgl_delta_check::Part::Removed(value) => hgl_rust::DeltaEntry::Remove(value),
                hgl_delta_check::Part::Child(key, ty, expr) => {
                    let value = self.expected_expression(module, expr, env, runtime, Some(&ty))?;
                    require_payload(&value)?;
                    if value.ty != ty {
                        return Err("delta child type mismatch".into());
                    }
                    hgl_rust::DeltaEntry::Child(key, value)
                }
            });
        }
        Ok(Value::new(ty, Kind::Delta(entries)))
    }
    fn value_call(
        &mut self,
        module: &str,
        name: &str,
        args: Vec<(Option<String>, Value)>,
        runtime: bool,
    ) -> Result<Value, String> {
        let (owner, item) = hgl_value_types::identity(&self.library, module, name);
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
            return set_call(name, &owner, &item, args);
        }
        if matches!(
            name,
            "valid" | "modified" | "last_modified" | "passivate" | "activate"
        ) {
            return endpoint_call(name, args, runtime);
        }
        self.call(module, name, &args, runtime)
    }
    fn log_info(&mut self, args: &Arguments) -> Result<Value, String> {
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
    ) -> Result<Value, String> {
        if matches!(operands.0, Expr::Null) || matches!(operands.1, Expr::Null) {
            if !matches!(op, "==" | "!=") {
                return Err("null only supports nullable presence comparisons".into());
            }
            let expr = if matches!(operands.0, Expr::Null) {
                operands.1
            } else {
                operands.0
            };
            let value = if let Expr::Name(name) = expr {
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
        hgl_local_check::binary(op, [a, b], &mut self.plan)
    }
}

fn select_eval(
    checker: &Checker,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
) -> Result<(Checker, EvalInputs, Arguments), String> {
    let (owner, item) = hgl_value_types::identity(&checker.library, module, name);
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
            checker.harness_values.clone(),
        );
        match eval_arguments(&mut candidate, module, &decl.module, &signature, args) {
            Ok((plan, values)) => candidates.push((candidate, plan, values)),
            Err(error) => errors.push(error),
        }
    }
    if candidates.len() != 1 {
        return Err(format!(
            "{owner}::{item}: expected one eval signature, found {}; {}",
            candidates.len(),
            errors.join("; ")
        ));
    }
    Ok(candidates.remove(0))
}

pub(crate) fn prepare_evaluation(
    library: Library,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
    values: &Env,
) -> Result<(Plan, Vec<hgl_harness_ir::Argument>), String> {
    let checker = test_checker(library, module, values.clone());
    let (mut checker, plan, mut values) = select_eval(&checker, module, name, args)?;
    let mut prepared = Vec::new();
    checker.static_values.prepared = values.iter().map(|(_, value)| value.clone()).collect();
    for (binding, (name, value)) in values.iter_mut().enumerate() {
        if let Kind::Wire(input) = value.kind {
            let shape = value.ty.clone();
            let entry_type = replay_type(&checker.library, &shape)?;
            let data = Value::new(
                Ty::List(Box::new(entry_type.clone()), None),
                Kind::Prepared(binding),
            );
            prepared.push(hgl_harness_ir::Argument::Dense {
                parameter: name.clone().ok_or("missing checked eval parameter")?,
                binding,
                shape,
                entry_type,
                slots: plan[input].clone(),
            });
            *value = checker.call("hgraph.std", "replay", &[(None, data)], false)?;
        } else {
            prepared.push(hgl_harness_ir::Argument::Constant {
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
        let key = hgl_eval_data::recording_key(&checker.plan);
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

fn replay_type(library: &Library, ty: &Ty) -> Result<Ty, String> {
    let decl = hgl_value_types::declaration(library, "hgraph.std", "TimedValue")?
        .ok_or("eval requires the ordinary TimedValue declaration")?;
    hgl_value_types::specialize(library, decl, vec![ty.clone()], &mut BTreeSet::new())
}

type EvalInputs = Vec<Vec<Option<Value>>>;

fn checked_eval(
    checker: &mut Checker,
    module: &str,
    expr: &Expr,
    expected: Option<&Ty>,
) -> Result<Value, String> {
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
        return Err(format!("expected {}, got {}", ty.name(), value.ty.name()));
    }
    Ok(value)
}

pub(crate) fn prepared_expected(
    library: Library,
    module: &str,
    ty: &Ty,
    slots: &[Option<Expr>],
    values: &Env,
) -> Result<Vec<Option<Value>>, String> {
    let mut checker = test_checker(library, module, values.clone());
    hgl_eval_data::sequence(slots, Some(ty.clone()), |expr, expected| {
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
) -> Result<Arguments, String> {
    let mut values = Vec::new();
    for (index, (label, expr)) in args.iter().enumerate() {
        let parameter = hgl_eval_data::parameter(signature, index, label.as_deref())?;
        if !parameter.constant {
            continue;
        }
        let name = checker.type_sizes(owner, &parameter.ty, &Env::new())?;
        let hint = hgl_value_types::resolve(&checker.library, owner, &name, &mut BTreeSet::new())
            .ok()
            .map(|ty| hgl_value_access::project(&ty));
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
) -> Result<(EvalInputs, Arguments), String> {
    let configuration = eval_configuration(checker, module, owner, signature, args)?;
    let signature = checker.signature_sizes(owner, signature.clone(), &configuration)?;
    let mut plan = Vec::new();
    let mut values = Vec::new();
    for (position, (label, expr)) in args.iter().enumerate() {
        let parameter = hgl_eval_data::parameter(&signature, position, label.as_deref())?;
        let type_name = checker.type_sizes(owner, &parameter.ty, &Env::new())?;
        let hint =
            hgl_value_types::resolve(&checker.library, owner, &type_name, &mut BTreeSet::new())
                .ok()
                .map(|ty| if let Ty::Ref(child) = ty { *child } else { ty });
        let value = if let Expr::Sequence(ticks) = expr {
            if parameter.constant {
                return Err("const parameter requires a fixed value".into());
            }
            let (ty, slots) = hgl_eval_data::sequence(ticks, hint, |expr, expected| {
                checked_eval(checker, module, expr, expected)
            })?;
            let index = plan.len();
            plan.push(slots);
            Value::new(ty, Kind::Wire(index))
        } else {
            if !parameter.constant {
                return Err("temporal eval argument requires a sequence".into());
            }
            configuration
                .iter()
                .find(|(name, _)| name.as_deref() == Some(parameter.name.as_str()))
                .map(|(_, value)| value.clone())
                .ok_or("missing evaluated configuration")?
        };
        values.push((Some(parameter.name.clone()), value));
    }
    let types = hgl_value_bind::signature_types(&checker.library, owner, &signature, &values)?;
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

fn inject_capability(name: &str, node: &mut Node, env: &mut Env) -> Result<(), String> {
    let ty = Ty::Void;
    node.alarm |= name == "alarm";
    node.global_state |= name == "global_state";
    if env
        .insert(name.into(), Value::new(ty, Kind::Capability))
        .is_some()
    {
        return Err(format!(
            "{name}: injectable conflicts with an existing binding"
        ));
    }
    Ok(())
}

fn node_guard(node: &Node, expr: Expr) -> Expr {
    if node.inputs.iter().all(|(_, _, ty)| ty.publication()) {
        expr.handler_guard(
            &node
                .inputs
                .iter()
                .map(|(n, _, _)| n.clone())
                .collect::<Vec<_>>(),
        )
    } else {
        expr
    }
}

#[cfg(test)]
#[path = "../tests/support/nullable.rs"]
mod nullable_tests;

#[cfg(test)]
#[path = "../tests/support/eval_composition.rs"]
mod eval_composition_tests;
