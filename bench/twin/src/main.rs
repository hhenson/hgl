//! The Rust half of each benchmark pair (`bench/README.md`).
//!
//! It takes the same command line as `bench/baselines/cpp/scenarios.cpp`,
//! builds the same graph, checks the same checksum, and prints the same line,
//! so that `cargo xtask bench` measures both halves the same way.
//!
//! Only `run_simulation` is timed: wiring and instantiation are outside the
//! clock, as they are in the baseline. Every run checks its own checksum
//! against a closed form, so a time is never reported for a graph that
//! computed the wrong thing.
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line program"
)]

use std::process::ExitCode;
use std::time::Instant;

use hgl_describe::{BuildError, Builder, GraphDescription, NodeRef, Registry, instantiate};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_proto_nodes::{Checksum, register_all};
use hgl_store::Store;
use hgl_types::{EngineDelta, EngineTime, NodeId, ScalarValue};

const USAGE: &str = "usage: hgl_twin tick|chain|wide_chain [--cycles N] [--depth D] [--width W]";

/// The graph a scenario builds, and how long it is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    cycles: u64,
    depth: u64,
    width: u64,
    /// Whether the pulse fans out to `width` branches that are folded again.
    fans_out: bool,
}

/// What a correct run of a shape must produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Expectation {
    /// The sum of every value the sink saw.
    checksum: u64,
    /// The number of nodes in the graph.
    nodes: u64,
}

/// What a run measured.
struct Measured {
    seconds: f64,
    checksum: u64,
    sink_evals: u64,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match scenario(&args) {
        Ok((name, shape)) => report(name, shape),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn scenario(args: &[String]) -> Result<(&str, Shape), String> {
    let (name, options) = args.split_first().ok_or(USAGE)?;
    let defaults = match name.as_str() {
        "tick" => Shape {
            cycles: 3_000_000,
            depth: 1,
            width: 1,
            fans_out: false,
        },
        "chain" => Shape {
            cycles: 100_000,
            depth: 100,
            width: 1,
            fans_out: false,
        },
        "wide_chain" => Shape {
            cycles: 20_000,
            depth: 30,
            width: 30,
            fans_out: true,
        },
        other => return Err(format!("unknown scenario '{other}'\n{USAGE}")),
    };
    Ok((name.as_str(), with_options(defaults, options)?))
}

fn with_options(mut shape: Shape, options: &[String]) -> Result<Shape, String> {
    let mut rest = options.iter();
    while let Some(flag) = rest.next() {
        let text = rest.next().ok_or(USAGE)?;
        let value: u64 = text
            .parse()
            .map_err(|error| format!("{flag} {text}: {error}"))?;
        match flag.as_str() {
            "--cycles" => shape.cycles = value,
            "--depth" => shape.depth = value,
            "--width" => shape.width = value,
            other => return Err(format!("unexpected option '{other}'\n{USAGE}")),
        }
    }
    Ok(shape)
}

/// The sum of `0..n`.
fn triangle(n: u64) -> u64 {
    n * n.saturating_sub(1) / 2
}

/// The closed forms of `bench/baselines/cpp/scenarios.cpp`.
///
/// `pulse` emits `0..cycles`. A chain adds its depth; a wide chain is `width`
/// chains, branch `b` first adding `b`, folded by addition.
fn expect(shape: Shape) -> Expectation {
    let (cycles, depth, width) = (shape.cycles, shape.depth, shape.width);
    let per_cycle_constant = triangle(width) + width * depth;
    let nodes = if shape.fans_out {
        // pulse; per branch an add-constant and a chain; the fold; the sink.
        1 + width * (1 + depth) + width.saturating_sub(1) + 1
    } else {
        // pulse, the chain, the sink.
        depth + 2
    };
    Expectation {
        checksum: width * triangle(cycles) + cycles * per_cycle_constant,
        nodes,
    }
}

/// A count as the scalar a node reads it as.
fn scalar(count: u64) -> ScalarValue {
    ScalarValue::I64(i64::try_from(count).unwrap_or(i64::MAX))
}

/// A node added to a description, or why it could not be.
type Wired = Result<NodeRef, BuildError>;

/// `depth` add-one nodes in a line: the baseline's `chain`.
fn chain(builder: &mut Builder<'_>, from: NodeRef, depth: u64) -> Wired {
    let mut last = from;
    for _ in 0..depth {
        let added = builder.node("add_one", &[])?;
        builder.connect(last, added, "in")?;
        last = added;
    }
    Ok(last)
}

/// One branch of a wide chain: the branch's number added to the source, then a
/// chain of its own.
fn branch(builder: &mut Builder<'_>, source: NodeRef, number: u64, depth: u64) -> Wired {
    let start = builder.node("add_const", &[("k", scalar(number))])?;
    builder.connect(source, start, "in")?;
    chain(builder, start, depth)
}

/// The graph `bench/baselines/cpp/scenarios.cpp` composes, node for node and
/// in the order it wires them, so that both halves rank the same.
fn describe(name: &str, shape: Shape, registry: &Registry) -> Result<GraphDescription, BuildError> {
    let mut builder = Builder::new(name, registry);
    let pulse = builder.node("pulse", &[("count", scalar(shape.cycles))])?;
    let mut total = if shape.fans_out {
        branch(&mut builder, pulse, 0, shape.depth)?
    } else {
        chain(&mut builder, pulse, shape.depth)?
    };
    for number in 1..shape.width {
        let folded = branch(&mut builder, pulse, number, shape.depth)?;
        let fold = builder.node("sum", &[])?;
        builder.connect(total, fold, "lhs")?;
        builder.connect(folded, fold, "rhs")?;
        total = fold;
    }
    let sink = builder.node("checksum", &[])?;
    builder.connect(total, sink, "in")?;
    builder.finish()
}

/// The baseline's window: from the earliest start, to a little past the last
/// cycle the source asks for.
fn end_time(cycles: u64) -> Result<EngineTime, String> {
    let steps = i64::try_from(cycles.saturating_add(8)).unwrap_or(i64::MAX);
    let end = EngineTime::MIN_START.checked_add(EngineDelta::from_micros(steps));
    end.ok_or_else(|| "the run does not fit before forever".to_owned())
}

/// Build the scenario's graph and run it.
fn run(name: &str, shape: Shape) -> Result<Measured, String> {
    let mut registry = Registry::new();
    register_all(&mut registry).map_err(|error| format!("the nodes: {error:?}"))?;
    let description =
        describe(name, shape, &registry).map_err(|error| format!("the graph: {error:?}"))?;
    // The sink's id is its position in the finished description (GRF-3).
    let sink = (description.nodes.iter())
        .position(|node| node.implementation == "checksum")
        .and_then(|position| u32::try_from(position).ok())
        .ok_or("the graph has no sink")?;
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store)
        .map_err(|error| format!("instantiation: {error:?}"))?;
    let config = RunConfig {
        start_time: EngineTime::MIN_START,
        end_time: end_time(shape.cycles)?,
    };

    let begin = Instant::now();
    let outcome = run_simulation(&mut graph, &mut store, &config);
    let seconds = begin.elapsed().as_secs_f64();

    outcome.map_err(|error| format!("the run: {error:?}"))?;
    let sink = graph
        .node::<Checksum>(NodeId(sink))
        .ok_or("the sink is not a checksum")?;
    Ok(Measured {
        seconds,
        checksum: sink.total(),
        sink_evals: sink.evals(),
    })
}

/// What the run had to compute for its time to mean anything
/// (`bench/README.md`): the closed form's checksum, and one evaluation of the
/// sink in every cycle.
fn passed(measured: &Measured, expected: Expectation, cycles: u64) -> bool {
    measured.checksum == expected.checksum && measured.sink_evals == cycles
}

fn report(name: &str, shape: Shape) -> ExitCode {
    let expected = expect(shape);
    let measured = run(name, shape);
    let (seconds, checksum, sink_evals) = match &measured {
        Ok(measured) => (measured.seconds, measured.checksum, measured.sink_evals),
        Err(_) => (0.0, 0, 0),
    };
    let ok = measured
        .as_ref()
        .is_ok_and(|measured| passed(measured, expected, shape.cycles));
    let ns_per_cycle = seconds * 1e9 / precise(shape.cycles);
    println!(
        "{{\"scenario\":\"{name}\",\"impl\":\"hgl-rust\",\"cycles\":{},\"depth\":{},\"width\":{},\
         \"nodes\":{},\"run_seconds\":{seconds:.6},\"ns_per_cycle\":{ns_per_cycle:.2},\
         \"ns_per_node_eval\":{:.2},\"checksum\":{checksum},\"expected\":{},\
         \"sink_evals\":{sink_evals},\"ok\":{ok}}}",
        shape.cycles,
        shape.depth,
        shape.width,
        expected.nodes,
        ns_per_cycle / precise(expected.nodes),
        expected.checksum,
    );
    if let Err(message) = measured {
        eprintln!("{name}: {message}");
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A count as a float, for a rate. Exact below 2^53, far above any count here.
fn precise(count: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "cycle and node counts are far below 2^53"
    )]
    let value = count as f64;
    value
}

/// The tests count what a cycle allocates, which the program itself must not
/// pay for.
#[cfg(test)]
#[global_allocator]
static ALLOCATOR: hgl_alloc_count::CountingAllocator = hgl_alloc_count::CountingAllocator;

#[cfg(test)]
mod tests {
    use hgl_alloc_count::count_in;
    use hgl_describe::Registry;
    use hgl_proto_nodes::register_all;

    use super::{Measured, Shape, describe, expect, passed, run, scenario};

    /// Small enough for a test, in each shape the scenarios take.
    fn small(depth: u64, width: u64) -> Shape {
        Shape {
            cycles: 1_000,
            depth,
            width,
            fans_out: width > 1,
        }
    }

    /// The closed form's two figures: the checksum, and the nodes it is over.
    fn figures(shape: Shape) -> (u64, u64) {
        let expected = expect(shape);
        (expected.checksum, expected.nodes)
    }

    /// The figures the C++ baseline printed, and checked, on the validation host.
    #[test]
    fn expectations_match_the_cpp_baseline() {
        assert_eq!(figures(small(1, 1)), (500_500, 3));
        assert_eq!(figures(small(7, 1)), (506_500, 9));
        assert_eq!(figures(small(7, 5)), (2_542_500, 46));
    }

    /// Each scenario builds the graph the closed form counts, and computes what
    /// it predicts.
    #[test]
    fn every_scenario_computes_what_the_baseline_computes() {
        let mut registry = Registry::new();
        register_all(&mut registry).unwrap();
        for shape in [small(1, 1), small(7, 1), small(7, 5)] {
            let (checksum, nodes) = figures(shape);
            let described = describe("test", shape, &registry).unwrap();
            assert_eq!(described.nodes.len(), usize::try_from(nodes).unwrap());
            let measured = run("test", shape).unwrap();
            let computed = (measured.checksum, measured.sink_evals);
            assert_eq!(computed, (checksum, shape.cycles));
        }
    }

    /// A run that computed the wrong thing does not pass, however long it took
    /// (`bench/README.md`).
    #[test]
    fn a_run_that_computed_the_wrong_thing_does_not_pass() {
        let shape = small(7, 1);
        let (expected, cycles) = (expect(shape), shape.cycles);
        let measured = |checksum, sink_evals| Measured {
            seconds: 1.0,
            checksum,
            sink_evals,
        };
        assert!(passed(
            &measured(expected.checksum, cycles),
            expected,
            cycles
        ));
        assert!(!passed(&measured(0, cycles), expected, cycles));
        let short = measured(expected.checksum, cycles - 1);
        assert!(!passed(&short, expected, cycles));
    }

    /// Card, "Done when": the cycles of a run allocate nothing. Two runs of one
    /// graph differ only in their cycles, so a thousand more of them making not
    /// one allocation more is a thousand cycles that allocate nothing.
    #[test]
    fn a_thousand_more_cycles_of_the_chain_allocate_nothing() {
        let chain = |cycles| Shape {
            cycles,
            depth: 100,
            width: 1,
            fans_out: false,
        };
        // Whatever the first run of anything sets up once.
        assert!(run("chain", chain(10)).is_ok());
        let (short, first) = count_in(|| run("chain", chain(1_000)));
        let (long, again) = count_in(|| run("chain", chain(2_000)));
        assert_eq!(short.unwrap().sink_evals, 1_000);
        assert_eq!(long.unwrap().sink_evals, 2_000);
        assert_eq!(first, again);
    }

    #[test]
    fn defaults_are_the_baselines_defaults() {
        let args = |text: &str| -> Vec<String> { text.split(' ').map(str::to_owned).collect() };
        let (_, tick) = scenario(&args("tick")).unwrap();
        assert_eq!((tick.cycles, tick.depth, tick.width), (3_000_000, 1, 1));
        let (_, wide) = scenario(&args("wide_chain --cycles 10 --width 4")).unwrap();
        assert_eq!((wide.cycles, wide.depth, wide.width), (10, 30, 4));
        assert_eq!(expect(wide).nodes, 1 + 4 * 31 + 3 + 1);
        assert!(scenario(&args("nope")).is_err());
        assert!(scenario(&args("tick --cycles")).is_err());
    }
}
