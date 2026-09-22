//! GRF-10/TS-14: a boundary assembled from descendants exports the final bindings.
use super::*;
use hgl_types::NodeId;

struct Quiet;
impl hgl_store::Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn child_template(catalog: &Registry, peered_left: bool, reverse: bool) -> ChildDescription {
    let mut builder = Builder::new("descendant boundary", catalog);
    let feed = builder.node("feed", &[]).unwrap();
    let choose = builder.node("choose", &[]).unwrap();
    builder
        .connect_path(
            feed,
            vec![field("a"), field("right")],
            choose,
            "a",
            vec![field("right")],
        )
        .unwrap();
    let mut paths = if peered_left {
        vec![vec![field("left")]]
    } else {
        vec![
            vec![field("left"), Step::Index(0)],
            vec![field("left"), Step::Index(1)],
        ]
    };
    if reverse {
        paths.reverse();
    }
    ChildDescription {
        graph: builder.finish().unwrap(),
        inputs: paths
            .into_iter()
            .map(|path| Boundary {
                source_input: 0,
                source_path: path.clone(),
                target: InputPort {
                    node: 1,
                    input: 1,
                    path,
                },
            })
            .collect(),
        output: Some(output(1)),
        keyed: false,
    }
}
fn read_leaf(store: &Store, root: InputId, side: usize, index: usize) -> In<i64> {
    let bindings = store.bindings();
    store
        .scalar_input(bindings.fixed_input(bindings.fixed_input(root, side), index))
        .unwrap()
}
fn exercise(peered_left: bool, reverse: bool) {
    let catalog = registry();
    let mut store = Store::new();
    let source = store.add_shaped_output(NodeId(0), shape());
    let left = store.bindings().fixed_output(source, 0);
    for (n, value) in [7, 11].into_iter().enumerate() {
        let output = store
            .scalar_output::<i64>(store.bindings().fixed_output(left, n))
            .unwrap();
        store.set(output, value, at(0), NodeId(0), &mut Quiet);
    }
    let owner = store.add_shaped_input(NodeId(1), shape(), false);
    store.bind(owner, source).unwrap();
    let scope = store.child_scope(NodeId(1));
    let previous = store.enter_scope(scope);
    let child = instantiate_child(
        &child_template(&catalog, peered_left, reverse),
        &catalog,
        &mut store,
        &[owner],
        None,
        at(2),
    )
    .unwrap();
    let root = child.input(&input(1, 1), &store).unwrap();
    assert_eq!(store.get(read_leaf(&store, root, 0, 0)), 7);
    assert_eq!(store.get(read_leaf(&store, root, 0, 1)), 11);
    let view = store.add_shaped_input(NodeId(2), shape(), false);
    let carrier = child.outputs[1].unwrap();
    store.follow(view, carrier, at(2), &mut Quiet).unwrap();
    let designation = store.bindings().input_reference(root);
    store
        .set_reference(carrier, designation, at(2), &mut Quiet)
        .unwrap();
    assert!(
        store.input_valid(view),
        "exported root REF lost its boundary descendants"
    );
    for (n, value) in [7, 11].into_iter().enumerate() {
        let leaf = read_leaf(&store, view, 0, n);
        assert_eq!(store.get(leaf), value);
        assert_eq!(store.bindings().last_modified(leaf.id()), at(2));
    }
    let bindings = store.bindings();
    assert!(!bindings.has_peer(view));
    assert_eq!(
        bindings.has_peer(bindings.fixed_input(view, 0)),
        peered_left
    );
    assert!(bindings.has_peer(bindings.fixed_input(view, 1)));
    let internal = bindings.fixed_output(child.outputs[0].unwrap(), 1);
    let right = bindings.fixed_output(internal, 1);
    let right = store
        .scalar_output::<i64>(bindings.fixed_output(right, 0))
        .unwrap();
    store.set(right, 19, at(3), NodeId(0), &mut Quiet);
    assert_eq!(store.get(read_leaf(&store, view, 1, 0)), 19);
    assert_eq!(store.bindings().last_modified(view), at(3));
    assert_eq!(
        store
            .bindings()
            .last_modified(read_leaf(&store, view, 0, 0).id()),
        at(2)
    );
    store.enter_scope(previous);
    assert_eq!(store.bindings().output(left).modified_at, at(0));
    let updated = store
        .scalar_output::<i64>(store.bindings().fixed_output(left, 0))
        .unwrap();
    store.set(updated, 23, at(4), NodeId(0), &mut Quiet);
    assert_eq!(store.get(read_leaf(&store, view, 0, 0)), 23);
    assert_eq!(store.bindings().last_modified(view), at(4));
    assert_eq!(
        store
            .bindings()
            .last_modified(read_leaf(&store, view, 0, 1).id()),
        at(2)
    );
}
#[test]
fn descendant_boundaries_export_complete_references() {
    for peered_left in [false, true] {
        for reverse in [false, true] {
            exercise(peered_left, reverse);
        }
    }
}
