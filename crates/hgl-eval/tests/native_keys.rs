//! Cold provider identities never use encoding bytes or invented provider order.
use hgl_eval::{
    ir::{DeltaEntry, Kind, Value},
    key_context::Context,
    value_eval::EvalError,
};
use hgl_types::{NodeError, NodeResult};
use hgl_value_model::{
    Ty,
    native_value::{NativeValue, Operations},
};
use std::{
    cmp::Ordering,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    time::Instant,
};
static COPIES: AtomicUsize = AtomicUsize::new(0);
static HASHES: AtomicUsize = AtomicUsize::new(0);
static EQUALS: AtomicUsize = AtomicUsize::new(0);
fn identity(data: &[u8]) -> NodeResult<u64> {
    let bytes = data
        .get(..8)
        .ok_or_else(|| NodeError::new("provider encoding"))?;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .map_err(|e| NodeError::new(format!("{e}")))?,
    ))
}
fn copy(data: &[u8], destination: &mut Vec<u8>) {
    COPIES.fetch_add(1, Relaxed);
    destination.clear();
    destination.extend_from_slice(data);
}
fn equal(a: &[u8], b: &[u8]) -> NodeResult<bool> {
    EQUALS.fetch_add(1, Relaxed);
    Ok(identity(a)? == identity(b)?)
}
fn hash(data: &[u8]) -> NodeResult<u64> {
    HASHES.fetch_add(1, Relaxed);
    identity(data)
}
fn compare(_: &[u8], _: &[u8]) -> NodeResult<Option<Ordering>> {
    Err(NodeError::new("key use must not request ordering"))
}
static OPS: Operations = Operations {
    copy,
    equal,
    hash,
    compare,
};
static COLLIDING: Operations = Operations {
    copy,
    equal,
    hash: |_| Ok(0),
    compare,
};
static BAD_HASH: Operations = Operations {
    copy,
    equal,
    hash: |_| Err(NodeError::coded("provider hash failed", "provider.hash")),
    compare,
};
static BAD_EQUAL: Operations = Operations {
    copy,
    equal: |_, _| {
        Err(NodeError::coded(
            "provider equality failed",
            "provider.equal",
        ))
    },
    hash,
    compare,
};
fn value(name: &str, id: u64, tag: u8, ops: &'static Operations) -> Value {
    let mut data = id.to_le_bytes().to_vec();
    data.push(tag);
    Value::new(
        Ty::Native(hgl_value_model::native_contracts::Scalar {
            identity: name.into(),
            capabilities: ["copy", "text", "equality", "hash"]
                .into_iter()
                .map(String::from)
                .collect(),
        }),
        Kind::Opaque(NativeValue::new(data, ops)),
    )
}
#[test]
fn provider_identity_collisions_failures_and_scaling() -> Result<(), EvalError> {
    let mut context = Context::default();
    let first = context.key(&value("first", 1, 0, &COLLIDING))?;
    let second = context.key(&value("first", 2, 0, &COLLIDING))?;
    assert_ne!(first, second);
    assert_eq!(first, context.key(&value("first", 1, 99, &COLLIDING))?);
    assert_ne!(first, context.key(&value("second", 1, 0, &COLLIDING))?);
    assert!(matches!(
        context.key(&value("failure", 1, 0, &BAD_HASH)),
        Err(EvalError::Coded("provider.hash", _))
    ));
    context.key(&value("failure", 1, 0, &BAD_EQUAL))?;
    assert!(matches!(
        context.key(&value("failure", 1, 1, &BAD_EQUAL)),
        Err(EvalError::Coded("provider.equal", _))
    ));
    let first = value("duplicates", 1, 0, &COLLIDING);
    let same = value("duplicates", 1, 99, &COLLIDING);
    let set = Ty::Set(Box::new(first.ty.clone()));
    assert_eq!(
        hgl_eval::collection_values::evaluate(&set, &[first.clone(), same.clone()], |v| Ok::<
            _,
            EvalError,
        >(
            v.clone()
        ))
        .map(|_| ()),
        Err(EvalError::Operation(
            "duplicate ordinary collection key or member".into()
        ))
    );
    assert_eq!(
        hgl_eval::publication_trace::materialized(&[
            DeltaEntry::Add(first.clone()),
            DeltaEntry::Remove(same.clone())
        ]),
        Err(EvalError::Operation(
            "delta additions/upserts overlap removals".into()
        ))
    );
    assert_eq!(
        hgl_eval::publication_trace::materialized(&[DeltaEntry::Add(first), DeltaEntry::Add(same)]),
        Err(EvalError::Operation("duplicate delta key/member".into()))
    );
    for size in [256_u64, 512, 1024, 2048] {
        for counter in [&COPIES, &HASHES, &EQUALS] {
            counter.store(0, Relaxed);
        }
        let start = Instant::now();
        let mut context = Context::default();
        let expected = (0..size)
            .map(|id| context.key(&value("scaling", id, 0, &OPS)))
            .collect::<Result<Vec<_>, _>>()?;
        for id in (0..size).rev() {
            assert_eq!(
                context.key(&value("scaling", id, 1, &OPS))?,
                expected[usize::try_from(id).map_err(|e| e.to_string())?]
            );
        }
        let size = usize::try_from(size).map_err(|e| e.to_string())?;
        assert_eq!(COPIES.load(Relaxed), size);
        assert_eq!(HASHES.load(Relaxed), 2 * size);
        assert_eq!(EQUALS.load(Relaxed), size);
        eprintln!(
            "native cold identities n={size}, hash={}, equal={}, elapsed={:?}",
            HASHES.load(Relaxed),
            EQUALS.load(Relaxed),
            start.elapsed()
        );
    }
    Ok(())
}
