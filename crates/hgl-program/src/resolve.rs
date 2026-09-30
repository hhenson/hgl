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
    Void,
}
#[derive(Debug)]
pub(crate) enum Statement {
    Let(usize, Value),
    Return(Value),
    Call(Value),
    Add(usize, Value),
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
    pub statements: Vec<Statement>,
}
#[derive(Debug)]
pub(crate) struct Native {
    pub name: String,
    pub method: String,
    pub args: Vec<Ty>,
    pub result: Ty,
}
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub nodes: Vec<Node>,
    pub natives: Vec<Native>,
    pub docs: Vec<String>,
}
struct Checker {
    library: Library,
    plan: Plan,
    active: BTreeSet<usize>,
    documented: BTreeSet<usize>,
}
type Bound = (Vec<Value>, BTreeMap<String, Ty>, Ty);
type Selection = (usize, Signature, Vec<Value>, BTreeMap<String, Ty>, Ty);
type Env = BTreeMap<String, Value>;

pub(crate) fn compile(library: Library, module: &str, entry: &str) -> Result<Plan, String> {
    let mut checker = Checker {
        library,
        plan: Plan::default(),
        active: BTreeSet::new(),
        documented: BTreeSet::new(),
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
        let mut env: Env = signature
            .parameters
            .iter()
            .zip(&values)
            .map(|(p, v)| (p.name.clone(), v.clone()))
            .collect();
        let mut cursor = Cursor::new(&signature.body);
        let value = if signature.body.iter().any(|t| t.text == "when") {
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
            if decl.module != module || decl.name != name || decl.role == Role::Implementation {
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
            match bind(&signature, args, runtime || decl.role == Role::Native) {
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
            let Ok((_, inferred, output)) = bind(&sig, &positional, runtime) else {
                continue;
            };
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
                if !self
                    .library
                    .instances
                    .contains(&(module.to_owned(), name.to_owned(), instance))
                {
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
        let statements = cursor.block()?;
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
                Stmt::Add(_, _) | Stmt::If(_, _, _) => {
                    return Err("runtime statement in graph".into());
                }
            }
        }
        if *result != Ty::Void && (output.ty != *result || !matches!(output.kind, Kind::Wire(_))) {
            return Err("graph return type mismatch".into());
        }
        Ok(if *result == Ty::Void {
            Value {
                ty: Ty::Void,
                kind: Kind::Void,
            }
        } else {
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
        if !matches!(result, Ty::I64 | Ty::Bool | Ty::Void) {
            return Err("unsupported temporal output type".into());
        }
        let mut node = Node {
            name: format!("{}::{}", decl.module, decl.name),
            inputs: Vec::new(),
            result: result.clone(),
            alarm: false,
            delays: Vec::new(),
            caches: Vec::new(),
            statements: Vec::new(),
        };
        for p in &signature.parameters {
            if !p.constant {
                let value = env.get_mut(&p.name).ok_or("missing parameter")?;
                let Kind::Wire(wire) = value.kind else {
                    return Err("temporal parameter requires a port".into());
                };
                if !matches!(value.ty, Ty::I64 | Ty::Bool) {
                    return Err("unsupported temporal input type".into());
                }
                let input = node.inputs.len();
                node.inputs.push((p.name.clone(), wire, value.ty.clone()));
                value.kind = Kind::Input(input);
            }
        }
        c.need("{")?;
        c.lines();
        if c.take("inject") {
            c.need("alarm")?;
            node.alarm = true;
            c.lines();
        }
        if node.alarm && !node.inputs.is_empty() {
            return Err("alarm is admitted only on sources".into());
        }
        self.initializers(decl, c, env, &mut node)?;
        c.need("when")?;
        let statements = c.block()?;
        c.lines();
        c.need("}")?;
        c.lines();
        if !c.at("") {
            return Err("unsupported node body suffix".into());
        }
        node.statements = self.statements(&decl.module, &statements, env, result, &mut 0)?;
        Ok(node)
    }
    fn initializers(
        &mut self,
        decl: &Decl,
        c: &mut Cursor<'_>,
        env: &mut Env,
        node: &mut Node,
    ) -> Result<(), String> {
        while c.take("cache") {
            let name = c.name()?;
            c.need(":")?;
            c.need("i64")?;
            c.need("=")?;
            let value = self.expression(&decl.module, &c.expr()?, env, false)?;
            let Kind::Literal(Literal::Int(initial)) = value.kind else {
                return Err("cache initializer must be fixed i64".into());
            };
            let index = node.caches.len();
            node.caches.push(Literal::Int(initial));
            if env
                .insert(
                    name,
                    Value {
                        ty: Ty::I64,
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
        let mut out = Vec::new();
        for statement in statements {
            out.push(match statement {
                Stmt::Let(name, expr) => {
                    let value = self.expression(module, expr, env, true)?;
                    let id = *next_local;
                    *next_local += 1;
                    if env
                        .insert(
                            name.clone(),
                            Value {
                                ty: value.ty.clone(),
                                kind: Kind::Local(id),
                            },
                        )
                        .is_some()
                    {
                        return Err(format!("duplicate local {name}"));
                    }
                    Statement::Let(id, value)
                }
                Stmt::Return(expr) => {
                    let v = self.expression(module, expr, env, true)?;
                    if &v.ty != result || *result == Ty::Void {
                        return Err("node return type mismatch".into());
                    }
                    Statement::Return(v)
                }
                Stmt::Call(expr) => Statement::Call(self.expression(module, expr, env, true)?),
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
            });
        }
        Ok(out)
    }
    fn expression(
        &mut self,
        module: &str,
        expr: &Expr,
        env: &Env,
        runtime: bool,
    ) -> Result<Value, String> {
        match expr {
            Expr::Literal(l) => Ok(Value {
                ty: l.ty(),
                kind: Kind::Literal(l.clone()),
            }),
            Expr::Name(name) => env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown value {name}")),
            Expr::Call(name, args) => {
                let args = args
                    .iter()
                    .map(|(n, v)| Ok((n.clone(), self.expression(module, v, env, runtime)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                self.call(module, name, &args, runtime)
            }
            Expr::Binary(op, a, b) => {
                let a = self.expression(module, a, env, runtime)?;
                let b = self.expression(module, b, env, runtime)?;
                if !runtime {
                    return Err("fixed argument arithmetic is not yet supported".into());
                }
                if a.ty != b.ty {
                    return Err("binary operand type mismatch".into());
                }
                let ty = match op.as_str() {
                    "+" if matches!(a.ty, Ty::Str | Ty::I64) => a.ty.clone(),
                    "-" | "*" | "%" if a.ty == Ty::I64 => Ty::I64,
                    ">" | "<" | ">=" | "<=" if a.ty == Ty::I64 => Ty::Bool,
                    "==" | "!=" if a.ty != Ty::Void => Ty::Bool,
                    "&&" | "||" if a.ty == Ty::Bool => Ty::Bool,
                    _ => return Err(format!("unsupported binary operation {op}")),
                };
                Ok(Value {
                    ty,
                    kind: Kind::Binary(op.clone(), Box::new(a), Box::new(b)),
                })
            }
        }
    }
}
fn bind(
    signature: &Signature,
    args: &[(Option<String>, Value)],
    runtime: bool,
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
        if signature.generics.contains(&p.ty) {
            if types
                .insert(p.ty.clone(), value.ty.clone())
                .is_some_and(|old| old != value.ty)
            {
                return Err("inconsistent generic inference".into());
            }
        } else if Ty::parse(&p.ty) != Some(value.ty.clone()) {
            return Err(format!("type mismatch for {}", p.name));
        }
        values.push(value);
    }
    let result = Ty::parse(&signature.result)
        .or_else(|| types.get(&signature.result).cloned())
        .ok_or("unresolved result type")?;
    Ok((values, types, result))
}
