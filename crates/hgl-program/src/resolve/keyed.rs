//! Independent dynamic bodies retain member endpoints and whole temporal captures.
use super::{BTreeSet, Checker, Env, Expr, Issue, Kind, Node, Stmt, Ty, Value};
use hgl_semantics::ir::Keyed;

impl Checker {
    pub(super) fn graph_keyed(
        &mut self,
        module: &str,
        statement: &Stmt,
        env: &Env,
    ) -> Result<(), Issue> {
        let (key, name, iterator, body) =
            if let Stmt::ForItems(key, name, iterator, body) = statement {
                (Some(key.as_str()), name.as_str(), iterator, body)
            } else if let Stmt::For(name, iterator, body) = statement {
                (None, name.as_str(), iterator, body)
            } else {
                unreachable!("checked graph loop")
            };
        let Expr::Call(operation, arguments) = iterator.syntax() else {
            return Err("dynamic graph iteration requires a collection iterator".into());
        };
        let [(_, expression)] = arguments.as_slice() else {
            return Err("graph-phase iterator predicates are unsupported".into());
        };
        let collection = self.expression(module, expression, env, false)?;
        let child = match (&collection.ty, key.is_some(), operation.as_str()) {
            (Ty::Map(_, child) | Ty::List(child, None), true, "items")
            | (Ty::Map(_, child), false, "values")
            | (Ty::List(child, None), false, "elements") => child.as_ref().clone(),
            _ => return Err("unsupported graph-phase collection iterator".into()),
        };
        let Kind::Wire(source) = collection.kind else {
            return Err("dynamic graph iteration requires a temporal collection".into());
        };
        if env.contains_key(name) || key.is_some_and(|key| env.contains_key(key) || key == name) {
            return Err("duplicate graph iteration binding".into());
        }
        independent(body, env, key)?;
        let owner = self.plan.nodes.len();
        let mut owner_node = placeholder("hgl.keyed", Ty::Void);
        owner_node.alarm = true;
        owner_node
            .inputs
            .push(("collection".into(), source, collection.ty));
        self.plan.nodes.push(owner_node);
        let member = self.plan.nodes.len();
        self.plan
            .nodes
            .push(placeholder("hgl.keyed.member", child.clone()));
        let mut scope = env.clone();
        scope.insert(name.into(), Value::new(child, Kind::Wire(member)));
        let incoming = self.graph_locals.clone();
        let begin = self.plan.nodes.len();
        let result = self.graph_body(module, body, &mut scope);
        self.graph_locals = incoming;
        result?;
        let nodes: Vec<_> = (begin..self.plan.nodes.len()).collect();
        let captures: BTreeSet<_> = nodes
            .iter()
            .flat_map(|&id| {
                self.plan.nodes[id]
                    .inputs
                    .iter()
                    .map(|(_, source, _)| *source)
            })
            .filter(|source| *source != member && !nodes.contains(source))
            .collect();
        for capture in captures {
            let ty = self.plan.nodes[capture].result.clone();
            self.plan.nodes[owner]
                .inputs
                .push((format!("capture{capture}"), capture, ty));
        }
        self.plan.keyed.push(Keyed {
            owner,
            nodes,
            member,
        });
        Ok(())
    }
}

fn placeholder(name: &str, result: Ty) -> Node {
    Node {
        name: name.into(),
        inputs: vec![],
        result,
        alarm: false,
        generator: None,
        start: vec![],
        global_state: false,
        globals: vec![],
        configuration: vec![],
        stop: vec![],
        caches: vec![],
        handlers: vec![],
    }
}

fn independent(body: &[Stmt], env: &Env, key: Option<&str>) -> Result<(), Issue> {
    for statement in body {
        match statement {
            Stmt::Assign(target, value) | Stmt::Add(target, value) => {
                let mut target = target.syntax();
                while let Expr::Property(parent, _) | Expr::Index(parent, _) = target {
                    target = parent.syntax();
                }
                if matches!(target, Expr::Name(name) if env.contains_key(name)) {
                    return Err("dynamic graph bodies cannot assign enclosing bindings".into());
                }
                captures(value, env, key)?;
            }
            Stmt::Let(_, _, value) | Stmt::Var(_, _, value) | Stmt::Call(value) => {
                captures(value, env, key)?;
            }
            Stmt::If(condition, yes, no) => {
                captures(condition, env, key)?;
                independent(yes, env, key)?;
                independent(no, env, key)?;
            }
            Stmt::For(_, iterator, body) | Stmt::ForItems(_, _, iterator, body) => {
                captures(iterator, env, key)?;
                independent(body, env, key)?;
            }
            Stmt::Return(_) | Stmt::Exit => {
                return Err("dynamic graph loop results and exits are unsupported".into());
            }
            Stmt::While(..) | Stmt::TimedYield(..) | Stmt::DeclareVar(..) => {}
        }
    }
    Ok(())
}

fn captures(expression: &Expr, env: &Env, key: Option<&str>) -> Result<(), Issue> {
    if let Expr::Name(name) = expression.syntax() {
        if key == Some(name.as_str()) {
            return Err("Rust dynamic graph key expression binding is unsupported".into());
        }
        if env
            .get(name)
            .is_some_and(|value| !matches!(value.kind, Kind::Wire(_)))
        {
            return Err("dynamic graph scalar captures require an agreed binding contract".into());
        }
    }
    for child in expression.children() {
        captures(child, env, key)?;
    }
    Ok(())
}
