//! The concrete fixture ABI owns prepared results and hides absent checked capabilities.
#[path = "../../../native/stdlib/atomic_values.rs"]
mod image;
use hgl_store::native_values::{Native, NativeType};
use hgl_store::{PreparedValue, ValueColumns};
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeResult};
use image::{TextOnly, Token, TokenOther};
use std::{any::TypeId, cmp::Ordering};
#[global_allocator]
static ALLOCATOR: hgl_alloc_count::CountingAllocator = hgl_alloc_count::CountingAllocator;

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
    execution: &hgl_store::any_values::operations::EXECUTION,
    capture: None,
    validate: None,
    identity: TypeId::of::<TextOnly>,
    copy: |data, out| out.write(&data),
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

#[test]
fn pure_bounds_and_single_call_publication_preserve_retained_values() -> NodeResult {
    use hgl_store::{
        Store, Wake,
        shapes::{Atomic, Input, Output, Shape},
    };
    use hgl_types::{EngineTime, NodeError, NodeId};
    struct Wakes(usize);
    impl Wake for Wakes {
        fn wake(&mut self, _: NodeId) {
            self.0 += 1;
        }
    }
    type M = Native<Token>;
    assert_eq!(image::identity_width(&[0])?, 0);
    assert_eq!(image::identity_width(&[usize::MAX])?, usize::MAX);
    assert!(image::identity_width(&[]).is_err());
    let mut store = Store::new();
    let id = store.add_atomic_output::<M>(NodeId(0));
    let output = Output::<Atomic<M>>::bind(store.bindings(), id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<M>::shape(), true);
    let input =
        Input::bind(store.bindings(), input_id).map_err(|e| NodeError::new(format!("{e:?}")))?;
    store
        .bind(input_id, id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let prepared = store.prepared();
    prepared
        .atomic
        .prepare_output::<M>(prepared.bindings, id, &image::identity_width(&[8])?)?;
    let mut calls = 0;
    let mut wakes = Wakes(0);
    let (result, allocations) = hgl_alloc_count::count_in(|| {
        store
            .prepared()
            .tick(EngineTime::from_micros(1), NodeId(0), &mut wakes)
            .native_result(output, |_, _| {
                calls += 1;
                Ok(image::token_str("retained"))
            })
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(calls, 1);
    let retained = store.atomic_get::<M>(input)?;
    assert!(
        store
            .prepared()
            .tick(EngineTime::from_micros(2), NodeId(0), &mut wakes)
            .native_result(output, |_, _| {
                calls += 1;
                Ok(image::token_str("unsound provider bound: too long"))
            })
            .is_err()
    );
    assert_eq!(calls, 2);
    assert_eq!(wakes.0, 1);
    assert_eq!(
        image::token_text_native(store.atomic_get::<M>(input)?.view()),
        "retained"
    );
    store
        .prepared()
        .tick(EngineTime::from_micros(3), NodeId(0), &mut wakes)
        .native_result(output, |_, _| {
            calls += 1;
            Ok(image::token_str(""))
        })?;
    assert_eq!(calls, 3);
    assert_eq!(
        image::token_text_native(store.atomic_get::<M>(input)?.view()),
        ""
    );
    drop(store);
    assert_eq!(image::token_text_native(retained.view()), "retained");
    Ok(())
}

#[test]
fn ordinary_helper_result_uses_a_separate_reserved_owner() -> NodeResult {
    type M = Native<Token>;
    let mut globals = hgl_store::GlobalState::default();
    let source = M::allocate(globals.values_mut(), &32)?;
    let destination = M::allocate(globals.values_mut(), &8)?;
    M::copy_view(globals.values_mut(), source, image::token_str("retained"));
    let mut store = hgl_store::Store::new();
    let mut prepared = store.prepared();
    let (observation, _) = prepared.observations();
    let mut calls = 0;
    let (result, allocations) = hgl_alloc_count::count_in(|| {
        globals.native_result(observation, destination, |_, globals| {
            calls += 1;
            Ok(globals.values().native(source))
        })
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(calls, 1);
    M::copy_view(globals.values_mut(), source, image::token_str("changed"));
    assert_eq!(
        image::token_text_native(globals.values().native(destination)),
        "retained"
    );
    assert!(
        globals
            .native_result(observation, destination, |_, _| {
                calls += 1;
                Ok(image::token_str("unsound result width exceeds reserve"))
            })
            .is_err()
    );
    assert_eq!(calls, 2);
    assert_eq!(
        image::token_text_native(globals.values().native(destination)),
        "retained"
    );
    let failure = globals
        .native_result(observation, destination, |_, _| {
            calls += 1;
            Err(hgl_types::NodeError::coded(
                "provider failed",
                "provider.operation",
            ))
        })
        .unwrap_err();
    assert_eq!(failure.code, Some("provider.operation"));
    assert_eq!(calls, 3);
    assert_eq!(
        image::token_text_native(globals.values().native(destination)),
        "retained"
    );
    globals.native_result(observation, destination, |_, _| {
        calls += 1;
        Ok(image::token_str(""))
    })?;
    assert_eq!(calls, 4);
    assert!(globals.values().native(destination).data().is_empty());
    assert_eq!(
        image::token_text_native(globals.values().native(source)),
        "changed"
    );
    Ok(())
}
