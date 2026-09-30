use crate::index::{Decl, Library, Role, Signature};
use crate::syntax::{Cursor, Expr, Literal, Stmt, Ty};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub(crate) struct Value {
    pub ty: Ty,
    pub kind: Kind,
}
#[derive(Debug, Clone)]
pub(crate) enum Kind {
    Literal(Literal),
    Wire(usize),
    Input(usize),
    Cache(usize),
    Local(usize),
    Native(usize, Vec<Value>),
    Binary(String, Box<Value>, Box<Value>),
    Unary(String, Box<Value>),
    Query(String, Vec<Value>),
    Output,
    Void,
}
#[derive(Debug)]
pub(crate) enum Statement {
    Let(usize, Value),
    Return(Value),
    Call(Value),
    Add(usize, Value),
    Assign(Value, Value),
    For(usize, Value, Vec<Self>),
    If(Value, Vec<Self>, Vec<Self>),
}
#[derive(Debug)]
pub(crate) struct Node {
    pub name: String,
    pub inputs: Vec<(String, usize, Ty)>,
    pub result: Ty,
    pub alarm: bool,
    pub delays: Vec<i64>,
    pub caches: Vec<Literal>,
    pub handlers: Vec<(Option<Value>, Vec<Statement>)>,
    pub replay: Option<Vec<Option<Literal>>>,
}
#[derive(Debug)]
pub(crate) struct Native {
    pub name: String,
    pub method: String,
    pub throws: bool,
    pub args: Vec<Ty>,
    pub result: Ty,
}
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub nodes: Vec<Node>,
    pub natives: Vec<Native>,
    pub docs: Vec<String>,
    pub output: Option<(usize, Ty)>,
    pub input_length: usize,
}
struct Checker {
    library: Library,
    plan: Plan,
    active: BTreeSet<usize>,
    documented: BTreeSet<usize>,
    test_scope: Option<String>,
    result_hint: Option<Ty>,
}
type Bound = (Vec<Value>, BTreeMap<String, Ty>, Ty);
type Selection = (usize, Signature, Vec<Value>, BTreeMap<String, Ty>, Ty);
type Env = BTreeMap<String, Value>;
type Arguments = Vec<(Option<String>, Value)>;

pub(crate) fn compile(library: Library, module: &str, entry: &str) -> Result<Plan, String> {
    let mut checker = Checker {
        library,
        plan: Plan::default(),
        active: BTreeSet::new(),
        documented: BTreeSet::new(),
        test_scope: None,
        result_hint: None,
    };
    checker.call(module, entry, &[], false)?;
    Ok(checker.plan)
}
impl Checker {
    fn identity(&self, module: &str, name: &str) -> (String, String) {
        if let Some((alias, item)) = name.split_once("::") {
            return (
                self.library
                    .imports
                    .get(&(module.into(), alias.into()))
                    .cloned()
                    .unwrap_or_else(|| alias.into()),
                item.into(),
            );
        }
        if let Some(target) = self.library.imports.get(&(module.into(), name.into()))
            && let Some((owner, item)) = target.rsplit_once("::")
        {
            return (owner.into(), item.into());
        }
        (module.into(), name.into())
    }
    fn call(
        &mut self,
        module: &str,
        name: &str,
        args: &[(Option<String>, Value)],
        runtime: bool,
    ) -> Result<Value, String> {
        let (module, name) = self.identity(module, name);
        let (mut id, mut signature, values, _types, result) =
            self.select(&module, &name, args, runtime)?;
        let mut decl = self.library.declarations[id].clone();
        self.document(id);
        if decl.role == Role::Operator {
            (id, signature) = self.implementation(&decl, &signature, &values, &result, runtime)?;
            decl = self.library.declarations[id].clone();
            self.document(id);
        }
        if decl.role == Role::Native {
            return self.native(&decl, &signature, values, result, runtime);
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
        let mut cursor = Cursor::new(&signature.body);
        let value = if signature.value_function || signature.body.iter().any(|t| t.text == "when") {
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
        self.active.remove(&id);
        self.test_scope = previous_scope;
        Ok(value)
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
                || matches!(decl.role, Role::Implementation | Role::Test)
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
            match bind(
                &signature,
                args,
                runtime || decl.role == Role::Native,
                self.result_hint.as_ref(),
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
    ) -> Result<(usize, Signature), String> {
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
            let Ok((_, inferred, output)) = bind(&sig, &positional, runtime, Some(result)) else {
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
            implementations.push((candidate, sig));
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
                        .map(|ty| {
                            (
                                None,
                                Value {
                                    ty,
                                    kind: Kind::Literal(Literal::Int(0)),
                                },
                            )
                        })
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
    fn native(
        &mut self,
        declaration: &Decl,
        signature: &Signature,
        values: Vec<Value>,
        result: Ty,
        runtime: bool,
    ) -> Result<Value, String> {
        let module = declaration.module.as_str();
        let name = declaration.name.as_str();
        if !runtime {
            return Err("native value calls in composition are not yet supported".into());
        }
        let selected = self
            .library
            .declarations
            .iter()
            .filter(|d| d.module == module && d.name == name && d.role == Role::Native)
            .filter_map(|d| d.signature().ok())
            .filter(|s| {
                s.generics.is_empty()
                    && s.parameters.len() == signature.parameters.len()
                    && s.parameters
                        .iter()
                        .zip(&signature.parameters)
                        .all(|(a, b)| a.name == b.name && a.ty == b.ty && a.constant == b.constant)
                    && s.throws == signature.throws
                    && s.result == signature.result
                    && !s.body.is_empty()
            })
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(format!(
                "{module}::{name}: expected one selected native implementation"
            ));
        }
        let mut body = Cursor::new(&selected[0].body);
        body.need("{")?;
        body.lines();
        body.need("}")?;
        body.lines();
        if !body.at("") {
            return Err("unsupported native implementation".into());
        }
        let method = format!(
            "{}_{}",
            name,
            values
                .iter()
                .map(|v| v.ty.name())
                .collect::<Vec<_>>()
                .join("_")
        );
        let full_name = format!("{module}::{name}");
        let native = self
            .plan
            .natives
            .iter()
            .position(|n| {
                n.name == full_name
                    && n.args == values.iter().map(|v| v.ty.clone()).collect::<Vec<_>>()
            })
            .unwrap_or(self.plan.natives.len());
        if native == self.plan.natives.len() {
            if self.plan.natives.iter().any(|n| n.method == method) {
                return Err("native Rust binding name collision".into());
            }
            self.plan.natives.push(Native {
                name: full_name,
                method,
                throws: signature.throws,
                args: values.iter().map(|v| v.ty.clone()).collect(),
                result: result.clone(),
            });
        }
        Ok(Value {
            ty: result,
            kind: Kind::Native(native, values),
        })
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
        let mut output = Value {
            ty: Ty::Void,
            kind: Kind::Void,
        };
        for (position, statement) in statements.iter().enumerate() {
            match statement {
                Stmt::Let(name, expr) => {
                    let value = self.expression(module, expr, env, false)?;
                    if env.insert(name.clone(), value).is_some() {
                        return Err(format!("duplicate local {name}"));
                    }
                }
                Stmt::Call(expr) => {
                    output = self.expression(module, expr, env, false)?;
                }
                Stmt::Return(expr) => {
                    if position + 1 != statements.len() {
                        return Err("statements after return".into());
                    }
                    output = self.expression(module, expr, env, false)?;
                }
                Stmt::Add(_, _) | Stmt::Assign(_, _) | Stmt::For(..) | Stmt::If(_, _, _) => {
                    return Err("runtime statement in graph".into());
                }
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
            Value {
                ty: Ty::Void,
                kind: Kind::Void,
            }
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
        let mut node = Node {
            name: format!("{}::{}", decl.module, decl.name),
            inputs: Vec::new(),
            result: result.clone(),
            alarm: false,
            delays: Vec::new(),
            caches: Vec::new(),
            handlers: Vec::new(),
            replay: None,
        };
        node_inputs(signature, env, &mut node)?;
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
        while c.take("when") {
            if c.at("scheduled") {
                return Err(
                    "expected { or a supported endpoint guard; scheduled() is not an alarm guard"
                        .into(),
                );
            }
            let guard = if c.at("{") {
                None
            } else {
                let condition = self.expression(&decl.module, &c.expr()?, env, true)?;
                if condition.ty != Ty::Bool {
                    return Err("handler guard requires bool".into());
                }
                Some(condition)
            };
            let body = c.block()?;
            node.handlers.push((
                guard,
                self.statements(&decl.module, &body, env, result, &mut next_local)?,
            ));
            c.lines();
        }
        if node.handlers.is_empty() {
            return Err("expected when handler".into());
        }
        c.need("}")?;
        c.lines();
        if !c.at("") {
            return Err("unsupported node body suffix".into());
        }
        Ok(node)
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
                    "alarm" => node.alarm = true,
                    "logger" => {
                        env.insert(
                            "logger".into(),
                            Value {
                                ty: Ty::Void,
                                kind: Kind::Void,
                            },
                        );
                    }
                    "out" => {
                        env.insert(
                            "out".into(),
                            Value {
                                ty: result.clone(),
                                kind: Kind::Output,
                            },
                        );
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
                .insert(
                    name,
                    Value {
                        ty,
                        kind: Kind::Cache(index),
                    },
                )
                .is_some()
            {
                return Err("duplicate cache name".into());
            }
            c.lines();
        }
        if c.take("start") {
            if !node.alarm {
                return Err("start scheduling requires inject alarm".into());
            }
            c.need("{")?;
            c.lines();
            while !c.take("}") {
                c.need("alarm")?;
                c.need(".")?;
                c.need("schedule")?;
                c.need("(")?;
                let value = self.expression(&decl.module, &c.expr()?, env, false)?;
                let Kind::Literal(Literal::Duration(delay)) = value.kind else {
                    return Err("alarm delay must be a fixed duration".into());
                };
                if delay < 0 {
                    return Err("negative alarm delay".into());
                }
                node.delays.push(delay);
                c.need(")")?;
                c.lines();
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
        statements
            .iter()
            .map(|statement| self.statement(module, statement, env, result, next_local))
            .collect()
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
            Stmt::Let(name, expr) => {
                let value = self.expression(module, expr, env, true)?;
                let id = local(env, name, value.ty.clone(), next_local)?;
                Statement::Let(id, value)
            }
            Stmt::Return(expr) => {
                let v = self.expression(module, expr, env, true)?;
                if &v.ty != result || matches!(result, Ty::Void | Ty::Set(_)) {
                    return Err("node return type mismatch".into());
                }
                Statement::Return(v)
            }
            Stmt::Call(expr) => Statement::Call(self.expression(module, expr, env, true)?),
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
                scope.insert(
                    name.clone(),
                    Value {
                        ty: *element.clone(),
                        kind: Kind::Local(id),
                    },
                );
                Statement::For(
                    id,
                    collection,
                    self.statements(module, body, &mut scope, result, next_local)?,
                )
            }
            Stmt::Assign(name, expr) => {
                let target = env.get(name).ok_or("unknown assignment target")?.clone();
                if !matches!(target.kind, Kind::Output | Kind::Cache(_)) {
                    return Err("assignment requires state, cache or out".into());
                }
                let value = self.expression(module, expr, env, true)?;
                if target.ty != value.ty || matches!(target.ty, Ty::Ref(_) | Ty::Set(_)) {
                    return Err("assignment type mismatch".into());
                }
                Statement::Assign(target, value)
            }
            Stmt::Add(name, expr) => {
                let Some(Value {
                    kind: Kind::Cache(id),
                    ..
                }) = env.get(name)
                else {
                    return Err("assignment requires a cache variable".into());
                };
                let id = *id;
                let v = self.expression(module, expr, env, true)?;
                if v.ty != Ty::I64 {
                    return Err("cache increment requires i64".into());
                }
                Statement::Add(id, v)
            }
            Stmt::If(expr, yes, no) => {
                let condition = self.expression(module, expr, env, true)?;
                if condition.ty != Ty::Bool {
                    return Err("condition requires bool".into());
                }
                Statement::If(
                    condition,
                    self.statements(module, yes, &mut env.clone(), result, next_local)?,
                    self.statements(module, no, &mut env.clone(), result, next_local)?,
                )
            }
        })
    }
    fn expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
    ) -> Result<Value, String> {
        match expr {
            Expr::Sequence(_) => Err("harness sequences are only valid in eval".into()),
            Expr::Literal(l) => Ok(Value {
                ty: l.ty(),
                kind: Kind::Literal(l.clone()),
            }),
            Expr::Name(name) => env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown value {name}")),
            Expr::Unary(op, expr) => {
                let value = self.expression(module, expr, env, runtime)?;
                if (op == "!" && value.ty != Ty::Bool)
                    || (op == "-" && !matches!(value.ty, Ty::I64 | Ty::F64))
                {
                    return Err("unary type mismatch".into());
                }
                Ok(Value {
                    ty: value.ty.clone(),
                    kind: Kind::Unary(op.clone(), Box::new(value)),
                })
            }
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
        if name == "elements" {
            if args.len() != 2 || !matches!(&args[1].1,Expr::Name(n) if n=="added") {
                return Err("elements currently requires added".into());
            }
            let collection = self.expression(module, &args[0].1, env, runtime)?;
            if !matches!(collection.ty, Ty::Set(_)) || !matches!(collection.kind, Kind::Input(_)) {
                return Err("elements requires a set input".into());
            }
            return Ok(collection);
        }
        let args = args
            .iter()
            .map(|(n, v)| Ok((n.clone(), self.expression(module, v, env, runtime)?)))
            .collect::<Result<Vec<_>, String>>()?;
        self.value_call(module, name, args, env, runtime)
    }
    fn value_call(
        &mut self,
        module: &str,
        name: &str,
        args: Vec<(Option<String>, Value)>,
        env: &Env,
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
        if name == "logger.info" {
            if !env.contains_key("logger") || args.len() != 1 || args[0].1.ty != Ty::Str {
                return Err("logger.info requires inject logger and text".into());
            }
            let id = self
                .plan
                .natives
                .iter()
                .position(|n| n.method == "log_info_str")
                .unwrap_or(self.plan.natives.len());
            if id == self.plan.natives.len() {
                self.plan.natives.push(Native {
                    name: "logger.info".into(),
                    method: "log_info_str".into(),
                    args: vec![Ty::Str],
                    result: Ty::Void,
                    throws: false,
                });
            }
            return Ok(Value {
                ty: Ty::Void,
                kind: Kind::Native(id, args.into_iter().map(|(_, v)| v).collect()),
            });
        }
        if matches!(
            name,
            "valid" | "modified" | "last_modified" | "passivate" | "activate"
        ) {
            if !runtime
                || args.is_empty()
                || args
                    .iter()
                    .any(|(n, v)| n.is_some() || !matches!(v.kind, Kind::Input(_) | Kind::Output))
            {
                return Err("endpoint query requires runtime endpoints".into());
            }
            if matches!(name, "last_modified" | "activate" | "passivate") && args.len() != 1 {
                return Err("endpoint operation requires exactly one argument".into());
            }
            if matches!(name, "activate" | "passivate")
                && args.iter().any(|(_, v)| {
                    !matches!(v.kind, Kind::Input(_)) || matches!(v.ty, Ty::Ref(_) | Ty::Set(_))
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
            return Ok(Value {
                ty,
                kind: Kind::Query(name.to_owned(), args.into_iter().map(|(_, v)| v).collect()),
            });
        }
        self.call(module, name, &args, runtime)
    }
    fn binary(
        &mut self,
        module: &str,
        op: &str,
        operands: (&Expr, &Expr),
        env: &Env,
        runtime: bool,
    ) -> Result<Value, String> {
        let mut a = self.expression(module, operands.0, env, runtime)?;
        let mut b = self.expression(module, operands.1, env, runtime)?;
        if a.ty == Ty::I64 && b.ty == Ty::F64 {
            a = Value {
                ty: Ty::F64,
                kind: Kind::Unary("float".into(), Box::new(a)),
            };
        }
        if a.ty == Ty::F64 && b.ty == Ty::I64 {
            b = Value {
                ty: Ty::F64,
                kind: Kind::Unary("float".into(), Box::new(b)),
            };
        }
        if !runtime {
            return Err("fixed argument arithmetic is not yet supported".into());
        }
        if a.ty != b.ty {
            return Err("binary operand type mismatch".into());
        }
        let ty = match op {
            "+" if matches!(a.ty, Ty::Str | Ty::I64 | Ty::F64) => a.ty.clone(),
            "/" if matches!(a.ty, Ty::I64 | Ty::F64) => Ty::F64,
            "-" | "*" | "%" if matches!(a.ty, Ty::I64 | Ty::F64) => a.ty.clone(),
            ">" | "<" | ">=" | "<="
                if matches!(
                    a.ty,
                    Ty::I64 | Ty::F64 | Ty::Date | Ty::DateTime | Ty::Time | Ty::Duration
                ) =>
            {
                Ty::Bool
            }
            "==" | "!=" if a.ty != Ty::Void => Ty::Bool,
            "&&" | "||" if a.ty == Ty::Bool => Ty::Bool,
            _ => return Err(format!("unsupported binary operation {op}")),
        };
        Ok(Value {
            ty,
            kind: Kind::Binary(op.to_owned(), Box::new(a), Box::new(b)),
        })
    }
}

fn bind(
    signature: &Signature,
    args: &[(Option<String>, Value)],
    runtime: bool,
    hint: Option<&Ty>,
) -> Result<Bound, String> {
    let mut supplied = BTreeMap::new();
    let mut named = false;
    for (position, (name, value)) in args.iter().enumerate() {
        let index = if let Some(name) = name {
            named = true;
            signature
                .parameters
                .iter()
                .position(|p| &p.name == name)
                .ok_or_else(|| format!("unknown argument {name}"))?
        } else {
            if named {
                return Err("positional argument after named argument".into());
            }
            position
        };
        if index >= signature.parameters.len() || supplied.insert(index, value.clone()).is_some() {
            return Err("duplicate or excess argument".into());
        }
    }
    let mut types = BTreeMap::new();
    let mut values = Vec::new();
    let mut parameter_names = BTreeSet::new();
    for (index, p) in signature.parameters.iter().enumerate() {
        if !parameter_names.insert(&p.name) {
            return Err("duplicate parameter".into());
        }
        let value = supplied
            .remove(&index)
            .or_else(|| {
                p.default.as_ref().and_then(|e| {
                    if let Expr::Literal(l) = e {
                        Some(Value {
                            ty: l.ty(),
                            kind: Kind::Literal(l.clone()),
                        })
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| format!("missing argument {}", p.name))?;
        if p.constant && !matches!(value.kind, Kind::Literal(_)) {
            return Err("fixed argument requires a wiring-time value".into());
        }
        if !runtime && !p.constant && !matches!(value.kind, Kind::Wire(_)) {
            return Err("temporal argument requires a port".into());
        }
        if runtime && matches!(value.kind, Kind::Wire(_) | Kind::Void) {
            return Err("native value argument requires a scalar".into());
        }
        let value = bind_type(signature, p, value, &mut types)?;
        values.push(value);
    }
    if signature.generics.contains(&signature.result)
        && !types.contains_key(&signature.result)
        && let Some(hint) = hint
    {
        types.insert(signature.result.clone(), hint.clone());
    }
    let result = resolve_type(&signature.result, &types).ok_or("unresolved result type")?;
    supported_type(&result)?;
    for value in &values {
        supported_type(&value.ty)?;
    }
    Ok((values, types, result))
}

fn supported_type(ty: &Ty) -> Result<(), String> {
    match ty {
        Ty::Set(child) if !matches!(**child, Ty::Bool | Ty::I64) => {
            Err("Rust set elements currently require bool or i64".into())
        }
        Ty::Ref(child) if **child == Ty::Void => Err("reference requires a temporal type".into()),
        Ty::Ref(child) => supported_type(child),
        Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Set(_)
        | Ty::Void => Ok(()),
    }
}

fn bind_type(
    signature: &Signature,
    p: &crate::index::Parameter,
    mut value: Value,
    types: &mut BTreeMap<String, Ty>,
) -> Result<Value, String> {
    let formal =
        p.ty.strip_prefix("ref<")
            .or_else(|| p.ty.strip_prefix("set<"))
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(&p.ty);
    let actual = if p.ty.starts_with("ref<") {
        if let Ty::Ref(child) = &value.ty {
            child.as_ref()
        } else {
            &value.ty
        }
    } else if p.ty.starts_with("set<") {
        if let Ty::Set(child) = &value.ty {
            child.as_ref()
        } else {
            return Err("set argument required".into());
        }
    } else {
        &value.ty
    };
    if signature.generics.contains(&formal.to_owned()) {
        if types
            .insert(formal.to_owned(), actual.clone())
            .is_some_and(|old| old != *actual)
        {
            return Err("inconsistent generic inference".into());
        }
    } else if p.ty != "signal" && Ty::parse(formal) != Some(actual.clone()) {
        return Err(format!("type mismatch for {}", p.name));
    }

    if p.ty.starts_with("ref<") && !matches!(value.ty, Ty::Ref(_)) {
        value.ty = Ty::Ref(Box::new(value.ty));
    }
    Ok(value)
}

pub(crate) fn evaluate(
    library: Library,
    module: &str,
    name: &str,
    args: &[(Option<String>, Expr)],
) -> Result<Plan, String> {
    let mut checker = Checker {
        library,
        plan: Plan::default(),
        active: BTreeSet::new(),
        documented: BTreeSet::new(),
        test_scope: Some(module.into()),
        result_hint: None,
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
            || matches!(decl.role, Role::Implementation | Role::Test | Role::Native)
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
    let (plan, values) = candidates.remove(0);
    checker.plan = plan;
    let output = checker.call(module, name, &values, false)?;
    if let Kind::Wire(id) = output.kind {
        checker.plan.output = Some((id, output.ty));
    }
    Ok(checker.plan)
}

fn resolve_type(name: &str, types: &BTreeMap<String, Ty>) -> Option<Ty> {
    if let Some(t) = name.strip_prefix("ref<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::Ref(Box::new(resolve_type(t, types)?)));
    }
    if let Some(t) = name.strip_prefix("set<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::Set(Box::new(resolve_type(t, types)?)));
    }
    Ty::parse(name).or_else(|| types.get(name).cloned())
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
    if matches!(op, "bound" | "len" | "contains") && !matches!(first.kind, Kind::Input(_)) {
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

fn node_inputs(signature: &Signature, env: &mut Env, node: &mut Node) -> Result<(), String> {
    for p in &signature.parameters {
        if !p.constant {
            let value = env.get_mut(&p.name).ok_or("missing parameter")?;
            let Kind::Wire(wire) = value.kind else {
                return Err("temporal parameter requires a port".into());
            };
            if !matches!(
                value.ty,
                Ty::I64
                    | Ty::Bool
                    | Ty::F64
                    | Ty::Str
                    | Ty::Duration
                    | Ty::Date
                    | Ty::Time
                    | Ty::DateTime
                    | Ty::Ref(_)
                    | Ty::Set(_)
            ) {
                return Err("unsupported temporal input type".into());
            }
            let input = node.inputs.len();
            node.inputs.push((p.name.clone(), wire, value.ty.clone()));
            value.kind = Kind::Input(input);
        }
    }
    Ok(())
}

fn local(env: &mut Env, name: &str, ty: Ty, next: &mut usize) -> Result<usize, String> {
    let id = *next;
    *next += 1;
    if env
        .insert(
            name.into(),
            Value {
                ty,
                kind: Kind::Local(id),
            },
        )
        .is_some()
    {
        return Err(format!("duplicate local {name}"));
    }
    Ok(id)
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
            let index = plan.nodes.len();
            plan.input_length = plan.input_length.max(literals.len());
            plan.nodes.push(Node {
                name: format!("eval.replay{position}"),
                inputs: Vec::new(),
                result: ty.clone(),
                alarm: true,
                delays: if literals.is_empty() {
                    Vec::new()
                } else {
                    vec![0]
                },
                caches: Vec::new(),
                handlers: Vec::new(),
                replay: Some(literals),
            });
            Value {
                ty,
                kind: Kind::Wire(index),
            }
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
            Value {
                ty: value.ty(),
                kind: Kind::Literal(value),
            }
        };
        values.push((label.clone(), value));
    }
    bind(signature, &values, false, None)?;
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
