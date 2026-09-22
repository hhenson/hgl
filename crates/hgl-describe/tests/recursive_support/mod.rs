use hgl_describe::{
    Boundary, BuildError, Buildable, Builder, ChildDescription, GraphDescription, InputPort,
    OutputPort, Ports, Registry, Step, instantiate_child, instantiate_complete,
};
use hgl_kernel::{Ctx, Node, NodeError, NodeResult, RunConfig, run_simulation};
use hgl_store::{In, InputId, Out, OutputId, Store};
use hgl_types::{EngineDelta, NodeType, ScalarType, TsType};
use std::cell::{Cell, RefCell};
mod owners;
mod snapshot;
mod sources;
use snapshot::{Observed, at, snapshot};
thread_local! {
    static OBSERVED:RefCell<Observed>=const { RefCell::new(Observed::new()) };
    static CHILD_COUNT:Cell<usize>=const{Cell::new(0)};
    static EVENTS:RefCell<Vec<&'static str>>=const{RefCell::new(Vec::new())};
}
fn event(value: &'static str) {
    EVENTS.with_borrow_mut(|v| v.push(value));
}
fn err(e: impl std::fmt::Debug) -> NodeError {
    *NodeError::new(format!("{e:?}"))
}
fn scalar() -> TsType {
    TsType::Ts(ScalarType::I64)
}
fn dict(t: TsType) -> TsType {
    TsType::Dictionary(Box::new(t))
}
fn reference(t: TsType) -> TsType {
    TsType::Reference(Box::new(t))
}
fn shape() -> TsType {
    let list = TsType::List(Box::new(scalar()), 2);
    TsType::Bundle(vec![("left".into(), list.clone()), ("right".into(), list)])
}
fn owner_inputs() -> Vec<(&'static str, TsType)> {
    vec![
        ("selectors", dict(scalar())),
        ("a", shape()),
        ("b", shape()),
    ]
}
fn leaf_out(s: &Store, id: OutputId) -> Out<i64> {
    let b = s.bindings();
    s.scalar_output(b.fixed_output(b.fixed_output(id, 0), 0))
        .unwrap()
}
fn input(node: u32, port: u32) -> InputPort {
    InputPort {
        node,
        input: port,
        path: vec![],
    }
}
fn output(node: u32) -> OutputPort {
    OutputPort { node, path: vec![] }
}
fn boundary(source_input: usize, target: InputPort) -> Boundary {
    Boundary {
        source_input,
        source_path: vec![],
        target,
    }
}
fn registry() -> Registry {
    let mut r = Registry::new();
    r.register::<sources::Feed>().unwrap();
    r.register::<sources::Choose>().unwrap();
    r.register::<sources::Route>().unwrap();
    r.register::<sources::Total>().unwrap();
    r.register::<owners::Map>().unwrap();
    r.register::<owners::Outer>().unwrap();
    r.register::<owners::Observe>().unwrap();
    r
}
fn templates(r: &Registry, capture: bool) -> ChildDescription {
    let mut b = Builder::new("key", r);
    let mut inputs = vec![];
    let result = if capture {
        b.node("total", &[]).unwrap();
        inputs.push(boundary(1, input(0, 0)));
        output(0)
    } else {
        let choose = b.node("choose", &[]).unwrap();
        let total = b.node("total", &[]).unwrap();
        b.connect(choose, total, "ts").unwrap();
        let mut key = boundary(0, input(0, 0));
        key.source_path.push(Step::Key);
        inputs.push(key);
        inputs.push(boundary(1, input(0, 1)));
        inputs.push(boundary(2, input(0, 2)));
        output(1)
    };
    let child = ChildDescription {
        graph: b.finish().unwrap(),
        inputs,
        output: Some(result),
        keyed: true,
    };
    let mut b = Builder::new("mapped", r);
    let map = b.node("map", &[]).unwrap();
    b.children(map, vec![child]).unwrap();
    ChildDescription {
        graph: b.finish().unwrap(),
        inputs: (0..3)
            .map(|n| boundary(n, input(0, u32::try_from(n).unwrap())))
            .collect(),
        output: Some(output(0)),
        keyed: false,
    }
}
fn field(s: &str) -> Step {
    Step::Field(s.into())
}
fn paths(mode: &str) -> Vec<Vec<Step>> {
    match mode {
        "owned" => vec![vec![]],
        "mixed" => vec![
            vec![field("left")],
            vec![field("right"), Step::Index(0)],
            vec![field("right"), Step::Index(1)],
        ],
        "assembled" => ["left", "right"]
            .iter()
            .flat_map(|s| (0..2).map(move |n| vec![field(s), Step::Index(n)]))
            .collect(),
        _ => unreachable!(),
    }
}
fn description(r: &Registry, case: &str) -> GraphDescription {
    let capture = case.ends_with("_capture");
    let mode = case.split('_').next().unwrap();
    let mut b = Builder::new(case, r);
    let feed = b.node("feed", &[]).unwrap();
    let route = if capture {
        Some(b.node("route", &[]).unwrap())
    } else {
        None
    };
    let outer = b.node("outer", &[]).unwrap();
    b.children(outer, vec![templates(r, capture)]).unwrap();
    let observe = b.node("observe", &[]).unwrap();
    b.connect_path(feed, vec![field("selectors")], outer, "selectors", vec![])
        .unwrap();
    b.connect(outer, observe, "rows").unwrap();
    for name in ["a", "b"] {
        let mut targets = vec![outer, observe];
        if let Some(route) = route {
            targets.push(route);
        }
        for target in targets {
            if capture && name == "a" && Some(target) != route {
                b.connect(route.unwrap(), target, name).unwrap();
                continue;
            }
            for path in paths(mode) {
                let mut source = vec![field(name)];
                source.extend(path.clone());
                b.connect_path(feed, source, target, name, path).unwrap();
            }
        }
    }
    b.finish().unwrap()
}
pub(crate) fn run(case: &str) -> usize {
    OBSERVED.with_borrow_mut(Observed::clear);
    EVENTS.with_borrow_mut(Vec::clear);
    CHILD_COUNT.with(|n| n.set(0));
    let r = registry();
    let d = description(&r, case);
    let unchanged = d.clone();
    let mut s = Store::new();
    let mut built = instantiate_complete(&d, &r, &mut s).unwrap();
    assert_eq!(d, unchanged);
    run_simulation(
        &mut built.graph,
        &mut s,
        &RunConfig {
            start_time: at(0),
            end_time: at(11),
        },
    )
    .unwrap();
    OBSERVED.with_borrow_mut(|rows| {
        rows.insert("/ticks".into(), "11".into());
        rows.insert(
            "/child_ticks".into(),
            CHILD_COUNT.with(Cell::get).to_string(),
        );
        EVENTS.with_borrow(|events| {
            rows.insert("/events".into(), events.len().to_string());
            for (n, e) in events.iter().enumerate() {
                rows.insert(format!("/events/{n}"), format!("{e:?}"));
            }
        });
        let mut count = 0;
        for line in include_str!("accepted.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            if f[0] != case {
                continue;
            }
            assert_eq!(
                rows.get(f[1]).map(String::as_str),
                Some(f[3]),
                "{case} {}",
                f[1]
            );
            count += 1;
        }
        count
    })
}
mod boundaries;
mod contracts;
