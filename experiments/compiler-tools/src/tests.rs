use crate::{check, hand, lex, model::*};
const VALID: &str = include_str!("../corpus/valid.hgl");
fn parsed(source: &str) -> Parsed {
    hand::parse(source, &lex::significant(source, &lex::handwritten(source)))
}
#[test]
fn source_and_trivia_are_lossless() {
    for source in [
        VALID.to_owned(),
        VALID.replace('\n', "\r\n"),
        "# λ comment\n/* first\nsecond */ @ λ\n/* unfinished".into(),
        "".into(),
    ] {
        let raw = lex::handwritten(&source);
        let restored: String = raw.iter().map(|t| &source[t.span.clone()]).collect();
        assert_eq!(restored, source);
        #[cfg(feature = "logos")]
        assert_eq!(lex::generated(&source), raw);
    }
}
#[test]
fn precedence_and_source_types_match_literal_expectations() {
    let p = parsed(VALID);
    assert_eq!(p.issues, vec![]);
    assert_eq!(p.declarations.len(), 5);
    let Decl::Function { name, body, .. } = &p.declarations[1] else {
        panic!("function expected")
    };
    assert_eq!(name, "affine");
    assert_eq!(
        *body,
        Expr::Add(
            Box::new(Expr::Name("x".into())),
            Box::new(Expr::Mul(
                Box::new(Expr::Name("y".into())),
                Box::new(Expr::Int(2))
            ))
        )
    );
    let checked = check::check(&p).unwrap();
    assert_eq!(checked.len(), 4);
    let Decl::Function { result, .. } = &checked[2] else {
        panic!("function expected")
    };
    let list = Ty::List(Box::new(Ty::I64), 2);
    assert_eq!(
        *result,
        Ty::Ref(Box::new(Ty::Bundle(vec![
            ("left".into(), list.clone()),
            ("right".into(), list)
        ])))
    );
}
#[test]
fn recovery_preserves_the_following_declaration() {
    let p = parsed(include_str!("../corpus/recovery.hgl"));
    assert_eq!(p.issues.len(), 2);
    assert_eq!(p.declarations.len(), 1);
    assert!(matches!(&p.declarations[0], Decl::Function { name, .. } if name == "retained"));
    assert!(check::check(&p).is_err());
}
#[test]
fn source_ref_nesting_is_rejected_but_substitution_collapses() {
    assert_eq!(
        check::check(&parsed(include_str!("../corpus/normalization.hgl"))).unwrap_err(),
        "explicit nested ref is invalid"
    );
    let generic = parsed(include_str!("../corpus/generic.hgl"));
    let result =
        check::instantiate(&generic, &[("T".into(), Ty::Ref(Box::new(Ty::I64)))].into()).unwrap();
    let Decl::Function { params, result, .. } = &result[0] else {
        panic!("function expected")
    };
    assert_eq!(*result, Ty::Ref(Box::new(Ty::I64)));
    assert_eq!(params[0].1, *result);
    let separated = "fn separated(x: ref<list<ref<i64>, 2>>) -> ref<list<ref<i64>, 2>> => x\n";
    assert!(check::check(&parsed(separated)).is_ok());
}
#[test]
fn checker_rejects_unknown_names_return_mismatch_and_cycles() {
    assert_eq!(
        check::check(&parsed(include_str!("../corpus/type_error.hgl"))).unwrap_err(),
        "unknown name missing"
    );
    assert!(
        check::check(&parsed("fn bad(x: i64) -> list<i64, 2> => x\n"))
            .unwrap_err()
            .contains("return type")
    );
    assert!(
        check::check(&parsed("struct A {\nx: A\n}\n"))
            .unwrap_err()
            .contains("recursive structure")
    );
}
#[cfg(feature = "chumsky")]
#[test]
fn parser_candidates_agree_on_valid_and_recovered_structure() {
    for source in [
        VALID,
        include_str!("../corpus/recovery.hgl"),
        include_str!("../corpus/generic.hgl"),
        include_str!("../corpus/normalization.hgl"),
        "fn bad(x: i64) -> i64 => x @\nfn ok(x: i64) -> i64 => x\n",
    ] {
        let tokens = lex::significant(source, &lex::handwritten(source));
        let a = hand::parse(source, &tokens);
        let b = crate::combinator::parse(source, &tokens);
        assert_eq!(a.declarations, b.declarations, "{source}\n{:?}", b.issues);
        assert_eq!(a.issues.len(), b.issues.len(), "{source}\n{:?}", b.issues);
        assert_eq!(
            a.issues.iter().map(|e| e.span.start).collect::<Vec<_>>(),
            b.issues.iter().map(|e| e.span.start).collect::<Vec<_>>()
        );
    }
}
#[cfg(feature = "rowan")]
#[test]
fn rowan_retains_invalid_text_as_well_as_valid_text() {
    for source in [VALID, include_str!("../corpus/recovery.hgl"), "# λ\r\n@\n"] {
        assert_eq!(
            crate::tooling::lossless(source, &lex::handwritten(source)),
            source
        );
    }
}
#[cfg(feature = "codespan-reporting")]
#[test]
fn diagnostics_render_both_errors_and_eof() {
    let source = include_str!("../corpus/recovery.hgl");
    let rendered = crate::tooling::diagnostics(source, &parsed(source).issues);
    assert_eq!(rendered.matches("error[syntax]").count(), 2);
    assert!(rendered.contains("case.hgl:2:"));
    assert!(rendered.contains("case.hgl:3:"));
    let source = "fn unfinished(";
    assert!(crate::tooling::diagnostics(source, &parsed(source).issues).contains("case.hgl:1:"));
}
#[cfg(feature = "chumsky")]
#[test]
fn larger_file_keeps_all_declarations() {
    let source: String = (0..100)
        .map(|n| format!("fn f{n}(x: i64, y: i64) -> i64 => (x + y) * 2\n"))
        .collect();
    let tokens = lex::significant(&source, &lex::handwritten(&source));
    let a = hand::parse(&source, &tokens);
    let b = crate::combinator::parse(&source, &tokens);
    assert!(a.issues.is_empty() && b.issues.is_empty());
    assert_eq!(a.declarations.len(), 100);
    assert_eq!(a.declarations, b.declarations);
}
