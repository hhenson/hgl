use super::{Expectation, annotations, match_errors};
use hgl_diagnostics::{Diagnostic, Issue};
fn expectation() -> Vec<Expectation> {
    vec![Expectation {
        line: 2,
        category: "type".into(),
        code: "rolling.size_kind".into(),
    }]
}
fn diagnostic(source: &str) -> Diagnostic {
    Diagnostic::new(
        source,
        "module x\ninvalid\n",
        Issue::coded("type", "rolling.size_kind", 9..10, "message may change"),
    )
}
#[test]
fn matching_requires_exact_file_and_one_to_one_primary_errors() {
    assert!(match_errors("a.hgl", &expectation(), &[diagnostic("a.hgl")]).is_ok());
    assert!(match_errors("a.hgl", &expectation(), &[diagnostic("b.hgl")]).is_err());
    assert!(
        match_errors(
            "a.hgl",
            &expectation(),
            &[diagnostic("a.hgl"), diagnostic("a.hgl")]
        )
        .is_err()
    );
    let mut uncoded = diagnostic("a.hgl");
    uncoded.issue.code = None;
    assert!(match_errors("a.hgl", &expectation(), &[uncoded]).is_err());
    assert!(match_errors("a.hgl", &expectation(), &[]).is_err());
}
#[test]
fn annotation_is_exact_standalone_lexical_comment() {
    for suffix in [" # note", " /* note */", " junk"] {
        assert!(
            annotations(&format!(
                "# expect-error(type, \"rolling.size_kind\"){suffix}\ninvalid\n"
            ))
            .is_err()
        );
    }
    let source = "/*\n# expect-error(unknown, \"unknown\")\n*/\nconst fn text() -> str => \"# expect-error(unknown, \\\"unknown\\\")\"\n# expect-error(type, \"rolling.size_kind\")\ninvalid\n";
    let expected = annotations(source).unwrap();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].line, 6);
}
#[test]
fn blank_lines_are_not_skipped_and_missing_source_line_is_invalid() {
    let expected = annotations("# expect-error(type, \"rolling.size_kind\")\n\ninvalid\n").unwrap();
    assert_eq!(expected[0].line, 2);
    assert!(annotations("# expect-error(type, \"rolling.size_kind\")\n").is_err());
}
