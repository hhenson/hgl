//! Native suffix admission must preserve the contextual raises/throws distinction.
fn sources(declaration: &str) -> Vec<(String, String)> {
    vec![(
        "native_keywords.hgl".into(),
        format!("module keywords\n{declaration}\ntest usable {{ assert true }}\n"),
    )]
}
#[test]
fn unused_native_raises_suffix_is_rejected_before_test_emission() {
    for suffix in ["raises", "raises {}", "raises { cpp(){ return 1; } }"] {
        let source = sources(&format!("native const fn f()->i64 {suffix}"));
        let diagnostics = hgl_program::diagnostics(&source);
        assert_eq!(diagnostics.len(), 1, "{suffix}: {diagnostics:?}");
        assert_eq!(diagnostics[0].issue.code, Some("syntax.expected_token"));
        assert_eq!(diagnostics[0].line, 2);
        assert!(
            hgl_program::compile_tests(&source).is_err(),
            "admitted {suffix}"
        );
    }
}
#[test]
fn throws_declarations_and_opaque_provider_bodies_remain_valid() -> Result<(), String> {
    for suffix in ["throws", "throws {}", "throws { cpp(){ return 1; } }"] {
        let source = sources(&format!("native const fn f()->i64 {suffix}"));
        let diagnostics = hgl_program::diagnostics(&source);
        assert!(diagnostics.is_empty(), "{suffix}: {diagnostics:?}");
        hgl_program::compile_tests(&source)?;
    }
    let source = vec![("provider.hgl".into(), "module provider\nnative const fn f()->i64 throws\nnative const fn f()->i64 throws {}\nfn main()->i64 { yield 0us:f() }\n".into())];
    hgl_program::compile(&source, "main").map(|_| ())
}
#[test]
fn raises_remains_an_ordinary_identifier_outside_assert_context() -> Result<(), String> {
    let source = vec![("identifier.hgl".into(), "module names\nconst fn raises()->bool=>true\ntest ordinary { let result=raises()\n assert result }\n".into())];
    hgl_program::compile_tests(&source).map(|_| ())
}
