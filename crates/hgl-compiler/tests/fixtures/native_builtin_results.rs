pub const PROVIDER: &str = "hgl.test.builtin_results";
pub const FINGERPRINT: &str = "builtin-results-v1";
pub fn owned_bytes(value: i64) -> Vec<u8> {
    vec![u8::try_from(value).expect("fixture byte")]
}
pub fn owned_zone(value: i64) -> hgl_types::ZoneId {
    hgl_types::ZoneId::from_validated_name(if value == 1 { "UTC" } else { "Etc/UTC" }.into())
}
pub fn owned_time(value: i64) -> hgl_types::ZonedTime {
    hgl_types::ZonedTime::from_validated_parts(hgl_types::Time(value), owned_zone(1))
}
pub fn owned_datetime(value: i64) -> hgl_types::ZonedDateTime {
    hgl_types::ZonedDateTime::from_validated_parts(
        hgl_types::EngineTime::from_micros(value),
        owned_zone(1),
        0,
    )
}
pub fn fixed(value: i64) -> i64 {
    value + 1
}
pub fn builtin_width(_: &[usize]) -> hgl_types::NodeResult<usize> {
    Ok(7)
}
