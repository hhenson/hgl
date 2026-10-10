//! Physical fixture image implementing the authored shared owning-text contract.
//! Encodings are provider-private complete UTF-8 buffers. Borrowed callback results
//! are copied into owning HGL destinations by the explicitly selected prepared ABI.
use hgl_store::native_values::{Contract, NativeType, NativeView};
use hgl_types::{NodeError, NodeResult};
use std::cmp::Ordering;

pub(crate) const PROVIDER: &str = "hgl.native_atomic_fixture";
pub(crate) const FINGERPRINT: &str = "native-atomic-values-v1";

fn text(data: &[u8]) -> &str {
    std::str::from_utf8(data)
        .unwrap_or_else(|_| unreachable!("native text provider produces complete UTF-8"))
}
macro_rules! scalar {
    ($name:ident, $identity:literal, $capabilities:expr) => {
        #[derive(Debug)]
        pub(crate) struct $name;
        impl NativeType for $name {
            const CONTRACT: Contract = Contract {
                provider: PROVIDER,
                fingerprint: FINGERPRINT,
                canonical: $identity,
                capabilities: $capabilities,
            };
            fn preflight(data: &[u8], capacity: usize) -> NodeResult {
                if data.len() > capacity {
                    return Err(NodeError::new("native copy capacity exceeded"));
                }
                Ok(())
            }
            fn copy(data: &[u8], destination: &mut Vec<u8>) {
                destination.clear();
                destination.extend_from_slice(data);
            }
            fn text(data: &[u8]) -> NodeResult<String> {
                Ok(text(data).to_owned())
            }
            fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
                Ok(text(lhs) == text(rhs))
            }
            fn hash(data: &[u8]) -> NodeResult<u64> {
                Ok(text(data).bytes().fold(0xcbf29ce484222325, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
                }))
            }
            fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
                Ok(Some(text(lhs).cmp(text(rhs))))
            }
        }
    };
}
scalar!(Token, "hgraph.std::Token", [true; 6]);
scalar!(TokenOther, "hgraph.std::TokenOther", [true; 6]);
scalar!(
    TextOnly,
    "hgraph.std::TextOnly",
    [true, true, false, false, false, true]
);

pub(crate) fn token_str(text: &str) -> NativeView<'_, Token> {
    NativeView::from_provider(text.as_bytes())
}
pub(crate) fn token_other_str(text: &str) -> NativeView<'_, TokenOther> {
    NativeView::from_provider(text.as_bytes())
}
pub(crate) fn text_only_str(text: &str) -> NativeView<'_, TextOnly> {
    NativeView::from_provider(text.as_bytes())
}
pub(crate) fn token_text_native(value: NativeView<'_, Token>) -> &str {
    text(value.data())
}
pub(crate) fn token_other_text_native(value: NativeView<'_, TokenOther>) -> &str {
    text(value.data())
}
pub(crate) fn text_only_text_native(value: NativeView<'_, TextOnly>) -> &str {
    text(value.data())
}
