//! Non-inserting map selection preserves provider failures and exact key identity.
#[global_allocator]
static ALLOCATOR: hgl_alloc_count::CountingAllocator = hgl_alloc_count::CountingAllocator;
use hgl_store::native_values::{Contract, Native, NativeType, NativeValue};
use hgl_store::{Key, Keys, NativeKeyDomain};
use hgl_types::{NodeError, NodeResult};
use std::{cmp::Ordering, sync::OnceLock};
#[derive(Debug)]
struct Token;
impl NativeType for Token {
    const CONTRACT: Contract = Contract {
        provider: "lookup-test",
        fingerprint: "1",
        canonical: "lookup-test::Token",
        capabilities: [true; 6],
    };
    fn preflight(data: &[u8], capacity: usize) -> NodeResult {
        if data.len() > capacity {
            return Err(NodeError::new("copy capacity"));
        }
        Ok(())
    }
    fn copy(data: &[u8], destination: &mut dyn hgl_types::any_values::AnyBuffer) {
        destination.write(data);
    }
    fn text(data: &[u8]) -> NodeResult<String> {
        Ok(format!("{data:?}"))
    }
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
        if rhs == [2] {
            return Err(NodeError::coded("provider equality failed", "lookup.equal"));
        }
        Ok(lhs == rhs)
    }
    fn hash(data: &[u8]) -> NodeResult<u64> {
        if data.is_empty() {
            return Err(NodeError::coded("provider hash failed", "lookup.hash"));
        }
        Ok(0)
    }
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
        Ok(Some(lhs.cmp(rhs)))
    }
}
#[derive(Debug)]
struct Domain;
impl NativeKeyDomain for Domain {
    type Scalar = Token;
    const MAPPING: (&'static str, &'static str) = ("lookup-test", "Token");
    fn slot() -> &'static OnceLock<usize> {
        static SLOT: OnceLock<usize> = OnceLock::new();
        &SLOT
    }
}
#[test]
fn absent_native_lookup_preserves_hash_and_collision_equality_failures() -> NodeResult {
    type Marker = Native<Token, Domain>;
    Domain::prepare()?;
    let mut keys = Keys::default();
    let present = NativeValue::from_owned(vec![1]);
    let absent = NativeValue::from_owned(vec![3]);
    let hash_failure = NativeValue::from_owned(vec![]);
    let equality_failure = NativeValue::from_owned(vec![2]);
    assert_eq!(Marker::find(&keys, &absent)?, None);
    assert_eq!(
        Marker::find(&keys, &hash_failure).unwrap_err().code,
        Some("lookup.hash")
    );
    Marker::prepare(&mut keys, &present)?;
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        assert_eq!(Marker::find(&keys, &present)?, Some(0));
        assert_eq!(Marker::find(&keys, &absent)?, None);
        assert_eq!(Marker::ids(&keys), [0]);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(
        Marker::key_view_find(&keys, hash_failure.view())
            .unwrap_err()
            .code,
        Some("lookup.hash")
    );
    assert_eq!(
        Marker::key_view_find(&keys, equality_failure.view())
            .unwrap_err()
            .code,
        Some("lookup.equal")
    );
    Ok(())
}
