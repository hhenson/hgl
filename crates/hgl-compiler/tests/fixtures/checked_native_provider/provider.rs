pub const PROVIDER: &str = "hgl.test.checked_lengths";
pub const FINGERPRINT: &str = "checked-lengths-v1";
pub fn length(value: &str) -> i64 {
    i64::try_from(value.len()).unwrap_or(i64::MAX)
}
