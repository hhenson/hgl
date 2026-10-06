//! Sparse-entry separators do not become part of temporal key spellings.
use hgl_source::lex::lex;
#[test]
fn temporal_key_separators_preserve_literal_and_separator_spans() {
    for literal in [
        "@1970-01-01",
        "@12:30",
        "@12:30:00.123456",
        "@2026-01-01T12:30",
        "@2026-01-01T12:30Z",
        "@2026-01-01T12:30+01:00",
        "@[UTC]",
        "@12:30[UTC]",
        "@2026-01-01T12:30Z[UTC]",
    ] {
        let source = format!("[{literal}: 0]");
        let tokens = lex(&source).unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            vec!["[", literal, ":", "0", "]"],
            "{source}"
        );
        assert_eq!(&source[tokens[1].span.clone()], literal);
        assert_eq!(&source[tokens[2].span.clone()], ":");
    }
    let tokens = lex("[@1970-01-01:0]").unwrap();
    assert_eq!(tokens[1].text, "@1970-01-01");
    assert_eq!(tokens[2].text, ":");
}
