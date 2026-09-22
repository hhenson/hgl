//! Failed described child startup must reclaim bindings and reusable storage.
#[cfg(test)]
mod tests {
    use hgl_describe::{
        Boundary, BuildError, Buildable, Builder, ChildDescription, InputPort, Ports, Registry,
        instantiate_child,
    };
    use hgl_kernel::{
        Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation,
    };
    use hgl_store::{InputId, Store, Wake};
    use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType, ScalarType, TsType};

    fn shape() -> TsType {
        TsType::List(Box::new(TsType::Ts(ScalarType::I64)), 2)
    }
    struct FailStart;
    impl Node for FailStart {
        fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
            Err(NodeError::new("intentional startup failure"))
        }
        fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
            unreachable!()
        }
    }
    impl Buildable for FailStart {
        fn node_type() -> NodeType {
            NodeType {
                name: "fail",
                inputs: vec![("ts", shape())],
                output: Some(shape()),
                ..NodeType::default()
            }
        }
        fn build(p: &mut Ports<'_>) -> Result<Self, BuildError> {
            p.shaped_input("ts")?;
            p.shaped_output()?;
            Ok(Self)
        }
    }
    struct Retry {
        template: ChildDescription,
        registry: Registry,
        input: InputId,
        count: usize,
        steady: Option<[usize; 4]>,
    }
    impl Node for Retry {
        fn eval(&mut self, c: &mut Ctx<'_>) -> NodeResult {
            let now = c.evaluation_time();
            let template = &self.template;
            let registry = &self.registry;
            let input = self.input;
            let result = c.create_child(|s| {
                let built = instantiate_child(template, registry, s, &[input], None, now)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                Ok((built.graph, ()))
            });
            assert!(result.is_err());
            self.count += 1;
            let counts = c.store().bindings().storage_counts();
            assert_eq!(counts[3], 1, "only the owner's REF subscription remains");
            if self.count == 3 {
                self.steady = Some(counts);
            } else if self.count > 3 {
                assert_eq!(self.steady, Some(counts));
            }
            c.schedule_in(EngineDelta::STEP)
        }
    }
    struct Empty;
    impl Node for Empty {
        fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
            Ok(())
        }
    }
    impl Wake for Empty {
        fn wake(&mut self, _: NodeId) {}
    }
    #[test]
    fn failed_start_leaves_no_subscription_and_bounded_storage() {
        let mut registry = Registry::new();
        registry.register::<FailStart>().unwrap();
        let mut builder = Builder::new("fails", &registry);
        builder.node("fail", &[]).unwrap();
        let template = ChildDescription {
            graph: builder.finish().unwrap(),
            inputs: vec![Boundary {
                source_input: 0,
                source_path: vec![],
                target: InputPort {
                    node: 0,
                    input: 0,
                    path: vec![],
                },
            }],
            output: None,
            keyed: false,
        };
        let mut store = Store::new();
        let carrier = store.add_shaped_output(NodeId(0), TsType::Reference(Box::new(shape())));
        let input = store.add_shaped_input(NodeId(1), shape(), false);
        store
            .follow(input, carrier, EngineTime::NEVER, &mut Empty)
            .unwrap();
        let mut graph = Graph::new(
            "rollback".into(),
            vec![
                NodeSlot {
                    node: Box::new(Empty),
                    label: "source".into(),
                    node_type: NodeType::default(),
                    required: vec![],
                },
                NodeSlot {
                    node: Box::new(Retry {
                        template,
                        registry,
                        input,
                        count: 0,
                        steady: None,
                    }),
                    label: "retry".into(),
                    node_type: NodeType {
                        uses_scheduler: true,
                        schedule_on_start: true,
                        ..NodeType::default()
                    },
                    required: vec![],
                },
            ],
        );
        run_simulation(
            &mut graph,
            &mut store,
            &RunConfig {
                start_time: EngineTime::MIN_START,
                end_time: EngineTime::from_micros(201),
            },
        )
        .unwrap();
        assert_eq!(graph.node::<Retry>(NodeId(1)).unwrap().count, 200);
    }
}
