//! Pure deterministic native module naming without linking or loading an image.
/// Name a checked provider module without constructing executable code.
pub fn image_module(provider: &str) -> String {
    format!("native_image{}", crate::identifiers::identifier(provider))
}
