//! B0 ordinary scalar global-state ownership and typed construction.
use hgl_store::{Global, Scalar, Store};
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, NodeId, Time};

type Result<T> = std::result::Result<T, Box<NodeError>>;

fn round_trip<T: Scalar>(first: &T, next: &T) -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let handle = store.bind_global::<T>("ordinary key")?;
    let alias = store.bind_global::<T>("ordinary key")?;
    let missing = store
        .global_get(handle)
        .err()
        .ok_or_else(|| NodeError::new("expected missing value"))?;
    assert!(missing.message.contains("missing value"));
    assert!(missing.message.contains("ordinary key"));
    store.global_set(handle, first)?;
    let retained = store.global_get(alias)?;
    assert_eq!(&retained, first);
    store.global_set(alias, next)?;
    assert_eq!(&store.global_get(handle)?, next);
    assert_eq!(&retained, first);
    Ok(())
}

#[test]
fn all_eight_scalar_types_distinguish_absence_from_values() -> Result<()> {
    round_trip(&false, &true)?;
    round_trip(&0_i64, &-3)?;
    round_trip(&0.0_f64, &1.5)?;
    round_trip(&String::new(), &"changed".to_owned())?;
    round_trip(&Date(0), &Date(1))?;
    round_trip(&Time(0), &Time(1))?;
    round_trip(&EngineTime::from_micros(0), &EngineTime::MIN_START)?;
    round_trip(&EngineDelta::from_micros(0), &EngineDelta::STEP)
}

#[test]
fn provisioning_type_conflicts_and_independent_runs() -> Result<()> {
    let mut first = Store::new();
    assert!(
        first
            .bind_global::<i64>("shared")
            .unwrap_err()
            .message
            .contains("unprovisioned")
    );
    first.provision_global_state();
    let count = first.bind_global::<i64>("shared")?;
    first.global_set(count, &7)?;
    assert!(
        first
            .bind_global::<bool>("shared")
            .unwrap_err()
            .message
            .contains("type conflict")
    );
    assert_eq!(first.global_get(count)?, 7);
    let mut second = Store::new();
    second.provision_global_state();
    let flag = second.bind_global::<bool>("shared")?;
    assert!(
        second
            .global_get(flag)
            .unwrap_err()
            .message
            .contains("missing value")
    );
    second.global_set(flag, &false)?;
    assert!(!second.global_get(flag)?);
    assert_eq!(first.global_get(count)?, 7);
    Ok(())
}

#[test]
fn typed_handles_survive_growth_and_nested_scope_changes() -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let first = store.bind_global::<i64>("first")?;
    store.global_set(first, &42)?;
    for index in 0..100 {
        let next = store.bind_global::<i64>(&format!("entry {index}"))?;
        store.global_set(next, &index)?;
    }
    let child = store.child_scope(NodeId(0));
    let parent = store.enter_scope(child);
    let nested = store.bind_global::<i64>("first")?;
    assert_eq!(store.global_get(nested)?, 42);
    store.global_set(nested, &43)?;
    store.enter_scope(parent);
    assert_eq!(store.global_get(first)?, 43);
    Ok(())
}

#[test]
fn text_handles_are_copy_and_values_outlive_source_and_store() -> Result<()> {
    fn copy_handle<T: Scalar>(handle: Global<T>) -> (Global<T>, Global<T>) {
        (handle, handle)
    }
    let mut store = Store::new();
    store.provision_global_state();
    let handle = store.bind_global::<String>("text")?;
    let (handle, alias) = copy_handle(handle);
    let mut source = "original".to_owned();
    store.global_set(handle, &source)?;
    source.clear();
    let mut copied = store.global_get(alias)?;
    copied.push_str(" changed locally");
    let retained = store.global_get(handle)?;
    store.global_set(alias, &"replacement".to_owned())?;
    drop(store);
    assert_eq!(retained, "original");
    assert_eq!(copied, "original changed locally");
    Ok(())
}
