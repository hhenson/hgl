use super::*;
use hgl_describe::Edge;
use hgl_types::{EngineTime, NodeId};

fn rejects(d: &GraphDescription, r: &Registry) -> BuildError {
    let mut store = Store::new();
    let before = store.bindings().storage_counts();
    let error = instantiate_complete(d, r, &mut store).unwrap_err();
    assert_eq!(before, store.bindings().storage_counts());
    error
}
#[test]
fn malformed_loaded_paths_and_nested_templates_allocate_nothing() {
    let r = registry();
    let original = description(&r, "owned");
    let bad_paths = vec![
        vec![field("missing")],
        vec![Step::Index(0)],
        vec![field("a"), field("left"), Step::Index(2)],
        vec![field("selectors"), Step::Key],
        vec![field("a"), field("left"), Step::Index(0), Step::Index(0)],
    ];
    for path in bad_paths {
        let mut d = original.clone();
        d.edges[0].source.path = path;
        assert!(matches!(rejects(&d, &r), BuildError::InvalidPath(_)));
    }
    let mut d = original.clone();
    d.edges[0].source.path = vec![field("a"), field("left")];
    assert!(matches!(rejects(&d, &r), BuildError::WrongType { .. }));
    let mut d = original.clone();
    d.edges[0].target.node = 999;
    assert!(matches!(rejects(&d, &r), BuildError::UnknownInput { .. }));
    let mut d = original.clone();
    d.edges[0].source.node = 999;
    assert_eq!(rejects(&d, &r), BuildError::Cycle);
    let mut d = original.clone();
    d.nodes[1].children[0].graph.nodes[0].children[0]
        .graph
        .nodes[1]
        .implementation = "unknown".into();
    assert_eq!(
        rejects(&d, &r),
        BuildError::UnknownImplementation("unknown".into())
    );
    let mut d = original;
    d.nodes[1].children.clear();
    assert!(matches!(rejects(&d, &r), BuildError::InvalidChildren(_)));
}
#[test]
fn whole_and_descendant_overlap_in_both_orders_but_siblings_are_valid() {
    let r = registry();
    for reverse in [false, true] {
        let mut b = Builder::new("overlap", &r);
        let feed = b.node("feed", &[]).unwrap();
        let total = b.node("total", &[]).unwrap();
        let mut paths = vec![vec![], vec![field("left"), Step::Index(0)]];
        if reverse {
            paths.reverse();
        }
        for (n, path) in paths.into_iter().enumerate() {
            let mut source = vec![field("a")];
            source.extend(path.clone());
            let result = b.connect_path(feed, source, total, "ts", path);
            if n == 0 {
                result.unwrap();
            } else {
                assert!(matches!(result, Err(BuildError::InputBoundTwice { .. })));
            }
        }
    }
    let d = description(&r, "assembled");
    assert!(hgl_plan::validate(&d, &r).is_ok());
    let mut d = description(&r, "owned");
    let edge = d
        .edges
        .iter()
        .find(|e| e.target.input == 1)
        .unwrap()
        .clone();
    let mut descendant = edge;
    descendant
        .source
        .path
        .extend([field("left"), Step::Index(0)]);
    descendant
        .target
        .path
        .extend([field("left"), Step::Index(0)]);
    d.edges.push(descendant);
    assert!(matches!(
        rejects(&d, &r),
        BuildError::InputBoundTwice { .. }
    ));
}
#[test]
fn child_boundaries_cannot_override_internal_edges_or_mismatch_shapes() {
    let r = registry();
    let original = description(&r, "owned");
    let mut d = original.clone();
    let key = &mut d.nodes[1].children[0].graph.nodes[0].children[0];
    key.inputs.push(boundary(1, input(1, 0)));
    assert!(matches!(
        rejects(&d, &r),
        BuildError::InputBoundTwice { .. }
    ));
    let mut d = original.clone();
    let key = &mut d.nodes[1].children[0].graph.nodes[0].children[0];
    key.inputs[0].source_path.clear();
    assert!(matches!(rejects(&d, &r), BuildError::InvalidChildren(_)));
    let mut d = original;
    d.nodes[1].children[0].output = Some(OutputPort {
        node: 0,
        path: vec![Step::Key],
    });
    assert!(matches!(rejects(&d, &r), BuildError::InvalidPath(_)));
}
#[test]
fn keyed_child_requires_a_present_key_before_allocating() {
    let r = registry();
    let outer = templates(&r, false);
    let child = &outer.graph.nodes[0].children[0];
    let mut s = Store::new();
    let owners: Vec<_> = owner_inputs()
        .into_iter()
        .map(|(_, t)| s.add_shaped_input(NodeId(0), t, true))
        .collect();
    let before = s.bindings().storage_counts();
    assert_eq!(
        instantiate_child(child, &r, &mut s, &owners, None, at(0)).unwrap_err(),
        BuildError::MissingKey
    );
    assert!(matches!(
        instantiate_child(child, &r, &mut s, &owners, Some(7), at(0)),
        Err(BuildError::InvalidPath(_))
    ));
    assert_eq!(s.bindings().storage_counts(), before);
}
struct DuplicateFields;
impl Node for DuplicateFields {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for DuplicateFields {
    fn node_type() -> NodeType {
        NodeType {
            name: "duplicate",
            output: Some(TsType::List(
                Box::new(TsType::Bundle(vec![
                    ("x".into(), scalar()),
                    ("x".into(), scalar()),
                ])),
                1,
            )),
            ..NodeType::default()
        }
    }
    fn build(_: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}
#[test]
fn duplicate_fields_are_rejected_recursively_at_registration() {
    let mut r = Registry::new();
    assert!(matches!(
        r.register::<DuplicateFields>(),
        Err(BuildError::InvalidPath(_))
    ));
    assert!(r.node_type("duplicate").is_none());
}
#[test]
fn loaded_ref_shape_mismatch_is_rejected_before_storage() {
    let r = registry();
    let mut b = Builder::new("bad-ref", &r);
    b.node("route", &[]).unwrap();
    b.node("total", &[]).unwrap();
    let mut d = b.finish().unwrap();
    d.edges.push(Edge {
        source: output(0),
        target: InputPort {
            node: 1,
            input: 0,
            path: vec![field("left")],
        },
    });
    assert!(matches!(rejects(&d, &r), BuildError::WrongType { .. }));
}
#[test]
fn retained_boundaries_validate_paths_even_after_construction() {
    let r = registry();
    let mut s = Store::new();
    let built = instantiate_complete(&description(&r, "owned"), &r, &mut s).unwrap();
    assert!(matches!(
        built.output(
            &OutputPort {
                node: 0,
                path: vec![field("a"), Step::Index(0)]
            },
            &s
        ),
        Err(BuildError::InvalidPath(_))
    ));
    assert!(matches!(
        built.input(
            &InputPort {
                node: 1,
                input: 0,
                path: vec![Step::Key]
            },
            &s
        ),
        Err(BuildError::MissingKey)
    ));
    assert!(built.output(&output(999), &s).is_err());
}
#[test]
fn one_template_makes_independent_ports_and_keeps_its_data() {
    let catalog = registry();
    let d = description(&catalog, "mixed_capture");
    let copy = d.clone();
    let mut s = Store::new();
    let a = instantiate_complete(&d, &catalog, &mut s).unwrap();
    let b = instantiate_complete(&d, &catalog, &mut s).unwrap();
    assert_ne!(a.outputs, b.outputs);
    assert_ne!(a.inputs, b.inputs);
    assert_eq!(d, copy);
    let mut s1 = Store::new();
    let mut s2 = Store::new();
    let mut a = instantiate_complete(&d, &catalog, &mut s1).unwrap();
    let b = instantiate_complete(&d, &catalog, &mut s2).unwrap();
    run_simulation(
        &mut a.graph,
        &mut s1,
        &RunConfig {
            start_time: at(0),
            end_time: at(2),
        },
    )
    .unwrap();
    assert!(s1.bindings().output(a.outputs[0].unwrap()).modified_at > EngineTime::NEVER);
    assert_eq!(
        s2.bindings().output(b.outputs[0].unwrap()).modified_at,
        EngineTime::NEVER
    );
}

struct Quiet;
impl hgl_store::Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
#[test]
fn projecting_through_a_live_capture_is_refused_before_allocation() {
    let catalog = registry();
    let mut store = Store::new();
    let carrier = store.add_shaped_output(NodeId(0), reference(shape()));
    let owner = store.add_shaped_input(NodeId(1), shape(), false);
    store.follow(owner, carrier, at(0), &mut Quiet).unwrap();
    let mut builder = Builder::new("projection", &catalog);
    builder.node("total", &[]).unwrap();
    let mut edge = boundary(0, input(0, 0));
    edge.source_path = vec![field("left")];
    edge.target.path = vec![field("left")];
    let child = ChildDescription {
        graph: builder.finish().unwrap(),
        inputs: vec![edge],
        output: Some(output(0)),
        keyed: false,
    };
    let before = store.bindings().storage_counts();
    assert!(matches!(
        instantiate_child(&child, &catalog, &mut store, &[owner], None, at(0)),
        Err(BuildError::InvalidPath(_))
    ));
    assert_eq!(store.bindings().storage_counts(), before);
}

#[test]
fn sibling_edges_have_canonical_path_order() {
    let catalog = registry();
    let build = |reverse: bool| {
        let mut builder = Builder::new("siblings", &catalog);
        let feed = builder.node("feed", &[]).unwrap();
        let total = builder.node("total", &[]).unwrap();
        let mut paths = paths("assembled");
        if reverse {
            paths.reverse();
        }
        for path in paths {
            let mut source = vec![field("a")];
            source.extend(path.clone());
            builder
                .connect_path(feed, source, total, "ts", path)
                .unwrap();
        }
        builder.finish().unwrap()
    };
    assert_eq!(build(false), build(true));
}
#[test]
fn ranking_remaps_parent_edges_without_changing_child_local_ports() {
    let catalog = registry();
    let template = templates(&catalog, false);
    let mut builder = Builder::new("ranking", &catalog);
    let owner = builder.node("outer", &[]).unwrap();
    builder.children(owner, vec![template.clone()]).unwrap();
    let feed = builder.node("feed", &[]).unwrap();
    for name in ["selectors", "a", "b"] {
        builder
            .connect_path(feed, vec![field(name)], owner, name, vec![])
            .unwrap();
    }
    let plan = builder.finish().unwrap();
    assert_eq!(plan.nodes[0].implementation, "feed");
    assert_eq!(plan.nodes[1].children, [template]);
    assert!(
        plan.edges
            .iter()
            .all(|edge| edge.source.node == 0 && edge.target.node == 1)
    );
}
