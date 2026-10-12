//! Static cold producer plans keep bounded registry identities across graph constructions.
use hgl_describe::Registry;
use hgl_types::{AnyOperation, AnyPlan, NodeResult};

static INTEGER: AnyPlan = AnyPlan {
    capture: None,
    validate: None,
    copy: |data, out| out.write(&data),
    execution: &hgl_store::any_values::operations::EXECUTION,
    identity: std::any::TypeId::of::<i64>,
    operation: AnyOperation::Integer,
    capabilities: [true; 3],
};
static OTHER_INTEGER: AnyPlan = AnyPlan {
    capture: None,
    validate: None,
    copy: |data, out| out.write(&data),
    execution: &hgl_store::any_values::operations::EXECUTION,
    identity: std::any::TypeId::of::<i64>,
    operation: AnyOperation::Integer,
    capabilities: [true; 3],
};

#[test]
fn repeated_static_plans_reuse_ids_in_clones_without_accepting_foreign_registries() -> NodeResult {
    let registry = Registry::new();
    let id = registry.retain_descriptor(&INTEGER)?;
    let clone = registry.clone();
    for _ in 0..1000 {
        assert_eq!(registry.retain_descriptor(&INTEGER)?, id);
        assert_eq!(clone.retain_descriptor(&INTEGER)?, id);
    }
    let other = registry.retain_descriptor(&OTHER_INTEGER)?;
    assert_ne!(other, id);
    assert!(std::ptr::eq(
        clone.descriptor(other)?,
        &raw const OTHER_INTEGER
    ));
    let foreign = Registry::new();
    assert!(foreign.descriptor(id).is_err());
    assert_ne!(foreign.retain_descriptor(&INTEGER)?, id);
    Ok(())
}
