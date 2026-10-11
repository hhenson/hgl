//! Exact private identifiers shared by all statically generated nominal markers.
/// Exact private identifier encoding; fixed-width octets never collide.
pub(crate) fn identifier(source: &str) -> String {
    use std::fmt::Write as _;
    source.bytes().fold(String::new(), |mut name, b| {
        write!(name, "{b:03}").unwrap_or_else(|_| unreachable!("String formatting"));
        name
    })
}
