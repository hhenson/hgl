use super::*;

pub(super) struct Feed {
    selectors: OutputId,
    prices: [Out<i64>; 2],
    step: i64,
}
impl Buildable for Feed {
    fn node_type() -> NodeType {
        NodeType {
            name: "feed",
            output: Some(TsType::Bundle(vec![
                ("selectors".into(), dict(scalar())),
                ("a".into(), shape()),
                ("b".into(), shape()),
            ])),
            uses_scheduler: true,
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        let out = p.shaped_output()?;
        let b = p.store().bindings();
        let selectors = b.fixed_output(out, 0);
        let prices = [1, 2].map(|n| leaf_out(p.store(), b.fixed_output(out, n)));
        Ok(Self {
            selectors,
            prices,
            step: 0,
        })
    }
}
impl Node for Feed {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        if let Some(v) = match self.step {
            0 | 7 => Some(0),
            2 => Some(1),
            _ => None,
        } {
            let id = c.get_or_create_shaped(self.selectors, 0);
            let out = c.store().scalar_output::<i64>(id).map_err(err)?;
            c.set(out, v);
        }
        if self.step == 5 {
            c.remove_shaped(self.selectors, 0);
        }
        for (step, n, v) in [(0, 0, 2), (0, 1, 10), (1, 0, 3), (3, 0, 4), (6, 0, 5)] {
            if step == self.step {
                c.set(self.prices[n], v);
            }
        }
        self.step += 1;
        if self.step < 11 {
            c.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}

pub(super) struct Choose {
    selector: In<i64>,
    sources: [InputId; 2],
    out: OutputId,
}
impl Buildable for Choose {
    fn node_type() -> NodeType {
        NodeType {
            name: "choose",
            inputs: vec![("selector", scalar()), ("a", shape()), ("b", shape())],
            output: Some(reference(shape())),
            active_inputs: Some(vec![0]),
            valid_inputs: Some(vec![0]),
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            selector: p.input("selector")?,
            sources: [p.shaped_input("a")?, p.shaped_input("b")?],
            out: p.shaped_output()?,
        })
    }
}
impl Node for Choose {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        let n = usize::try_from(c.get(self.selector)).map_err(err)?;
        c.set_reference(
            self.out,
            c.store().bindings().input_reference(self.sources[n]),
        )
    }
}

pub(super) struct Route {
    sources: [InputId; 2],
    out: OutputId,
    step: i64,
}
impl Buildable for Route {
    fn node_type() -> NodeType {
        NodeType {
            name: "route",
            inputs: vec![("a", shape()), ("b", shape())],
            output: Some(reference(shape())),
            active_inputs: Some(vec![]),
            valid_inputs: Some(vec![]),
            uses_scheduler: true,
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            sources: [p.shaped_input("a")?, p.shaped_input("b")?],
            out: p.shaped_output()?,
            step: 0,
        })
    }
}
impl Node for Route {
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        if let Some(n) = match self.step {
            0 | 7 => Some(0),
            2 => Some(1),
            _ => None,
        } {
            c.set_reference(
                self.out,
                c.store().bindings().input_reference(self.sources[n]),
            )?;
        }
        self.step += 1;
        c.schedule_in(EngineDelta::STEP)
    }
}

pub(super) struct Total {
    input: InputId,
    leaf: In<i64>,
    out: OutputId,
    value: Out<i64>,
    sum: i64,
}
impl Buildable for Total {
    fn node_type() -> NodeType {
        NodeType {
            name: "total",
            inputs: vec![("ts", shape())],
            output: Some(shape()),
            uses_scheduler: true,
            ..NodeType::default()
        }
    }
    fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
        let input = p.shaped_input("ts")?;
        let out = p.shaped_output()?;
        let b = p.store().bindings();
        let leaf = b.fixed_input(b.fixed_input(input, 0), 0);
        Ok(Self {
            input,
            leaf: p.store().scalar_input(leaf).map_err(BuildError::Bind)?,
            out,
            value: leaf_out(p.store(), out),
            sum: 0,
        })
    }
}
impl Node for Total {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        event("start");
        Ok(())
    }
    fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
        let t = c.evaluation_time().micros() - 1;
        let n = CHILD_COUNT.with(|count| {
            let n = count.get();
            count.set(n + 1);
            n
        });
        OBSERVED.with_borrow_mut(|rows| {
            rows.insert(format!("/child_ticks/{n}/time"), t.to_string());
            snapshot(
                c.store(),
                self.input,
                t,
                &format!("/child_ticks/{n}/input"),
                rows,
            );
        });
        if c.modified(self.leaf) {
            self.sum += c.get(self.leaf);
            c.set(self.value, self.sum);
            c.schedule_in(EngineDelta::from_micros(2))?;
        } else if c.is_scheduled_now() {
            c.set(self.value, -self.sum);
        }
        OBSERVED.with_borrow_mut(|rows| {
            snapshot::output(
                c.store(),
                self.out,
                t,
                &format!("/child_ticks/{n}/output"),
                rows,
            );
        });
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        event("stop");
        Ok(())
    }
}
