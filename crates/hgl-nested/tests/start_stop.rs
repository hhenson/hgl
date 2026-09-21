//! ENG-10: a child start-hook stop request reaches the root without an eval.
use hgl_kernel::{
    Ctx, EngineError, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation,
};
use hgl_nested::Children;
use hgl_store::Store;
use hgl_types::{EngineDelta, EngineTime, NodeType};
use std::{cell::RefCell, rc::Rc};
type Log = Rc<RefCell<Vec<&'static str>>>;
fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "node".into(),
        required: vec![],
        node_type: NodeType {
            schedule_on_start: scheduled,
            uses_scheduler: true,
            ..NodeType::default()
        },
    }
}
struct Stopper(Log);
impl Node for Stopper {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.0.borrow_mut().push("child_start");
        ctx.request_stop();
        Ok(())
    }
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Err(NodeError::new("the child has no scheduled evaluation"))
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.0.borrow_mut().push("child_stop");
        Ok(())
    }
}
struct Owner {
    children: Children,
    depth: usize,
    create_in_eval: bool,
    log: Log,
}
impl Owner {
    fn create(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let depth = self.depth;
        let log = Rc::clone(&self.log);
        self.children.insert(0, ctx, |_| {
            let node = if depth == 0 {
                slot(Stopper(log), false)
            } else {
                slot(
                    Self {
                        children: Children::new(),
                        depth: depth - 1,
                        create_in_eval: false,
                        log,
                    },
                    false,
                )
            };
            Ok((Graph::new("child".into(), vec![node]), ()))
        })
    }
}
impl Node for Owner {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if !self.create_in_eval {
            self.create(ctx)?;
        }
        Ok(())
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if self.create_in_eval {
            self.create(ctx)?;
            self.create_in_eval = false;
        }
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)?;
        self.log.borrow_mut().push("owner_stop");
        Ok(())
    }
}
struct Pulse(Log);
impl Node for Pulse {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.0.borrow_mut().push("root_eval");
        ctx.schedule_in(EngineDelta::STEP)
    }
}
#[test]
fn unscheduled_child_start_stop_reaches_root_from_start_or_eval_through_each_owner()
-> Result<(), EngineError> {
    for depth in 0..=1 {
        for create_in_eval in [false, true] {
            let log = Log::default();
            let mut store = Store::new();
            let owner = Owner {
                children: Children::new(),
                depth,
                create_in_eval,
                log: Rc::clone(&log),
            };
            let mut graph = Graph::new(
                "root".into(),
                vec![
                    slot(owner, create_in_eval),
                    slot(Pulse(Rc::clone(&log)), true),
                ],
            );
            let cycles = run_simulation(
                &mut graph,
                &mut store,
                &RunConfig {
                    start_time: EngineTime::MIN_START,
                    end_time: EngineTime::from_micros(10),
                },
            )?;
            assert_eq!(cycles, u64::from(create_in_eval));
            let mut expected = vec!["child_start"];
            if create_in_eval {
                expected.push("root_eval");
            }
            expected.push("child_stop");
            expected.extend(std::iter::repeat_n("owner_stop", depth + 1));
            assert_eq!(*log.borrow(), expected);
        }
    }
    Ok(())
}
