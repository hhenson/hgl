//! Capture composes the complete ordinary selected call after its written arguments.
use super::{
    BTreeSet, Checker, Decl, Env, Expr, Issue, Kind, Node, Phase, Signature, Statement, Ty, Value,
};
impl Checker {
    pub(super) fn capture_call(
        &mut self,
        module: &str,
        args: &[(Option<String>, Expr)],
        env: &Env,
        runtime: bool,
    ) -> Result<Value, Issue> {
        if runtime || !self.preparing_graph || self.propagation_result.is_some() {
            return Err(Issue::coded(
                "phase",
                "capture.phase",
                0..0,
                "capture requires graph composition",
            ));
        }
        let Some((None, callee)) = args.first() else {
            return Err("capture requires a positional temporal callable".into());
        };
        let Expr::Name(callee) = callee.syntax() else {
            return Err("capture requires a selected callable identity".into());
        };
        let args = args[1..]
            .iter()
            .map(|(name, expr)| Ok((name.clone(), self.expression(module, expr, env, false)?)))
            .collect::<Result<Vec<_>, Issue>>()?;
        let args = self.call_values(&args, false)?;
        let owner = self.plan.nodes.len();
        self.plan.nodes.push(empty("hgl.capture", Ty::Void));
        let begin = self.plan.nodes.len();
        let (value, result) = if matches!(callee.as_str(), "is_ok" | "is_err") {
            (self.outcome_predicate_values(callee, args)?, Ty::Bool)
        } else {
            self.call_selected(module, callee, &args, false)?
        };
        let output = if let Kind::Wire(output) = value.kind {
            Some(output)
        } else if matches!(value.kind, Kind::Void) && self.plan.nodes.len() > begin {
            None
        } else {
            return Err(Issue::coded(
                "phase",
                "capture.phase",
                0..0,
                "capture requires a temporal call; wiring-time values are not captured",
            ));
        };
        let nodes = (begin..self.plan.nodes.len()).collect::<Vec<_>>();
        let captures = self.capture_sources(&nodes, output);
        if matches!(result, Ty::Ref(_)) {
            return Err(Issue::typed(
                0..0,
                "Rust capture target cannot prepare a source-declared REF normal connection",
            ));
        }
        let fields = if output.is_some() {
            vec![
                ("value".into(), result.clone()),
                ("error".into(), Ty::Error),
            ]
        } else {
            vec![("error".into(), Ty::Error)]
        };
        let bundle = Ty::Struct(
            format!("\0capture.bundle.{owner}.{}", result.source_name()).into(),
            fields,
            vec![],
        );
        let mut node = empty("hgl.capture", Ty::Ref(Box::new(bundle.clone())));
        node.alarm = true;
        for source in captures {
            node.inputs.push((
                format!("capture{source}"),
                source,
                self.plan.nodes[source].result.clone(),
            ));
        }
        self.plan.nodes[owner] = node;
        self.plan.captures.push(hgl_semantics::ir::Capture {
            owner,
            nodes,
            output,
            value: result,
        });
        Ok(Value::new(bundle, Kind::Wire(owner)))
    }
    fn capture_sources(&self, nodes: &[usize], output: Option<usize>) -> BTreeSet<usize> {
        let mut captures = nodes
            .iter()
            .flat_map(|&node| {
                self.plan.nodes[node]
                    .inputs
                    .iter()
                    .map(|(_, source, _)| *source)
            })
            .filter(|source| !nodes.contains(source))
            .collect::<BTreeSet<_>>();
        if let Some(output) = output.filter(|output| !nodes.contains(output)) {
            captures.insert(output);
        }
        captures
    }
    pub(super) fn lift_native(
        &mut self,
        decl: &Decl,
        signature: &Signature,
        values: Vec<Value>,
        result: Ty,
    ) -> Result<Value, Issue> {
        let mut env = hgl_semantics::body_check::parameter_scope(signature, values);
        let mut node = hgl_semantics::value_check::prepare_node(
            signature,
            &mut env,
            format!("{}::{}", decl.module, decl.name),
            result.clone(),
        )?;
        let values = signature
            .parameters
            .iter()
            .map(|parameter| env[&parameter.name].clone())
            .collect::<Vec<_>>();
        let native = hgl_semantics::value_bind::native(
            &self.library,
            decl,
            signature,
            (values, result.clone()),
            (true, Phase::Evaluation.name()),
            &mut self.plan.natives,
        )?;
        node.handlers.push((None, vec![Statement::Return(native)]));
        let id = self.plan.nodes.len();
        self.plan.nodes.push(node);
        Ok(Value::new(result, Kind::Wire(id)))
    }
}
fn empty(name: &str, result: Ty) -> Node {
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
