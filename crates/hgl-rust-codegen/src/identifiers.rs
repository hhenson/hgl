//! Exact private identifiers shared by all statically generated nominal markers.
/// Exact private identifier encoding; fixed-width octets never collide.
pub(crate) fn identifier(source: &str) -> String {
    use std::fmt::Write as _;
    source.bytes().fold(String::new(), |mut name, b| {
        write!(name, "{b:03}").unwrap_or_else(|_| unreachable!("String formatting"));
        name
    })
}

/// Exact canonical bytes form shared const-generic marker types, balanced for long names.
pub(crate) fn any_identity(source: &[u8]) -> String {
    if source.len() <= 16 {
        let mut bytes = [0; 16];
        bytes[..source.len()].copy_from_slice(source);
        return format!(
            "hgl_types::any_values::IdentityChunk<{},{}>",
            u128::from_le_bytes(bytes),
            source.len()
        );
    }
    let middle = source.len().div_ceil(16).div_ceil(2) * 16;
    format!(
        "({},{})",
        any_identity(&source[..middle]),
        any_identity(&source[middle..])
    )
}
