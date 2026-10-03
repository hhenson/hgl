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
        ctx.global_state().set(self.0, &0)
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let next = ctx.global_state().get(self.0)? + 1;
        ctx.global_state().set(self.0, &next)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let next = ctx.global_state().get(self.0)? + 10;
        ctx.global_state().set(self.0, &next)
    }
}
impl Buildable for Counter {
    fn node_type() -> NodeType {
        NodeType {
            name: "counter",
            uses_global_state: true,
            global_entries: vec![("count", ScalarType::I64.into())],
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
        ctx.global_state().get(self.0).map(|_| ())
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
            global_entries: vec![("count", ScalarType::Bool.into())],
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

struct Point;
impl hgl_store::GlobalValue for Point {
    type Value = (i64,);
    type Slots = hgl_store::ValueSlot<i64>;
    const WIDTH: usize = 1;
    fn prepare(
        value: &Self::Value,
        capacity: &mut hgl_store::Capacity,
        layouts: &mut hgl_store::Layouts,
    ) -> NodeResult {
        <i64 as hgl_store::GlobalValue>::prepare(&value.0, capacity, layouts)
    }
    fn install(
        columns: &mut hgl_store::ValueColumns,
        value: Self::Value,
        layouts: &mut hgl_store::Layouts,
    ) -> Self::Slots {
        hgl_store::ValueSlot::install(columns, value.0, layouts)
    }
    fn release(columns: &mut hgl_store::ValueColumns, slots: Self::Slots) {
        slots.release(columns);
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.flatten(layout);
    }
    fn schema() -> hgl_types::OrdinaryType {
        hgl_types::OrdinaryType::Struct("test::Point", vec![("x", ScalarType::I64.into())])
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        hgl_store::ValueSlot::bind(layout)
    }
    fn retain(value: &Self::Value) -> Result<Self::Value, Box<hgl_types::NodeError>> {
        Ok(*value)
    }
    fn read(
        columns: &hgl_store::ValueColumns,
        slots: Self::Slots,
    ) -> Result<Self::Value, Box<hgl_types::NodeError>> {
        Ok((slots.read(columns)?,))
    }
    fn commit(
        columns: &mut hgl_store::ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut hgl_store::Layouts,
    ) {
        slots.commit(columns, value.0, layouts);
    }
}
struct Aggregate(Global<Point>);
impl Node for Aggregate {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let root = ctx.global_state().borrow(self.0)?;
        let next = ctx.global_state().read(root.fields())? + 1;
        ctx.global_state().write(root.fields(), &next)
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let root = ctx.global_state().borrow(self.0)?;
        let next = ctx.global_state().read(root.fields())? + 2;
        ctx.global_state().write(root.fields(), &next)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let root = ctx.global_state().borrow(self.0)?;
        let next = ctx.global_state().read(root.fields())? + 3;
        ctx.global_state().write(root, &(next,))
    }
}
impl Buildable for Aggregate {
    fn node_type() -> NodeType {
        NodeType {
            name: "aggregate",
            uses_global_state: true,
            global_entries: vec![("count", <Point as hgl_store::GlobalValue>::schema())],
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self(ports.global("count")?))
    }
}

#[test]
fn aggregate_borrows_in_all_hook_phases_reach_the_run_owned_entry() {
    let mut registry = registry().unwrap();
    registry.register::<Aggregate>().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<Point>("count").unwrap();
    store.global_set(entry, &(10,)).unwrap();
    let mut graph = instantiate(
        &graph(&registry, &["aggregate", "aggregate"]).unwrap(),
        &registry,
        &mut store,
    )
    .unwrap();
    graph.start(&mut store, EngineTime::MIN_START).unwrap();
    graph.evaluate(&mut store, EngineTime::MIN_START).unwrap();
    graph.stop(&mut store, EngineTime::MIN_START).unwrap();
    assert_eq!(store.global_get(entry).unwrap(), (22,));
}

#[test]
fn aggregate_nested_preflight_rejects_scalar_and_nominal_seed_conflicts() {
    let mut registry = registry().unwrap();
    registry.register::<Aggregate>().unwrap();
    let description = nested(&registry, "aggregate", true).unwrap();
    for ty in [
        ScalarType::I64.into(),
        hgl_types::OrdinaryType::Struct("test::Different", vec![("x", ScalarType::I64.into())]),
    ] {
        let mut store = Store::new();
        store.provision_global_state();
        store.prepare_global("count", ty).unwrap();
        construction_error(&description, &registry, &mut store, "type conflict");
    }
    let mut store = Store::new();
    store.provision_global_state();
    let description = graph(&registry, &["aggregate"]).unwrap();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();
    let error = graph.start(&mut store, EngineTime::MIN_START).unwrap_err();
    assert_eq!(error.phase, Phase::Start);
    assert!(error.message.contains("missing value") && error.message.contains("count"));
}

struct OrdinaryList(Global<hgl_store::List<i64>>);
impl Node for OrdinaryList {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let slot = ctx.global_state().borrow(self.0)?;
        ctx.global_state().list_push(slot, &2)
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let slot = ctx.global_state().borrow(self.0)?;
        ctx.global_state().list_push(slot, &3)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let slot = ctx.global_state().borrow(self.0)?;
        ctx.global_state().list_push(slot, &4)
    }
}
impl Buildable for OrdinaryList {
    fn node_type() -> NodeType {
        NodeType {
            name: "ordinary_list",
            uses_global_state: true,
            global_entries: vec![(
                "count",
                <hgl_store::List<i64> as hgl_store::GlobalValue>::schema(),
            )],
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self(ports.global("count")?))
    }
}

#[test]
fn prepared_list_borrow_grows_same_entry_in_every_hook() {
    let mut registry = registry().unwrap();
    registry.register::<OrdinaryList>().unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<hgl_store::List<i64>>("count").unwrap();
    store.global_set(entry, &vec![1]).unwrap();
    let mut graph = instantiate(
        &graph(&registry, &["ordinary_list"]).unwrap(),
        &registry,
        &mut store,
    )
    .unwrap();
    graph.start(&mut store, EngineTime::MIN_START).unwrap();
    graph.evaluate(&mut store, EngineTime::MIN_START).unwrap();
    graph.stop(&mut store, EngineTime::MIN_START).unwrap();
    assert_eq!(store.global_get(entry).unwrap(), [1, 2, 3, 4]);
}

#[test]
fn nested_list_preflight_checks_exact_fixedness_and_element_identity() {
    let mut registry = registry().unwrap();
    registry.register::<OrdinaryList>().unwrap();
    let description = nested(&registry, "ordinary_list", true).unwrap();
    for ty in [
        <hgl_store::List<i64, 0> as hgl_store::GlobalValue>::schema(),
        <hgl_store::List<bool> as hgl_store::GlobalValue>::schema(),
        <hgl_store::List<hgl_store::List<i64>> as hgl_store::GlobalValue>::schema(),
    ] {
        let mut store = Store::new();
        store.provision_global_state();
        store.prepare_global("count", ty).unwrap();
        construction_error(&description, &registry, &mut store, "type conflict");
    }
}
