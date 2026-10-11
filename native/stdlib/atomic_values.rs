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
#[derive(Debug)]
pub(crate) struct Token;
impl NativeType for Token {
    const CONTRACT: Contract = Contract {
        provider: PROVIDER,
        fingerprint: FINGERPRINT,
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
        Ok(text(data).to_owned())
    }
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
        Ok(text(lhs) == text(rhs))
    }
    fn hash(data: &[u8]) -> NodeResult<u64> {
        Ok(text(data)
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            }))
    }
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
        Ok(Some(text(lhs).cmp(text(rhs))))
    }
}

#[derive(Debug)]
pub(crate) struct TokenOther;
impl NativeType for TokenOther {
    const CONTRACT: Contract = Contract {
        provider: PROVIDER,
        fingerprint: FINGERPRINT,
        canonical: "hgraph.std::TokenOther",
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
        Ok(text(data).to_owned())
    }
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
        Ok(text(lhs) == text(rhs))
    }
    fn hash(data: &[u8]) -> NodeResult<u64> {
        Ok(text(data)
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            }))
    }
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
        Ok(Some(text(lhs).cmp(text(rhs))))
    }
}

#[derive(Debug)]
pub(crate) struct TextOnly;
impl NativeType for TextOnly {
    const CONTRACT: Contract = Contract {
        provider: PROVIDER,
        fingerprint: FINGERPRINT,
        canonical: "hgraph.std::TextOnly",
        capabilities: [true, true, false, false, false, true],
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
        Ok(text(data).to_owned())
    }
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool> {
        Ok(text(lhs) == text(rhs))
    }
    fn hash(data: &[u8]) -> NodeResult<u64> {
        Ok(text(data)
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            }))
    }
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>> {
        Ok(Some(text(lhs).cmp(text(rhs))))
    }
}

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

/// Pure cold capacity contract for this image's identity-width UTF-8 transformations.
pub(crate) fn identity_width(arguments: &[usize]) -> NodeResult<usize> {
    match arguments {
        [width] => Ok(*width),
        _ => Err(NodeError::new("native bound arity mismatch")),
    }
}
