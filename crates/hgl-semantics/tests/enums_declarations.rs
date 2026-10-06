//! Declaration numbers and imported identities are checked before emission.
use hgl_semantics::enums::{member, resolve, validate};
use hgl_semantics::library::{Library, load};
use hgl_source::Literal;

fn library(body: &str) -> Result<Library, String> {
    load(&[("enum.hgl".into(), format!("module example\n{body}"))])
}

#[test]
fn signed_endpoints_reset_and_automatic_numbers_preserve_declaration_order() {
    let library = library(
        "enum E {zero, one, low = -9223372036854775808, next, high = 9223372036854775807, reset = -7, after, arithmetic = 10 + 2,}",
    ).unwrap();
    validate(&library).unwrap();
    let ty = resolve(&library, "example", "E").unwrap().unwrap();
    assert_eq!(ty.origin, "example::E");
    assert_eq!(
        ty.members.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
        vec![0, 1, i64::MIN, i64::MIN + 1, i64::MAX, -7, -6, 12]
    );
    assert_eq!(
        member(&library, "example", "E::low").unwrap(),
        Some(Literal::Enum(ty, i64::MIN))
    );
}

#[test]
fn malformed_unused_declarations_are_rejected() {
    for body in [
        "enum E {a = 9223372036854775808}",
        "enum E {a = -9223372036854775809}",
        "enum E {a = 9223372036854775807, b}",
        "enum E {a = 1, b = 1}",
        "enum E {a = 1, b = 0, c}",
        "enum E {a, a = 2}",
        "enum E {a = true}",
        "enum E {a = 1.0}",
        "enum E {a = missing}",
        "enum E {a b}",
        "enum E<T> {a}",
        "enum E {a}\nenum E {b}",
        "enum E {a}\nstruct E {value:i64}",
    ] {
        assert!(validate(&library(body).unwrap()).is_err(), "{body}");
    }
}

#[test]
fn imports_keep_canonical_identity_and_exact_member_spelling() {
    let library = load(&[
        (
            "main.hgl".into(),
            "module main\nuse types as t\nuse types::{E}".into(),
        ),
        (
            "types.hgl".into(),
            "module types\nexport enum E {first = -7}\nenum Private {first = -7}".into(),
        ),
    ])
    .unwrap();
    let aliased = member(&library, "main", "E::first").unwrap();
    assert_eq!(aliased, member(&library, "main", "t::E::first").unwrap());
    assert!(matches!(aliased, Some(Literal::Enum(ref ty, -7)) if ty.origin == "types::E"));
    assert!(
        member(&library, "main", "t::E::First")
            .unwrap_err()
            .contains("unknown enum member")
    );
    assert!(
        member(&library, "main", "t::Private::first")
            .unwrap_err()
            .contains("not exported")
    );
    assert!(
        member(&library, "main", "types::E::first")
            .unwrap_err()
            .contains("imported module alias")
    );
}
