//! Described graphs replay the independently validated boundary corpus.
#[cfg(test)]
mod recursive_support;
#[test]
fn validated_nested_descriptions() {
    let count = [
        "owned",
        "assembled",
        "mixed",
        "owned_capture",
        "mixed_capture",
    ]
    .iter()
    .map(|c| recursive_support::run(c))
    .sum::<usize>();
    assert_eq!(count, 9990);
}
