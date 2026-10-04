//! B0 ordinary scalar global-state ownership and typed construction.
use hgl_store::{Global, Scalar, Store};
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, NodeId, Time};

type Result<T> = std::result::Result<T, Box<NodeError>>;

fn round_trip<T: Scalar>(first: &T, next: &T) -> Result<()> {
    let mut store = Store::new();
    store.global_state().provision();
    let handle = store.global_state().bind::<T>("ordinary key")?;
    let alias = store.global_state().bind::<T>("ordinary key")?;
    let missing = store
        .global_state()
        .get(handle)
        .err()
        .ok_or_else(|| NodeError::new("expected missing value"))?;
    assert!(missing.message.contains("missing value"));
    assert!(missing.message.contains("ordinary key"));
    store.global_state().set(handle, first)?;
    let retained = store.global_state().get(alias)?;
    assert_eq!(&retained, first);
    store.global_state().set(alias, next)?;
    assert_eq!(&store.global_state().get(handle)?, next);
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
            .global_state()
            .bind::<i64>("shared")
            .unwrap_err()
            .message
            .contains("unprovisioned")
    );
    first.global_state().provision();
    let count = first.global_state().bind::<i64>("shared")?;
    first.global_state().set(count, &7)?;
    assert!(
        first
            .global_state()
            .bind::<bool>("shared")
            .unwrap_err()
            .message
            .contains("type conflict")
    );
    assert_eq!(first.global_state().get(count)?, 7);
    let mut second = Store::new();
    second.global_state().provision();
    let flag = second.global_state().bind::<bool>("shared")?;
    assert!(
        second
            .global_state()
            .get(flag)
            .unwrap_err()
            .message
            .contains("missing value")
    );
    second.global_state().set(flag, &false)?;
    assert!(!second.global_state().get(flag)?);
    assert_eq!(first.global_state().get(count)?, 7);
    Ok(())
}

#[test]
fn typed_handles_survive_growth_and_nested_scope_changes() -> Result<()> {
    let mut store = Store::new();
    store.global_state().provision();
    let first = store.global_state().bind::<i64>("first")?;
    store.global_state().set(first, &42)?;
    for index in 0..100 {
        let next = store
            .global_state()
            .bind::<i64>(&format!("entry {index}"))?;
        store.global_state().set(next, &index)?;
    }
    let child = store.child_scope(NodeId(0));
    let parent = store.enter_scope(child);
    let nested = store.global_state().bind::<i64>("first")?;
    assert_eq!(store.global_state().get(nested)?, 42);
    store.global_state().set(nested, &43)?;
    store.enter_scope(parent);
    assert_eq!(store.global_state().get(first)?, 43);
    Ok(())
}

#[test]
fn text_handles_are_copy_and_values_outlive_source_and_store() -> Result<()> {
    fn copy_handle<T: Scalar>(handle: Global<T>) -> (Global<T>, Global<T>) {
        (handle, handle)
    }
    let mut store = Store::new();
    store.global_state().provision();
    let handle = store.global_state().bind::<String>("text")?;
    let (handle, alias) = copy_handle(handle);
    let mut source = "original".to_owned();
    store.global_state().set(handle, &source)?;
    source.clear();
    let mut copied = store.global_state().get(alias)?;
    copied.push_str(" changed locally");
    let retained = store.global_state().get(handle)?;
    store.global_state().set(alias, &"replacement".to_owned())?;
    drop(store);
    assert_eq!(retained, "original");
    assert_eq!(copied, "original changed locally");
    Ok(())
}
