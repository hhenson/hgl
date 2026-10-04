//! Prepared primitive globals allocate nothing during successful repeated access.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::Store;
use hgl_types::NodeError;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn typed_primitive_access_uses_prepared_slots() -> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    store.global_state().provision();
    let handle = store.global_state().bind::<i64>("count")?;
    store.global_state().set(handle, &0)?;
    let (result, allocations) = count_in(|| {
        for _ in 0..10_000 {
            let previous = store.global_state().get(handle)?;
            store.global_state().set(handle, &(previous + 1))?;
        }
        store.global_state().get(handle)
    });
    assert_eq!(result?, 10_000);
    assert_eq!(allocations, 0);
    Ok(())
}
