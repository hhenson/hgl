//! Source failure clauses reject; selected opaque Rust ABI metadata remains private.
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
fn obsolete_throws_suffix_rejects_and_opaque_abi_metadata_is_private() -> Result<(), String> {
    for suffix in ["throws", "throws {}", "throws { cpp(){ return 1; } }"] {
        let source = sources(&format!("native const fn f()->i64 {suffix}"));
        let diagnostics = hgl_program::diagnostics(&source);
        assert_eq!(diagnostics.len(), 1, "{suffix}: {diagnostics:?}");
        assert_eq!(diagnostics[0].issue.code, Some("syntax.expected_token"));
        assert!(hgl_program::compile_tests(&source).is_err());
    }
    for suffix in [
        "",
        "{}",
        "{ cpp(){ return 1; } }",
        "{ rust_abi_node_result }",
    ] {
        let source = sources(&format!("native const fn f()->i64 {suffix}"));
        assert!(hgl_program::diagnostics(&source).is_empty());
        hgl_program::compile_tests(&source)?;
    }
    let source=vec![("provider.hgl".into(),"module provider\nnative const fn f()->i64\nnative const fn f()->i64 { rust_abi_node_result }\nfn main()->i64 { yield 0us:f() }\n".into())];
    let plan = hgl_program::compile(&source, "main")?;
    assert!(
        hgl_program::emit_rust(&plan).contains("fn f_()->Result<i64,Box<hgl_kernel::NodeError>>"),
        "private ABI still translates operational failure"
    );
    Ok(())
}
#[test]
fn raises_remains_an_ordinary_identifier_outside_assert_context() -> Result<(), String> {
    let source = vec![("identifier.hgl".into(), "module names\nconst fn raises()->bool=>true\ntest ordinary { let result=raises()\n assert result }\n".into())];
    hgl_program::compile_tests(&source).map(|_| ())
}
