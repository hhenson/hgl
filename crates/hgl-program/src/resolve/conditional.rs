//! Checked temporal branch ownership retains real reference producers.
use super::{BTreeSet, Checker, Env, Issue, Kind, Node, Stmt, Ty, Value};
use hgl_semantics::ir::{Conditional, ConditionalBranch};
impl Checker {
    pub(super) fn graph_conditional(
        &mut self,
        module: &str,
        statement: &Stmt,
        env: &mut Env,
        ports: &BTreeSet<String>,
    ) -> Result<(), Issue> {
        let Stmt::If(expr, yes, no) = statement else {
            unreachable!("checked graph if")
        };
        let condition = self.expression(module, expr, env, false)?;
        if let Kind::Wire(selector) = condition.kind {
            if condition.ty != Ty::Bool {
                return Err("condition requires bool".into());
            }
            return self.temporal_branches(module, selector, [yes, no], env);
        }
        if hgl_semantics::wiring_locals::handles(statement, env) {
            hgl_semantics::wiring_locals::conditional(
                statement,
                env,
                ports,
                (&self.graph_locals, &self.static_values),
                (&mut self.plan, &mut self.wiring),
                &mut self.next_wiring_local,
            )?;
        } else {
            let mut next = self.next_wiring_local;
            let checked = self.statement(module, statement, env, &Ty::Void, &mut next)?;
            self.next_wiring_local = next;
            self.wiring_statement(&checked)?;
        }
        Ok(())
    }
    fn temporal_branches(
        &mut self,
        module: &str,
        selector: usize,
        bodies: [&Vec<Stmt>; 2],
        env: &mut Env,
    ) -> Result<(), Issue> {
        let slots: Vec<_> = self
            .graph_locals
            .declarations()
            .map(|(n, t)| (n.clone(), t.clone()))
            .collect();
        let [(name, ty)] = slots.as_slice() else {
            return Err(
                "temporal graph conditionals are unsupported by this backend outside the single declared REF export profile"
                    .into(),
            );
        };
        let owner = self.plan.nodes.len();
        self.plan.nodes.push(owner_node(selector, ty.clone()));
        let incoming = self.graph_locals.clone();
        let mut branches = Vec::new();
        for body in bodies {
            self.graph_locals = incoming.clone();
            let mut branch_env = env.clone();
            let begin = self.plan.nodes.len();
            self.graph_body(module, body, &mut branch_env)?;
            let value = branch_env.get(name).ok_or_else(|| {
                format!("local {name} is not definitely assigned in both branches")
            })?;
            if value.ty != *ty {
                return Err("conditional reference export type mismatch".into());
            }
            let Kind::Wire(output) = value.kind else {
                return Err("conditional export requires a temporal reference connection".into());
            };
            if output < begin {
                return Err("conditional export requires a branch reference producer".into());
            }
            branches.push(ConditionalBranch {
                nodes: (begin..self.plan.nodes.len()).collect(),
                output,
            });
        }
        self.graph_locals = incoming;
        let captures: BTreeSet<_> = branches
            .iter()
            .flat_map(|branch| {
                branch
                    .nodes
                    .iter()
                    .flat_map(|&id| {
                        self.plan.nodes[id]
                            .inputs
                            .iter()
                            .map(|(_, source, _)| *source)
                    })
                    .filter(|source| !branch.nodes.contains(source))
            })
            .collect();
        for source in captures {
            let ty = self.plan.nodes[source].result.clone();
            self.plan.nodes[owner]
                .inputs
                .push((format!("capture{source}"), source, ty));
        }
        let branches = branches
            .try_into()
            .unwrap_or_else(|_| unreachable!("two branches"));
        self.plan.conditionals.push(Conditional { owner, branches });
        self.graph_locals.initialized(name);
        env.insert(name.clone(), Value::new(ty.clone(), Kind::Wire(owner)));
        Ok(())
    }
}
fn owner_node(selector: usize, result: Ty) -> Node {
    Node {
        name: "hgl.conditional".into(),
        inputs: vec![("selector".into(), selector, Ty::Bool)],
        result,
        alarm: true,
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
