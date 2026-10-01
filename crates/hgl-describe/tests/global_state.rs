//! B0 typed bindings must be reconciled before root construction and start.
use hgl_describe::{
    BuildError, Buildable, Builder, ChildDescription, GraphDescription, Ports, Registry,
    instantiate,
};
use hgl_kernel::{Ctx, Node, NodeResult, Phase};
use hgl_store::{Global, Store};
use hgl_types::{EngineTime, NodeType, ScalarType};

struct Counter(Global<i64>);
impl Node for Counter {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.global_set(self.0, &0)
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let next = ctx.global_get(self.0)? + 1;
        ctx.global_set(self.0, &next)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let next = ctx.global_get(self.0)? + 10;
        ctx.global_set(self.0, &next)
    }
}
impl Buildable for Counter {
    fn node_type() -> NodeType {
        NodeType {
            name: "counter",
            uses_global_state: true,
            global_entries: vec![("count", ScalarType::I64)],
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self(ports.global("count")?))
    }
}

struct Reader(Global<bool>);
impl Node for Reader {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.global_get(self.0).map(|_| ())
    }
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Reader {
    fn node_type() -> NodeType {
        NodeType {
            name: "reader",
            uses_global_state: true,
            global_entries: vec![("count", ScalarType::Bool)],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self(ports.global("count")?))
    }
}

struct Owner;
impl Node for Owner {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Owner {
    fn node_type() -> NodeType {
        NodeType {
            name: "owner",
            child_graphs: 1,
            ..NodeType::default()
        }
    }
    fn build(_: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

struct Guard;
impl Node for Guard {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Guard {
    fn node_type() -> NodeType {
        NodeType {
            name: "guard",
            ..NodeType::default()
        }
    }
    fn build(_: &mut Ports<'_>) -> Result<Self, BuildError> {
        Err(BuildError::InvalidNodeType {
            node: "guard",
            what: "constructor reached before preflight".into(),
        })
    }
}

struct Undeclared;
impl Node for Undeclared {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Undeclared {
    fn node_type() -> NodeType {
        NodeType {
            name: "undeclared",
            uses_global_state: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        let _: Global<i64> = ports.global("secret")?;
        Ok(Self)
    }
}

fn registry() -> Result<Registry, BuildError> {
    let mut registry = Registry::new();
    registry.register::<Counter>()?;
    registry.register::<Reader>()?;
    registry.register::<Owner>()?;
    registry.register::<Guard>()?;
    registry.register::<Undeclared>()?;
    Ok(registry)
}
fn graph(registry: &Registry, nodes: &[&str]) -> Result<GraphDescription, BuildError> {
    let mut builder = Builder::new("globals", registry);
    for name in nodes {
        builder.node(name, &[])?;
    }
    builder.finish()
}
fn nested(registry: &Registry, child: &str, guard: bool) -> Result<GraphDescription, BuildError> {
    let template = ChildDescription {
        graph: graph(registry, &[child])?,
        inputs: vec![],
        output: None,
        keyed: false,
    };
    let mut builder = Builder::new("nested globals", registry);
    if guard {
        builder.node("guard", &[])?;
    }
    let owner = builder.node("owner", &[])?;
    builder.children(owner, vec![template])?;
    builder.finish()
}
fn construction_error(
    description: &GraphDescription,
    registry: &Registry,
    store: &mut Store,
    text: &str,
) {
    let result = instantiate(description, registry, store);
    assert!(
        matches!(&result, Err(error) if format!("{error:?}").contains(text)),
        "{result:?}"
    );
}

#[test]
fn all_hook_phases_share_the_same_typed_entry_and_final_owner_read() {
    let registry = registry().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let handle = store.bind_global::<i64>("count").unwrap();
    let mut graph = instantiate(
        &graph(&registry, &["counter", "counter"]).unwrap(),
        &registry,
        &mut store,
    )
    .unwrap();
    assert!(
        store.global_get(handle).is_err(),
        "construction must not initialize"
    );
    graph.start(&mut store, EngineTime::MIN_START).unwrap();
    graph.evaluate(&mut store, EngineTime::MIN_START).unwrap();
    graph.stop(&mut store, EngineTime::MIN_START).unwrap();
    assert_eq!(store.global_get(handle).unwrap(), 22);
}

#[test]
fn absent_prepared_value_is_a_hook_error_not_a_construction_error() {
    let registry = registry().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let mut graph = instantiate(
        &graph(&registry, &["reader"]).unwrap(),
        &registry,
        &mut store,
    )
    .unwrap();
    let error = graph.start(&mut store, EngineTime::MIN_START).unwrap_err();
    assert_eq!(error.phase, Phase::Start);
    assert!(error.message.contains("missing value"));
    assert!(error.message.contains("count"));
}

#[test]
fn unprovisioned_nested_requirement_fails_before_any_root_constructor() {
    let registry = registry().unwrap();
    construction_error(
        &nested(&registry, "reader", true).unwrap(),
        &registry,
        &mut Store::new(),
        "unprovisioned",
    );
}

#[test]
fn incompatible_seed_and_nested_requirements_fail_in_preflight() {
    let registry = registry().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let seed = store.bind_global::<i64>("count").unwrap();
    store.global_set(seed, &7).unwrap();
    construction_error(
        &nested(&registry, "reader", true).unwrap(),
        &registry,
        &mut store,
        "type conflict",
    );
    assert_eq!(store.global_get(seed).unwrap(), 7);
    let mut store = Store::new();
    store.provision_global_state();
    let mut description = nested(&registry, "reader", true).unwrap();
    description
        .nodes
        .insert(1, graph(&registry, &["counter"]).unwrap().nodes.remove(0));
    construction_error(&description, &registry, &mut store, "type conflict");
}

#[test]
fn native_constructor_cannot_request_an_undeclared_key() {
    let registry = registry().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    construction_error(
        &graph(&registry, &["undeclared"]).unwrap(),
        &registry,
        &mut store,
        "undeclared key/type",
    );
}
