//! Keyed child schemas must fail preflight before any runtime constructor runs.
use hgl_describe::{
    BuildError, ChildDescription, GraphDescription, NodeDescription, OutputPort, Registry,
    instantiate_complete,
};
use hgl_kernel::{Ctx, Node, NodeResult};
use hgl_store::Store;
use hgl_types::{NodeType, ScalarType, TsType};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Empty;
impl Node for Empty {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
fn scalar() -> TsType {
    TsType::Ts(ScalarType::I64)
}
fn dictionary() -> TsType {
    TsType::Dictionary(Box::new(scalar()))
}
fn reference(target: TsType) -> TsType {
    TsType::Reference(Box::new(target))
}
fn description(keyed: bool) -> GraphDescription {
    GraphDescription {
        label: "owner".into(),
        nodes: vec![NodeDescription {
            implementation: "owner".into(),
            label: "owner".into(),
            scalars: vec![],
            children: vec![ChildDescription {
                graph: GraphDescription {
                    label: "child".into(),
                    nodes: vec![NodeDescription {
                        implementation: "child".into(),
                        label: "child".into(),
                        scalars: vec![],
                        children: vec![],
                    }],
                    edges: vec![],
                },
                inputs: vec![],
                output: Some(OutputPort {
                    node: 0,
                    path: vec![],
                }),
                keyed,
            }],
        }],
        edges: vec![],
    }
}
fn registry(
    owner: TsType,
    child: TsType,
    calls: &Arc<AtomicUsize>,
) -> Result<Registry, BuildError> {
    let mut registry = Registry::new();
    for (name, output, child_graphs) in [("owner", owner, 1), ("child", child, 0)] {
        let calls = Arc::clone(calls);
        registry.register_with(
            NodeType {
                name,
                output: Some(output),
                child_graphs,
                ..NodeType::default()
            },
            move |ports| {
                calls.fetch_add(1, Ordering::Relaxed);
                ports.shaped_output()?;
                Ok(Empty)
            },
        )?;
    }
    Ok(registry)
}
#[test]
fn keyed_whole_dictionary_outputs_fail_before_construction() -> Result<(), BuildError> {
    for owner in [dictionary(), reference(dictionary())] {
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = registry(owner.clone(), owner, &calls)?;
        let mut store = Store::new();
        let before = store.bindings().storage_counts();
        assert_eq!(
            instantiate_complete(&description(true), &registry, &mut store).unwrap_err(),
            BuildError::InvalidChildren("child output shape".into()),
        );
        assert_eq!(store.bindings().storage_counts(), before);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
    Ok(())
}
#[test]
fn keyed_members_and_unkeyed_reference_exports_keep_their_exact_shapes() -> Result<(), BuildError> {
    for (owner, child, keyed) in [
        (dictionary(), scalar(), true),
        (reference(dictionary()), scalar(), true),
        (reference(scalar()), reference(scalar()), false),
        (reference(scalar()), scalar(), false),
        (reference(dictionary()), reference(dictionary()), false),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = registry(owner, child, &calls)?;
        instantiate_complete(&description(keyed), &registry, &mut Store::new())?;
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
    Ok(())
}
