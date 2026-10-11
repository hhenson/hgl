//! Source-table sharing preserves independent specialization and mutation state.
use hgl_semantics::{library, value_types};
use hgl_source::{Literal, Ty};
use std::{collections::BTreeMap, sync::Arc};
#[test]
fn declaration_clones_share_until_mutated_and_keep_original_provenance() {
    let original = library::load(&[(
        "original.hgl".into(),
        "module sample\nconst fn choose(value:i64)->i64=>value".into(),
    )])
    .unwrap();
    let mut changed = original.clone();
    assert!(Arc::ptr_eq(&original.declarations, &changed.declarations));
    let original_signature = original.declarations[0].signature_checked().unwrap();
    let declaration = &mut Arc::make_mut(&mut changed.declarations)[0];
    declaration.name = "other".into();
    declaration.source = "other.hgl".into();
    declaration.tokens.clear();
    assert!(!Arc::ptr_eq(&original.declarations, &changed.declarations));
    assert_eq!(original.declarations[0].name, "choose");
    assert_eq!(original.declarations[0].source, "original.hgl");
    let retained = original.declarations[0].signature_checked().unwrap();
    let origins = |signature: &library::Signature| {
        signature.parameters[0]
            .type_tokens
            .iter()
            .map(|token| (token.text.clone(), token.span.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(origins(&retained), origins(&original_signature));
    assert!(changed.declarations[0].signature_checked().is_err());
}
#[test]
fn specialization_caches_and_module_bindings_remain_independent() {
    let original =
        library::load(&[("original.hgl".into(), "module sample\nfn main() {}".into())]).unwrap();
    let mut first = original.clone();
    let mut second = original.clone();
    for (candidate, width) in [(&mut first, 2), (&mut second, 4)] {
        candidate
            .type_sizes
            .insert(("sample".into(), "width()".into()), Literal::Int(width));
        let ty = value_types::concrete(candidate, "sample", "list<i64,width()>", &BTreeMap::new())
            .unwrap();
        assert_eq!(
            ty,
            Ty::List(Box::new(Ty::I64), Some(usize::try_from(width).unwrap()))
        );
    }
    first
        .imports
        .insert(("sample".into(), "alias".into()), "first::Name".into());
    assert!(second.imports.is_empty());
    assert!(original.type_sizes.is_empty());
    assert_eq!(
        first.type_sizes[&("sample".into(), "width()".into())],
        Literal::Int(2)
    );
    assert!(Arc::ptr_eq(&first.declarations, &second.declarations));
}
