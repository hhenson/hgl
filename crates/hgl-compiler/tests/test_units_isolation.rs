//! Declaration isolation and original source positions.
use hgl_compiler::test_units::{mask, units};
#[test]
fn nested_contexts_own_inner_declarations_and_preserve_positions() {
    let text = "module sample\ntest {\n fn helper(value i64)->i64=>value\n test marked { assert false }\n}\ntest survivor { assert true }\n";
    let owners = units(text).unwrap();
    assert_eq!(owners.len(), 3);
    assert_eq!(owners[0].line, 3);
    assert_eq!(owners[1].test_name().as_deref(), Some("sample::marked"));
    let retained = mask(text, [owners[0].span.clone(), owners[1].span.clone()]);
    assert_eq!(retained.len(), text.len());
    assert_eq!(retained.find("test survivor"), text.find("test survivor"));
    assert_eq!(retained.lines().count(), text.lines().count());
    assert!(retained.contains("test {"));
}
#[test]
fn recoverable_sole_final_declaration_can_end_at_eof() {
    let text = "module sample\nfn invalid(value:i64 ->i64=>value\n";
    let owners = units(text).unwrap();
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].span.end, text.len());
    assert!(units(&(text.to_owned() + "test neighbour {assert true}\n")).is_err());
}
#[test]
fn unclosed_nested_bodies_cannot_absorb_a_neighbour() {
    assert!(units("module sample\nfn bad(){ if true {\ntest neighbour {assert true}\n").is_err());
}
#[test]
fn malformed_name_can_have_a_reliable_body_boundary() {
    let owners = units(
        "module sample\nfn (value:i64)->i64 {when{return value}}\ntest neighbour {assert true}\n",
    )
    .unwrap();
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[0].name, None);
    assert_eq!(owners[1].name.as_deref(), Some("neighbour"));
}
#[test]
fn masking_unicode_preserves_original_byte_offsets() {
    let text = "module sample\nconst fn text()->str=>\"雪\"\ntest valid {assert true}\n";
    let owners = units(text).unwrap();
    let retained = mask(text, [owners[0].span.clone()]);
    assert_eq!(retained.len(), text.len());
    assert_eq!(retained.find("test valid"), text.find("test valid"));
}

#[test]
fn type_domain_braces_are_not_declaration_bodies() {
    let text = "module m\nfn f<T>(x:T)->T requires T in {i64,f64} {return x}\nstruct S<T> requires T in {i64,f64} {value:T}\ntest live {assert true}\n";
    let found = units(text).unwrap();
    assert_eq!(found.len(), 3);
    assert_eq!(
        found.iter().map(|u| u.name.as_deref()).collect::<Vec<_>>(),
        vec![Some("f"), Some("S"), Some("live")]
    );
}
