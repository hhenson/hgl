//! HGL evaluation with sparse recording and dense sequence comparison.
use hgl_describe::{
    BuildError, Buildable, Edge, GraphDescription, InputPort, NodeDescription, OutputPort, Ports,
    Registry, instantiate_complete,
};
use hgl_kernel::{Ctx, Node, NodeResult, RunConfig, run_simulation};
use hgl_store::{In, Scalar, Store};
use hgl_types::{EngineTime, NodeId, NodeType, TsType};

#[derive(Debug)]
struct Recorder<T: Scalar> {
    input: In<T>,
    ticks: Vec<(usize, T)>,
}
impl<T: Scalar> Node for Recorder<T> {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let cycle =
            usize::try_from(ctx.evaluation_time().micros() - EngineTime::MIN_START.micros())
                .map_err(|e| hgl_kernel::NodeError::new(e.to_string()))?;
        self.ticks.push((cycle, ctx.get(self.input)));
        Ok(())
    }
}
impl<T: Scalar> Buildable for Recorder<T> {
    fn node_type() -> NodeType {
        NodeType {
            name: "eval.record",
            inputs: vec![("value", TsType::Ts(T::TYPE))],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("value")?,
            ticks: Vec::new(),
        })
    }
}

/// Sparse storage of a dense result, including its trailing silent cells.
#[derive(Debug)]
pub struct Observation<T> {
    length: usize,
    ticks: Vec<(usize, T)>,
}

/// Execute a graph and record only its selected output ticks.
/// The horizon comes from the inputs and actual output ticks, never expectations.
pub fn evaluate<T: Scalar>(
    mut description: GraphDescription,
    registry: &mut Registry,
    output: u32,
    input_length: usize,
) -> Result<Observation<T>, String> {
    registry
        .register::<Recorder<T>>()
        .map_err(|e| format!("{e:?}"))?;
    let record = u32::try_from(description.nodes.len()).map_err(|e| e.to_string())?;
    description.nodes.push(NodeDescription {
        implementation: "eval.record".into(),
        label: "eval.record".into(),
        scalars: Vec::new(),
        children: Vec::new(),
    });
    description.edges.push(Edge {
        source: OutputPort {
            node: output,
            path: Vec::new(),
        },
        target: InputPort {
            node: record,
            input: 0,
            path: Vec::new(),
        },
    });
    let mut store = Store::new();
    let mut built =
        instantiate_complete(&description, registry, &mut store).map_err(|e| format!("{e:?}"))?;
    run_simulation(
        &mut built.graph,
        &mut store,
        &RunConfig {
            start_time: EngineTime::MIN_START,
            end_time: EngineTime::MAX_END,
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    let recorder = built
        .graph
        .node::<Recorder<T>>(NodeId(record))
        .ok_or("missing eval recorder")?;
    let last = recorder.ticks.last().map_or(Ok(0), |(i, _)| {
        i.checked_add(1).ok_or("eval horizon overflow")
    })?;
    Ok(Observation {
        length: input_length.max(last),
        ticks: recorder.ticks.clone(),
    })
}

/// Compare equal-length dense sequences, reporting the first differing cycle.
pub fn compare<T: PartialEq + std::fmt::Debug>(
    expected: &[Option<T>],
    observed: &Observation<T>,
) -> Result<(), String> {
    let mut ticks = observed.ticks.iter().peekable();
    for (cycle, wanted) in expected.iter().take(observed.length).enumerate() {
        let actual = if ticks.peek().is_some_and(|(i, _)| *i == cycle) {
            ticks.next().map(|(_, value)| value)
        } else {
            None
        };
        if wanted.as_ref() != actual {
            return Err(format!(
                "cycle {cycle}: expected {wanted:?}, observed {actual:?}"
            ));
        }
    }
    if expected.len() != observed.length {
        return Err(format!(
            "cycle {}: expected length {}, observed length {}",
            expected.len().min(observed.length),
            expected.len(),
            observed.length
        ));
    }
    Ok(())
}
