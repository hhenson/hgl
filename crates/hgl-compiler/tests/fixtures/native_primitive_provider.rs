#[path = "../../../../native/stdlib/atomic_values.rs"]
mod fixture;
pub(crate) use fixture::*;
use hgl_store::native_values::NativeView;

pub(crate) fn primitive_text(value: i64, flag: bool, fixed: i64) -> &'static str {
    crate::NATIVE_CALLS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if value == 1 && flag && fixed == 1 {
        "yes"
    } else {
        "no"
    }
}
pub(crate) fn primitive_token(value: i64, flag: bool, fixed: i64) -> NativeView<'static, Token> {
    crate::NATIVE_CALLS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    NativeView::from_provider(primitive_text(value, flag, fixed).as_bytes())
}
pub(crate) fn primitive_size(value: &str) -> i64 {
    crate::NATIVE_CALLS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    i64::try_from(value.len()).unwrap_or(i64::MAX)
}
pub(crate) fn primitive_width(_: &[usize]) -> hgl_types::NodeResult<usize> {
    Ok(3)
}
