//! Native storage invokes the selected provider while retaining independent typed buffers.
use hgl_store::native_values::{Contract, Native, NativeType, NativeValue};
use hgl_store::{GlobalState, GlobalValue, PreparedValue, ValueColumns};
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::cmp::Ordering;
#[derive(Debug)]
struct Token;
impl NativeType for Token {
    const CONTRACT: Contract = Contract {
        provider: "hgl.native_atomic_fixture",
        fingerprint: "native-atomic-values-v1",
        canonical: "hgraph.std::Token",
        capabilities: [true; 6],
    };
    fn preflight(data: &[u8], capacity: usize) -> NodeResult {
        if data.len() > capacity {
            return Err(NodeError::new("native copy capacity exceeded"));
        }
        Ok(())
    }
    fn copy(data: &[u8], destination: &mut dyn hgl_types::any_values::AnyBuffer) {
        destination.write(data);
    }
    fn text(data: &[u8]) -> NodeResult<String> {
        String::from_utf8(data.to_vec()).map_err(|e| NodeError::new(e.to_string()))
    }
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
        Ok(lhs == rhs)
    }
    fn hash(data: &[u8]) -> NodeResult<u64> {
        Ok(data
            .iter()
            .fold(0, |s, b| s.wrapping_mul(31).wrapping_add(u64::from(*b))))
    }
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
        Ok(Some(lhs.cmp(rhs)))
    }
}
fn value(text: &str) -> NativeValue<Token> {
    NativeValue::from_owned(text.as_bytes().to_vec())
}
#[test]
fn prepared_copies_are_independent_reusable_and_failure_preserving() -> NodeResult {
    type M = Native<Token>;
    let mut source = ValueColumns::default();
    let from = M::allocate(&mut source, &128)?;
    let original = value("retained");
    M::check_native(&source, from, &original)?;
    M::copy_native(&mut source, from, &original);
    let mut destination = ValueColumns::default();
    let to = M::allocate(&mut destination, &8)?;
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        M::check_slots(&source, from, &destination, to)?;
        M::copy_between(&source, from, &mut destination, to);
        M::copy_within(&mut destination, to, to);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    let replacement = value("replacement is too long");
    M::check_native(&source, from, &replacement)?;
    M::copy_native(&mut source, from, &replacement);
    assert!(M::check_slots(&source, from, &destination, to).is_err());
    assert_eq!(Token::text(destination.native(to).data())?, "retained");
    let empty = value("");
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        M::check_native(&source, from, &empty)?;
        M::copy_native(&mut source, from, &empty);
        M::check_slots(&source, from, &destination, to)?;
        M::copy_between(&source, from, &mut destination, to);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(destination.native(to).data().is_empty());
    assert_eq!(M::schema(), OrdinaryType::Native("hgraph.std::Token"));
    Ok(())
}
#[test]
fn typed_global_read_retains_independent_ordinary_native_data() -> NodeResult {
    type M = Native<Token>;
    let mut globals = GlobalState::default();
    globals.provision();
    let handle = globals.bind::<M>("native.saved")?;
    globals.set(handle, &value("first"))?;
    let mut copied = globals.get(handle)?;
    assert_eq!(Token::text(copied.view().data())?, "first");
    copied = value("second");
    assert_eq!(Token::text(copied.view().data())?, "second");
    assert_eq!(Token::text(globals.get(handle)?.view().data())?, "first");
    Ok(())
}
#[test]
fn image_identity_and_required_capabilities_are_checked_before_execution() -> NodeResult {
    let contract = Token::CONTRACT;
    contract.validate(
        "hgl.native_atomic_fixture",
        "native-atomic-values-v1",
        "hgraph.std::Token",
        [true; 6],
    )?;
    assert!(
        contract
            .validate(
                "another.provider",
                contract.fingerprint,
                contract.canonical,
                [true; 6]
            )
            .is_err()
    );
    assert!(
        contract
            .validate(
                contract.provider,
                "another.version",
                contract.canonical,
                [true; 6]
            )
            .is_err()
    );
    assert!(
        contract
            .validate(
                contract.provider,
                contract.fingerprint,
                "hgraph.std::TokenOther",
                [true; 6]
            )
            .is_err()
    );
    let missing = Contract {
        capabilities: [true, true, false, false, false, true],
        ..contract
    };
    assert!(
        missing
            .validate(
                missing.provider,
                missing.fingerprint,
                missing.canonical,
                [true; 6]
            )
            .is_err()
    );
    missing.validate(
        missing.provider,
        missing.fingerprint,
        missing.canonical,
        [true, true, false, false, false, true],
    )?;
    Ok(())
}
