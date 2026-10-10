//! The concrete fixture ABI owns prepared results and hides absent checked capabilities.
#[path = "../../../native/stdlib/atomic_values.rs"]
mod image;
use hgl_store::native_values::{Native, NativeType};
use hgl_store::{PreparedValue, ValueColumns};
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeResult};
use image::{TextOnly, Token, TokenOther};
use std::{any::TypeId, cmp::Ordering};

#[test]
fn provider_views_materialize_independent_native_results_without_hot_allocations() -> NodeResult {
    type M = Native<Token>;
    let mut input = String::from("retained Δ text");
    let retained = image::token_str(&input).retain()?;
    input.clear();
    assert_eq!(image::token_text_native(retained.view()), "retained Δ text");
    let mut columns = ValueColumns::default();
    let destination = M::allocate(&mut columns, &32)?;
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        let view = image::token_str("prepared");
        M::check_view(&columns, destination, view)?;
        M::copy_view(&mut columns, destination, view);
        assert_eq!(
            image::token_text_native(columns.native(destination)),
            "prepared"
        );
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    let oversized = image::token_str("too long for the prepared destination native slot capacity");
    assert!(M::check_view(&columns, destination, oversized).is_err());
    assert_eq!(
        image::token_text_native(columns.native(destination)),
        "prepared"
    );
    let empty = image::token_str("");
    M::check_view(&columns, destination, empty)?;
    M::copy_view(&mut columns, destination, empty);
    assert_eq!(image::token_text_native(columns.native(destination)), "");
    assert_eq!(image::token_text_native(retained.view()), "retained Δ text");
    Ok(())
}

#[test]
fn canonical_contracts_and_provider_operations_match_the_authored_fixture() -> NodeResult {
    let token = image::token_str("same").retain()?;
    let other = image::token_other_str("same").retain()?;
    let text_only = image::text_only_str("same").retain()?;
    assert_eq!(image::token_other_text_native(other.view()), "same");
    assert_eq!(image::text_only_text_native(text_only.view()), "same");
    assert_ne!(Token::CONTRACT.canonical, TokenOther::CONTRACT.canonical);
    assert_ne!(Token::CONTRACT.canonical, TextOnly::CONTRACT.canonical);
    assert_eq!(Token::CONTRACT.capabilities, [true; 6]);
    assert_eq!(TokenOther::CONTRACT.capabilities, [true; 6]);
    assert_eq!(
        TextOnly::CONTRACT.capabilities,
        [true, true, false, false, false, true]
    );
    assert!(Token::equal(
        token.view().data(),
        image::token_str("same").data()
    )?);
    assert_eq!(
        Token::hash(token.view().data())?,
        Token::hash(image::token_str("same").data())?
    );
    assert_eq!(
        Token::compare(token.view().data(), image::token_str("z").data())?,
        Some(Ordering::Less)
    );
    assert_eq!(Token::text(token.view().data())?, "same");
    Ok(())
}

static RESTRICTED: AnyPlan = AnyPlan {
    index: 0,
    identity: TypeId::of::<TextOnly>,
    operation: AnyOperation::Native {
        equal: TextOnly::equal,
        hash: TextOnly::hash,
        compare: TextOnly::compare,
    },
    capabilities: [false; 3],
};
#[test]
fn checked_text_only_boxing_hides_extra_physical_provider_operations() {
    let a = AnyValue::from_encoded(&RESTRICTED, image::text_only_str("same").data().to_vec());
    let b = AnyValue::from_encoded(&RESTRICTED, image::text_only_str("same").data().to_vec());
    for result in [
        a.equal(&b).map(|_| ()),
        a.key_hash().map(|_| ()),
        a.compare(&b).map(|_| ()),
    ] {
        assert!(matches!(result, Err(error) if error.code == Some("value.capability")));
    }
}
