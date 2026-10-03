use crate::index::{Decl, Library, Role, Signature};
use crate::syntax::{Cursor, Expr, Literal, Stmt, Ty};
use hgl_body_check::runtime_body;
use hgl_rust::{Kind, Native, Node, Plan, Statement, Value};
use hgl_value_bind::{bind, method_arguments, order_arguments, resolve_type};
use hgl_value_check::{field, ordinary, writable};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Checker {
    library: Library,
    plan: Plan,
    active: BTreeSet<usize>,
    documented: BTreeSet<usize>,
    test_scope: Option<String>,
    result_hint: Option<Ty>,
    phase: Phase,
    types: BTreeMap<String, Ty>,
    runtime_node: bool,
    value_context: ValueContext,
    generator: bool,
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

pub(crate) fn assertion(library: Library, module: &str, expr: &Expr) -> Result<bool, String> {
    let mut checker = Checker {
        library,
        test_scope: Some(module.into()),
        ..Checker::default()
    };
    let value = checker.expression(module, expr, &Env::new(), false)?;
    let value = checker
        .wiring
        .value(&value)
        .map_err(|error| format!("constant evaluation: {error}"))?;
    let Kind::Literal(Literal::Bool(result)) = value.kind else {
        return Err("ordinary assertion requires bool".into());
    };
    Ok(result)
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
    fn identity(&self, module: &str, name: &str) -> (String, String) {
        hgl_value_types::identity(&self.library, module, name)
    }
    fn call(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Value, String> {
        let folded;
        let args = if runtime {
            args
        } else {
            folded = args
                .iter()
                .map(|(name, value)| {
                    let value = if matches!(value.kind, Kind::Wire(_)) {
                        value.clone()
                    } else {
                        self.wiring_value(value)?
                    };
                    Ok((name.clone(), value))
                })
                .collect::<Result<Vec<_>, String>>()?;
            &folded
        };
        let (module, name) = self.identity(module, name);
        let (mut id, mut signature, values, mut types, result) =
            self.select(&module, &name, args, runtime)?;
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
            return self.direct_call(id, &decl, &signature, (values, types, result), runtime);
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
            let node = self.node(&decl, &signature, &mut cursor, &mut env, &result)?;
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
            self.graph(&module, &mut cursor, &mut env, &result)?
        };
        self.types = previous_types;
        self.active.remove(&id);
        self.test_scope = previous_scope;
        Ok(value)
    }

    fn direct_call(
        &mut self,
        id: usize,
        decl: &Decl,
        signature: &Signature,
        (args, types, result): (Vec<Value>, BTreeMap<String, Ty>, Ty),
        runtime: bool,
    ) -> Result<Value, String> {
        if !self.active.insert(id) {
            return Err("recursive ordinary value calls are unsupported".into());
        }
        let mut env: Env = signature
            .parameters
            .iter()
            .zip(&args)
            .enumerate()
            .map(|(id, (p, v))| (p.name.clone(), Value::new(v.ty.clone(), Kind::Local(id))))
            .collect();
        let previous_types = std::mem::replace(&mut self.types, types);
        let runtime = match self.value_context {
            ValueContext::Outside => runtime,
            ValueContext::Constant => false,
            ValueContext::Runtime => true,
        };
        let previous_context = std::mem::replace(
            &mut self.value_context,
            if runtime {
                ValueContext::Runtime
            } else {
                ValueContext::Constant
            },
        );
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
        let (owner, item) = self.identity(module, name);
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
        self.ordinary_type(&owner, &parameter.ty, &mut BTreeSet::new())
            .ok()
    }
    fn select(
        &self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Selection, String> {
        let mut candidates = Vec::new();
        let mut errors = Vec::new();
        for (id, decl) in self.library.declarations.iter().enumerate() {
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
            let signature = match decl.signature() {
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
                && resolve_type(&signature.result, &declared).is_some_and(|ty| !scalar(&ty))
            {
                errors.push("generator requires a declared scalar output type".into());
                continue;
            }
            match bind(
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
        &self,
        declaration: &Decl,
        signature: &Signature,
        values: &[Value],
        result: &Ty,
        runtime: bool,
    ) -> Result<(usize, Signature, BTreeMap<String, Ty>), String> {
        let module = declaration.module.as_str();
        let name = declaration.name.as_str();
        let mut implementations = Vec::new();
        for (candidate, implementation) in self.library.declarations.iter().enumerate() {
            if implementation.module != module
                || implementation.name != name
                || implementation.role != Role::Implementation
            {
                continue;
            }
            let Ok(sig) = implementation.signature() else {
                continue;
            };
            let positional = values
                .iter()
                .cloned()
                .map(|v| (None, v))
                .collect::<Vec<_>>();
            let Ok((_, inferred, output)) =
                bind(&sig, &positional, runtime, Some(result), &BTreeMap::new())
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
            if !sig.generics.is_empty() {
                let instance = sig
                    .generics
                    .iter()
                    .filter_map(|g| inferred.get(g).map(|t| t.name().to_owned()))
                    .collect::<Vec<_>>();
                if !self.library.instances.iter().any(|(m, n, args)| {
                    m == module
                        && n == name
                        && args.len() == instance.len()
                        && args.iter().zip(&instance).all(|(a, b)| a == "_" || a == b)
                }) {
                    continue;
                }
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
    fn requirement(&self, module: &str, sig: &Signature, inferred: &BTreeMap<String, Ty>) -> bool {
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
            let (owner, item) = self.identity(module, required);
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
        if matches!(value.kind, Kind::Wire(_)) {
            env.insert(name.into(), value);
            return Ok(());
        }
        let id = local(env, name, value.ty.clone(), &mut self.next_wiring_local);
        env.insert(
            name.into(),
            hgl_value_check::binding(id, &value, mutable, annotated)?,
        );
        let value = self.wiring_value(&value)?;
        if matches!(value.kind, Kind::WiringFailure(_)) {
            self.wiring.bind_failed(id, value, mutable);
            return Ok(());
        }
        let statement = if mutable {
            Statement::Var(id, value)
        } else {
            Statement::Let(id, value)
        };
        self.wiring_statement(&statement)
    }
    fn graph(
        &mut self,
        module: &str,
        cursor: &mut Cursor<'_>,
        env: &mut Env,
        result: &Ty,
    ) -> Result<Value, String> {
        let previous_hint = self.result_hint.replace(result.clone());
        let statements = if cursor.take("=>") {
            cursor.lines();
            vec![Stmt::Return(cursor.expr()?)]
        } else {
            cursor.block()?
        };
        cursor.lines();
        if !cursor.at("") {
            return Err("unsupported graph body suffix".into());
        }
        let mut output = Value::new(Ty::Void, Kind::Void);
        for (position, statement) in statements.iter().enumerate() {
            match statement {
                Stmt::Let(name, annotation, expr) | Stmt::Var(name, annotation, expr) => {
                    let value =
                        self.local_initializer(module, annotation.as_deref(), expr, env, false)?;
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
        if *result != Ty::Void
            && (output.ty != *result
                && !matches!(&output.ty,Ty::Ref(child) if child.as_ref()==result)
                || !matches!(output.kind, Kind::Wire(_)))
        {
            return Err("graph return type mismatch".into());
        }
        self.result_hint = previous_hint;
        Ok(if *result == Ty::Void {
            Value::new(Ty::Void, Kind::Void)
        } else {
            output.ty = result.clone();
            output
        })
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
        if !scalar(&node.result) {
            return Err("generator requires a declared scalar output type".into());
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
                    name @ ("alarm" | "clock" | "logger" | "global_state" | "replay_input"
                    | "capture") => {
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
        if matches!(&node.capability, Some((name, _)) if name == "replay_input") && !node.alarm {
            return Err("replay_input: source requires alarm".into());
        }
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
        let ty = annotation
            .map(|name| {
                resolve_type(name, &self.types).map_or_else(
                    || {
                        hgl_value_types::substitute(
                            &self.library,
                            module,
                            name,
                            &self.types,
                            &mut BTreeSet::new(),
                        )
                    },
                    Ok,
                )
            })
            .transpose()?;
        let value = self.expected_expression(module, expr, env, runtime, ty.as_ref())?;
        if value.ty == Ty::Void {
            return Err("statement operation has no initializer value".into());
        }
        if !matches!(value.ty, Ty::Nullable(_)) {
            require_payload(&value)?;
        }
        if ty.as_ref().is_some_and(|ty| *ty != value.ty) {
            return Err("local initializer type mismatch".into());
        }
        Ok(value)
    }
    fn struct_declaration(&self, module: &str, name: &str) -> Result<Option<&Decl>, String> {
        hgl_value_types::declaration(&self.library, module, name)
    }
    fn ordinary_type(
        &self,
        module: &str,
        name: &str,
        active: &mut BTreeSet<String>,
    ) -> Result<Ty, String> {
        hgl_value_types::resolve(&self.library, module, name, active)
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
        if let Expr::Sequence(elements) = expr {
            return hgl_value_check::list_literal(elements, expected);
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
                if mutable {
                    require_payload(&value)?;
                }
                if mutable && !ordinary(&value.ty) {
                    return Err(
                        "mutable locals currently require an ordinary scalar or struct".into(),
                    );
                }
                let id = local(env, name, value.ty.clone(), next_local);
                let binding = hgl_value_check::binding(id, &value, mutable, annotation.is_some())?;
                let borrowed = matches!(binding.kind, Kind::BorrowedLocal(..));
                env.insert(name.clone(), binding);
                if borrowed {
                    Statement::Borrow(id, value, mutable)
                } else if mutable {
                    Statement::Var(id, value)
                } else {
                    Statement::Let(id, value)
                }
            }
            Stmt::Exit => {
                if self.phase != Phase::Evaluation || !self.runtime_node {
                    return Err("bare return requires runtime evaluation".into());
                }
                Statement::Exit
            }
            Stmt::Return(expr) => {
                if self.generator && self.value_context == ValueContext::Outside {
                    return Err("generator return cannot carry a value; use yield".into());
                }
                let v = self.expected_expression(module, expr, env, true, Some(result))?;
                if &v.ty != result || matches!(result, Ty::Void | Ty::Set(_)) {
                    return Err("node return type mismatch".into());
                }
                require_payload(&v)?;
                if self.value_context == ValueContext::Outside {
                    Statement::Return(v)
                } else {
                    Statement::Yield(v)
                }
            }
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
        let payload = self.expected_expression(module, payload, env, true, Some(result))?;
        require_payload(&payload)?;
        if payload.ty != *result {
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
        if (self.phase != Phase::Evaluation && matches!(target.kind, Kind::Output))
            || !(matches!(target.kind, Kind::Output | Kind::Cache(_)) || writable(&target))
        {
            return Err("assignment requires writable var, state, cache or out".into());
        }
        let value = self.expected_expression(module, expr, env, true, Some(&target.ty))?;
        if target.ty != value.ty || !ordinary(&target.ty) {
            return Err("assignment type mismatch".into());
        }
        require_payload(&value)?;
        Ok(Statement::Assign(target, value))
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
            Expr::Null => Err("null requires a contextual nullable comparison".into()),
            Expr::Property(receiver, name) => {
                if injected_clock(receiver, env) {
                    return clock_property(receiver, name, env, runtime);
                }
                let parent = self.expression(module, receiver, env, runtime)?;
                field(parent, name)
            }
            Expr::Index(receiver, index) => {
                if !matches!(receiver.as_ref(), Expr::Name(name) if name == "replay_input") {
                    let parent = self.expression(module, receiver, env, runtime)?;
                    let index = self.expression(module, index, env, runtime)?;
                    return hgl_value_check::indexed(parent, index);
                }
                if !matches!(receiver.as_ref(), Expr::Name(name) if name == "replay_input")
                    || !runtime
                    || !self.runtime_node
                    || self.phase != Phase::Evaluation
                {
                    return Err(
                        "replay_input: indexing requires the injected source in evaluation".into(),
                    );
                }
                let payload = capability_payload("replay_input", env)?;
                let index = self.expression(module, index, env, runtime)?;
                if index.ty != Ty::I64 {
                    return Err("replay_input: index requires i64".into());
                }
                Ok(Value::new(
                    Ty::Nullable(Box::new(payload)),
                    Kind::ReplaySlot(Box::new(index)),
                ))
            }
            Expr::Sequence(elements) => hgl_value_check::list_literal(elements, None),
            Expr::Literal(l) => Ok(Value::new(l.ty(), Kind::Literal(l.clone()))),
            Expr::Name(name) => {
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
            return capability_function(receiver, name, &values, env, self.phase);
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
        if !scalar(&value.ty) {
            return Err("delta_value: structural delta contract is not admitted".into());
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
            value.ty.clone(),
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
        let mut check = hgl_struct_check::Constructor::new(
            &self.library,
            module,
            name,
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
        check.finish(&self.library)
    }
    fn value_call(
        &mut self,
        module: &str,
        name: &str,
        args: Vec<(Option<String>, Value)>,
        runtime: bool,
    ) -> Result<Value, String> {
        let (owner, item) = self.identity(module, name);
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
        let mut a = self.expression(module, operands.0, env, runtime)?;
        let previous = self.facts.clone();
        if matches!(op, "&&" | "||") {
            self.facts = facts(&a, op == "&&", &self.facts);
        }
        let mut b = self.expression(module, operands.1, env, runtime)?;
        self.facts = previous;
        require_payload(&a)?;
        require_payload(&b)?;
        if a.ty == Ty::I64 && b.ty == Ty::F64 {
            a = Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(a)));
        }
        if a.ty == Ty::F64 && b.ty == Ty::I64 {
            b = Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(b)));
        }
        let ty = hgl_value_check::binary_type(op, &a.ty, &b.ty)?;
        Ok(Value::new(
            ty,
            Kind::Binary(op.to_owned(), Box::new(a), Box::new(b)),
        ))
    }
}

pub(crate) fn evaluate(
    library: Library,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
) -> Result<Plan, String> {
    let mut checker = Checker {
        library,
        test_scope: Some(module.into()),
        ..Checker::default()
    };
    let (owner, item) = checker.identity(module, name);
    let helpers =
        checker.library.declarations.iter().any(|d| {
            d.module == owner && d.name == item && d.test_only && d.role == Role::Function
        });
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for decl in &checker.library.declarations {
        if decl.module != owner
            || decl.name != item
            || matches!(
                decl.role,
                Role::Implementation | Role::Test | Role::Native | Role::Struct
            )
            || (decl.test_only && owner != module)
            || (helpers && owner == module && !decl.test_only)
        {
            continue;
        }
        let signature = decl.signature()?;
        match eval_arguments(&signature, args) {
            Ok((plan, values)) => candidates.push((plan, values)),
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
    let (plan, mut values) = candidates.remove(0);
    checker.plan.input_length = plan.input_length;
    for (_, value) in &mut values {
        if let Kind::Wire(input) = value.kind {
            checker.result_hint = Some(value.ty.clone());
            *value = checker.call("hgraph.std", "replay", &[], false)?;
            let Kind::Wire(node) = value.kind else {
                return Err("replay requires a temporal result".into());
            };
            checker
                .plan
                .replay_inputs
                .push((node, plan.replay_inputs[input].1.clone()));
        }
    }
    checker.result_hint = None;
    let output = checker.call(module, name, &values, false)?;
    if matches!(output.kind, Kind::Wire(_)) {
        if !scalar(&output.ty) {
            return Err("eval currently records scalar outputs".into());
        }
        let ty = output.ty.clone();
        checker.call("hgraph.std", "record", &[(None, output)], false)?;
        checker.plan.output = Some((checker.plan.nodes.len() - 1, ty));
    }
    Ok(checker.plan)
}

fn set_call(
    name: &str,
    owner: &str,
    item: &str,
    args: Vec<(Option<String>, Value)>,
) -> Result<Value, String> {
    let op = if owner == "hgraph.native" { item } else { name };
    let first = args
        .first()
        .ok_or("set operation needs an endpoint")?
        .1
        .clone();
    let Ty::Set(element) = &first.ty else {
        return Err("set operation needs a set".into());
    };
    if !matches!(element.as_ref(), Ty::I64 | Ty::Bool) {
        return Err("set keys currently require bool or i64".into());
    }
    if matches!(op, "upsert" | "discard") && !matches!(first.kind, Kind::Output) {
        return Err("set mutation requires out".into());
    }
    if matches!(op, "bound" | "len" | "contains") && !matches!(first.kind, Kind::Input(..)) {
        return Err("set observation requires an input".into());
    }
    let n = if matches!(op, "bound" | "len") { 1 } else { 2 };
    if args.len() != n || (n == 2 && args[1].1.ty != **element) {
        return Err("set argument type mismatch".into());
    }
    let ty = match op {
        "bound" | "contains" => Ty::Bool,
        "len" => Ty::I64,
        _ => Ty::Void,
    };
    Ok(Value {
        ty,
        kind: Kind::Query(
            format!("set_{op}"),
            args.into_iter().map(|(_, v)| v).collect(),
        ),
    })
}

fn local(env: &mut Env, name: &str, ty: Ty, next: &mut usize) -> usize {
    let id = *next;
    *next += 1;
    env.insert(name.into(), Value::new(ty, Kind::Local(id)));
    id
}

fn eval_arguments(
    signature: &Signature,
    args: &[(Option<String>, Expr)],
) -> Result<(Plan, Arguments), String> {
    let mut plan = Plan::default();
    let mut values = Vec::new();
    for (position, (label, expr)) in args.iter().enumerate() {
        let parameter = if let Some(label) = label {
            signature.parameters.iter().find(|p| p.name == *label)
        } else {
            signature.parameters.get(position)
        }
        .ok_or("unknown eval argument")?;
        let hint = Ty::parse(&parameter.ty);
        let value = if let Expr::Sequence(ticks) = expr {
            if parameter.constant {
                return Err("const parameter requires a fixed value".into());
            }
            let mut literals = ticks
                .iter()
                .map(|e| {
                    e.as_ref()
                        .map(|e| {
                            e.fixed()
                                .ok_or("eval elements must be fixed values".to_owned())
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ty = hint
                .or_else(|| literals.iter().flatten().next().map(Literal::ty))
                .ok_or("cannot infer empty generic sequence")?;
            coerce_sequence(&mut literals, &ty).map_err(|e| format!("{}: {e}", parameter.name))?;
            let index = plan.replay_inputs.len();
            plan.input_length = plan.input_length.max(literals.len());
            plan.replay_inputs.push((index, literals));
            Value::new(ty, Kind::Wire(index))
        } else {
            if !parameter.constant {
                return Err("temporal eval argument requires a sequence".into());
            }
            let mut value = expr
                .fixed()
                .ok_or("const eval argument requires a fixed expression")?;
            if let Some(ty) = hint {
                coerce_literal(&mut value, &ty)?;
            }
            Value::new(value.ty(), Kind::Literal(value))
        };
        values.push((label.clone(), value));
    }
    bind(signature, &values, false, None, &BTreeMap::new())?;
    Ok((plan, values))
}
pub(crate) fn coerce_sequence(values: &mut [Option<Literal>], ty: &Ty) -> Result<(), String> {
    for value in values.iter_mut().flatten() {
        coerce_literal(value, ty)?;
    }
    Ok(())
}
#[expect(
    clippy::cast_precision_loss,
    reason = "HGL contextual integer literals in f64 positions use IEEE conversion"
)]
fn coerce_literal(value: &mut Literal, ty: &Ty) -> Result<(), String> {
    if *ty == Ty::F64
        && let Literal::Int(i) = value
    {
        *value = Literal::Float(*i as f64);
    }
    if value.ty() != *ty {
        return Err(format!("expected {}, got {}", ty.name(), value.ty().name()));
    }
    Ok(())
}

fn scalar(ty: &Ty) -> bool {
    !matches!(
        ty,
        Ty::Void | Ty::Ref(_) | Ty::Set(_) | Ty::Nullable(_) | Ty::Struct(..) | Ty::List(..)
    )
}

fn endpoint_metadata(name: &str) -> bool {
    matches!(
        name,
        "valid" | "modified" | "last_modified" | "activate" | "passivate"
    )
}

fn injected_clock(expr: &Expr, env: &Env) -> bool {
    matches!(expr, Expr::Name(name) if name == "clock" && !env.get(name).is_some_and(|v| matches!(v.ty, Ty::Struct(..))))
}
fn clock_property(receiver: &Expr, name: &str, env: &Env, runtime: bool) -> Result<Value, String> {
    if !matches!(receiver, Expr::Name(receiver) if receiver == "clock") || !runtime {
        return Err("clock property requires the direct injected clock in a runtime hook".into());
    }
    capability_payload("clock", env)?;
    match name {
        "evaluation_time" | "next_cycle_evaluation_time" => Ok(Value::new(
            Ty::DateTime,
            Kind::Query(format!("clock.{name}"), Vec::new()),
        )),
        "now" => Err("clock.now: wall-clock observations are not supported by this backend".into()),
        _ => Err(format!("clock: unknown property {name}")),
    }
}

fn require_payload(value: &Value) -> Result<(), String> {
    if matches!(value.kind, Kind::Input(_, true)) {
        return Err("signal has no ordinary scalar value".into());
    }
    if matches!(value.ty, Ty::Nullable(_)) {
        return Err("nullable replay result requires presence proof before payload use".into());
    }
    Ok(())
}
fn capability_payload(receiver: &str, env: &Env) -> Result<Ty, String> {
    env.get(receiver)
        .filter(|v| matches!(v.kind, Kind::Capability))
        .map(|v| v.ty.clone())
        .ok_or_else(|| format!("{receiver}: missing inject {receiver}"))
}

fn capability_function(
    receiver: &str,
    method: &str,
    args: &Arguments,
    env: &Env,
    phase: Phase,
) -> Result<Value, String> {
    let name = format!("{method}({receiver})");
    let payload = capability_payload(receiver, env)?;
    let (parameters, result) = match (receiver, method) {
        ("replay_input", "len") if phase != Phase::Stop => (vec![], Ty::I64),
        ("capture", "begin") if phase == Phase::Start => (vec![], Ty::Void),
        ("capture", "append") if phase == Phase::Evaluation => {
            (vec![("time", Ty::DateTime), ("delta", payload)], Ty::Void)
        }
        ("alarm", "schedule") if phase != Phase::Stop => (vec![("delay", Ty::Duration)], Ty::Void),
        ("alarm", "schedule_at") if phase != Phase::Stop => {
            (vec![("time", Ty::DateTime)], Ty::Void)
        }
        _ => {
            return Err(format!(
                "{name}: unknown capability operation or forbidden hook phase"
            ));
        }
    };
    let values = method_arguments(&name, &parameters, args)?;
    if receiver == "alarm"
        && method == "schedule"
        && matches!(values[0].kind, Kind::Literal(Literal::Duration(delay)) if delay < 0)
    {
        return Err("negative alarm delay".into());
    }
    Ok(Value::new(
        result,
        Kind::Query(format!("{receiver}.{method}"), values),
    ))
}

fn inject_capability(name: &str, node: &mut Node, env: &mut Env) -> Result<(), String> {
    let ty = match name {
        "replay_input" if node.inputs.is_empty() && scalar(&node.result) => node.result.clone(),
        "capture"
            if node.result == Ty::Void
                && node.inputs.len() == 1
                && scalar(&node.inputs[0].2)
                && !env.values().any(|v| matches!(v.kind, Kind::Input(_, true))) =>
        {
            node.inputs[0].2.clone()
        }
        "alarm" | "clock" | "logger" | "global_state" => Ty::Void,
        _ => return Err(format!("{name}: unsupported node shape or scalar type")),
    };
    if matches!(name, "replay_input" | "capture")
        && node.capability.replace((name.into(), ty.clone())).is_some()
    {
        return Err(format!("{name}: only one storage capability is admitted"));
    }
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

fn endpoint_call(name: &str, args: Arguments, runtime: bool) -> Result<Value, String> {
    if !runtime
        || args.is_empty()
        || args
            .iter()
            .any(|(n, v)| n.is_some() || !matches!(v.kind, Kind::Input(..) | Kind::Output))
    {
        return Err("endpoint query requires runtime endpoints".into());
    }
    if matches!(name, "last_modified" | "activate" | "passivate") && args.len() != 1 {
        return Err("endpoint operation requires exactly one argument".into());
    }
    if matches!(name, "activate" | "passivate")
        && args.iter().any(|(_, v)| {
            !matches!(v.kind, Kind::Input(..)) || matches!(v.ty, Ty::Ref(_) | Ty::Set(_))
        })
    {
        return Err("activity requires a scalar input".into());
    }
    if args.iter().any(|(_, v)| {
        matches!(v.kind, Kind::Output) && matches!(v.ty, Ty::Ref(_) | Ty::Set(_) | Ty::Void)
    }) {
        return Err("querying structural out is not yet supported".into());
    }
    let ty = match name {
        "last_modified" => Ty::DateTime,
        "activate" | "passivate" => Ty::Void,
        _ => Ty::Bool,
    };
    Ok(Value::new(
        ty,
        Kind::Query(name.to_owned(), args.into_iter().map(|(_, v)| v).collect()),
    ))
}

fn terminates(body: &[Statement]) -> bool {
    body.iter().any(|statement| match statement {
        Statement::Return(_) | Statement::Yield(_) | Statement::Exit => true,
        Statement::If(_, yes, no) => terminates(yes) && terminates(no),
        Statement::Let(..)
        | Statement::Call(_)
        | Statement::Var(..)
        | Statement::Borrow(..)
        | Statement::Assign(..)
        | Statement::For(..)
        | Statement::While(..)
        | Statement::TimedYield(..) => false,
    })
}
fn facts(
    value: &Value,
    truth: bool,
    incoming: &BTreeSet<(String, usize)>,
) -> BTreeSet<(String, usize)> {
    if let Kind::Unary(op, value) = &value.kind
        && op == "!"
    {
        return facts(value, !truth, incoming);
    }
    if let Kind::Binary(op, a, b) = &value.kind
        && matches!(op.as_str(), "&&" | "||")
    {
        let short = op == "||";
        let continuing = facts(b, truth, &facts(a, !short, incoming));
        return if truth == short {
            merge_facts(&facts(a, truth, incoming), &continuing)
        } else {
            continuing
        };
    }
    let mut result = incoming.clone();
    if let Kind::IsPresent(value) = &value.kind
        && let Kind::Local(id) = value.kind
    {
        result.insert((if truth { "present" } else { "absent" }.into(), id));
    }
    if let Kind::Query(op, args) = &value.kind
        && truth
        && (op == "valid" || (op == "modified" && args.len() == 1))
    {
        result.extend(args.iter().filter_map(|v| {
            if let Kind::Input(id, _) = v.kind {
                Some((op.clone(), id))
            } else {
                None
            }
        }));
    }
    result
}
fn merge_facts(
    a: &BTreeSet<(String, usize)>,
    b: &BTreeSet<(String, usize)>,
) -> BTreeSet<(String, usize)> {
    let reachable = |facts: &BTreeSet<(String, usize)>| {
        !facts
            .iter()
            .any(|(kind, id)| kind == "present" && facts.contains(&("absent".into(), *id)))
    };
    if !reachable(a) {
        return b.clone();
    }
    if !reachable(b) {
        return a.clone();
    }
    a.intersection(b).cloned().collect()
}

fn handler_facts(guard: Option<&Value>, inputs: usize) -> BTreeSet<(String, usize)> {
    if let Some(guard) = guard {
        return facts(guard, true, &BTreeSet::new());
    }
    let mut facts = (0..inputs)
        .map(|i| ("valid".into(), i))
        .collect::<BTreeSet<_>>();
    if inputs == 1 {
        facts.insert(("modified".into(), 0));
    }
    facts
}

fn node_guard(node: &Node, expr: Expr) -> Expr {
    if node.inputs.iter().all(|(_, _, ty)| scalar(ty)) {
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
