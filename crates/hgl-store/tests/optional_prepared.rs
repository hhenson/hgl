//! Prepared optional presence and independent descendant ownership.
use hgl_store::global_value::{GlobalValue, ValueColumns};
use hgl_store::list_storage::List;
use hgl_store::optional::Optional;
use hgl_store::prepared_value::PreparedValue;
#[global_allocator]
static ALLOCATOR: hgl_alloc_count::CountingAllocator = hgl_alloc_count::CountingAllocator;

type Payload = Optional<List<String>>;
#[test]
fn presence_replacement_retains_independent_prepared_descendants() -> hgl_types::NodeResult {
    let first = Some(vec!["a longer independent string".into(), "second".into()]);
    let smaller = Some(vec!["short".into()]);
    let empty = Some(Vec::new());
    let mut bounds = None;
    Payload::include(&mut bounds, &first);
    let mut source = ValueColumns::default();
    let mut destination = ValueColumns::default();
    let input = Payload::allocate(&mut source, &bounds)?;
    let captured = Payload::allocate(&mut destination, &bounds)?;
    let replaced = Payload::allocate(&mut destination, &bounds)?;
    assert_eq!(Payload::read(&source, input.fields())?, None);
    let (result, allocations) = hgl_alloc_count::count_in(|| -> hgl_types::NodeResult {
        for value in [&first, &None, &empty, &smaller, &first] {
            Payload::check_native(&source, input, value)?;
            Payload::copy_native(&mut source, input, value);
            Payload::check_slots(&source, input, &destination, replaced)?;
            Payload::copy_between(&source, input, &mut destination, replaced);
        }
        Payload::check_slots(&destination, replaced, &destination, captured)?;
        Payload::copy_within(&mut destination, replaced, captured);
        Payload::check_native(&destination, replaced, &smaller)?;
        Payload::copy_native(&mut destination, replaced, &smaller);
        Payload::copy_native(&mut source, input, &None);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(Payload::read(&source, input.fields())?, None);
    assert_eq!(Payload::read(&destination, captured.fields())?, first);
    assert_eq!(Payload::read(&destination, replaced.fields())?, smaller);
    Ok(())
}
#[test]
fn absent_bounds_reject_presence_without_changing_any_value() -> hgl_types::NodeResult {
    let mut columns = ValueColumns::default();
    let slot = Payload::allocate(&mut columns, &None)?;
    assert!(Payload::check_native(&columns, slot, &Some(Vec::new())).is_err());
    assert_eq!(Payload::read(&columns, slot.fields())?, None);
    let value = Some(vec!["fits".into()]);
    let mut bounds = None;
    Payload::include(&mut bounds, &value);
    let prepared = Payload::allocate(&mut columns, &bounds)?;
    Payload::check_native(&columns, prepared, &value)?;
    Payload::copy_native(&mut columns, prepared, &value);
    assert!(Payload::check_slots(&columns, prepared, &columns, slot).is_err());
    let too_large = Some(vec!["this does not fit the prepared string".into()]);
    assert!(Payload::check_native(&columns, prepared, &too_large).is_err());
    assert_eq!(Payload::read(&columns, prepared.fields())?, value);
    assert_eq!(Payload::read(&columns, slot.fields())?, None);
    Ok(())
}
