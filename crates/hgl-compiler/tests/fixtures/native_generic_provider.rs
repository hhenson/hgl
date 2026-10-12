#[path = "../../../../native/stdlib/atomic_values.rs"]
mod base;
pub(crate) use base::*;

pub(crate) fn token_str(
    text: &str,
) -> hgl_types::NodeResult<hgl_store::native_values::NativeView<'_, Token>> {
    crate::CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if text == "failure" {
        return Err(hgl_types::NodeError::coded(
            "provider rejected input",
            "native.generic.failure",
        ));
    }
    Ok(base::token_str(text))
}
