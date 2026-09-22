use super::*;
use hgl_nested::Children;

pub(super) struct Map {
    inputs: Vec<InputId>,
    out: OutputId,
    template: ChildDescription,
    registry: Registry,
    children: Children,
}
impl Buildable for Map {
    fn node_type() -> NodeType {
        NodeType {
            name: "map",
            inputs: owner_inputs(),
            output: Some(dict(shape())),
            active_inputs: Some(vec![0]),
            valid_inputs: Some(vec![0]),
            child_graphs: 1,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            inputs: vec![
                p.shaped_input("selectors")?,
                p.shaped_input("a")?,
                p.shaped_input("b")?,
            ],
            out: p.shaped_output()?,
            template: p.children()[0].clone(),
            registry: p.registry().clone(),
            children: Children::new(),
        })
    }
}
impl Node for Map {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        let removed: Vec<_> = c.store().bindings().removed_keys(self.inputs[0]).collect();
        for key in removed {
            self.children.remove(key, c)?;
            c.remove_shaped(self.out, key);
        }
        let added: Vec<_> = c.store().bindings().added_keys(self.inputs[0]).collect();
        for key in added {
            let now = c.evaluation_time();
            let out = self.children.insert(key, c, |s| {
                let built = instantiate_child(
                    &self.template,
                    &self.registry,
                    s,
                    &self.inputs,
                    Some(key),
                    now,
                )
                .map_err(err)?;
                let out = built
                    .output(self.template.output.as_ref().unwrap(), s)
                    .map_err(err)?;
                Ok((built.graph, out))
            })?;
            c.attach_shaped(self.out, key, c.store().reference(out))?;
        }
        self.children.evaluate(c)
    }
    fn stop(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(c)
    }
}

pub(super) struct Outer {
    inputs: Vec<InputId>,
    out: OutputId,
    template: ChildDescription,
    registry: Registry,
    children: Children,
    started: bool,
}
impl Buildable for Outer {
    fn node_type() -> NodeType {
        NodeType {
            name: "outer",
            inputs: owner_inputs(),
            output: Some(reference(dict(shape()))),
            active_inputs: Some(vec![0]),
            valid_inputs: Some(vec![0]),
            child_graphs: 1,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            inputs: vec![
                p.shaped_input("selectors")?,
                p.shaped_input("a")?,
                p.shaped_input("b")?,
            ],
            out: p.shaped_output()?,
            template: p.children()[0].clone(),
            registry: p.registry().clone(),
            children: Children::new(),
            started: false,
        })
    }
}
impl Node for Outer {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        if !self.started {
            let now = c.evaluation_time();
            let out = self.children.insert(0, c, |s| {
                let built =
                    instantiate_child(&self.template, &self.registry, s, &self.inputs, None, now)
                        .map_err(err)?;
                let out = built
                    .output(self.template.output.as_ref().unwrap(), s)
                    .map_err(err)?;
                Ok((built.graph, out))
            })?;
            c.set_reference(self.out, c.store().reference(out))?;
            self.started = true;
        }
        self.children.evaluate(c)
    }
    fn stop(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(c)
    }
}

pub(super) struct Observe {
    a: InputId,
    b: InputId,
    rows: InputId,
}
impl Buildable for Observe {
    fn node_type() -> NodeType {
        NodeType {
            name: "observe",
            inputs: vec![("a", shape()), ("b", shape()), ("rows", dict(shape()))],
            active_inputs: Some(vec![]),
            valid_inputs: Some(vec![]),
            uses_scheduler: true,
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            a: p.shaped_input("a")?,
            b: p.shaped_input("b")?,
            rows: p.shaped_input("rows")?,
        })
    }
}
impl Node for Observe {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        let t = c.evaluation_time().micros() - 1;
        let path = format!("/ticks/{t}");
        let b = c.store().bindings();
        OBSERVED.with_borrow_mut(|r| {
            snapshot(c.store(), self.a, t, &format!("{path}/a"), r);
            snapshot(c.store(), self.b, t, &format!("{path}/b"), r);
            let live: Vec<_> = b.keys(self.rows).collect();
            let removed: Vec<_> = b.removed_keys(self.rows).collect();
            let added: Vec<_> = b.added_keys(self.rows).collect();
            for (name, ks) in [("keys", &live), ("added", &added), ("removed", &removed)] {
                r.insert(format!("{path}/{name}"), ks.len().to_string());
                for (n, k) in ks.iter().enumerate() {
                    r.insert(format!("{path}/{name}/{n}"), k.to_string());
                }
            }
            for (name, ks) in [("rows", live), ("retired", removed)] {
                r.insert(
                    format!("{path}/{name}"),
                    format!(
                        "[{}]",
                        ks.iter()
                            .map(|k| format!("\"{k}\""))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                );
                for k in ks {
                    let i = [b.child_input(self.rows, k), b.removed_input(self.rows, k)]
                        [usize::from(name != "rows")]
                    .unwrap();
                    snapshot(c.store(), i, t, &format!("{path}/{name}/{k}"), r);
                }
            }
        });
        c.schedule_in(EngineDelta::STEP)
    }
}
