//! Canonical finite recursive specializations and rejected recursive edge forms.
use hgl_source::{RecursiveType, Ty};
fn resolve(source: &str, name: &str) -> Result<Ty, String> {
    let library = hgl_semantics::library::load(&[(
        "recursive.hgl".into(),
        format!("module recursive\n{source}"),
    )])?;
    hgl_semantics::value_types::resolve_ordinary(&library, "recursive", name)
}
fn batch(ty: &Ty) -> Result<&RecursiveType, String> {
    if let Ty::Recursive(batch) = ty {
        Ok(batch)
    } else {
        Err("expected recursive batch".into())
    }
}
#[test]
fn mutual_roots_and_edges_keep_consistent_identity() -> Result<(), String> {
    let source="struct Left {value:i64\nother:atomic<Right>=null}\nstruct Right {value:i64\nother:atomic<Left>=null}".replace(">=","> =");
    let left = resolve(&source, "Left")?;
    let right = resolve(&source, "Right")?;
    let left_batch = batch(&left)?;
    let right_batch = batch(&right)?;
    assert_eq!(left_batch.definitions().len(), 2);
    assert_eq!(right_batch.definitions().len(), 2);
    let edge = &left_batch.definition(left_batch.identity())?.fields()[1].1;
    assert_eq!(edge, &right);
    assert!(batch(edge)?.definition(batch(edge)?.identity()).is_err());
    assert_eq!(
        left_batch.definition(batch(edge)?.identity())?.identity(),
        right_batch.identity()
    );
    assert!(!left.publication());
    assert!(left.clone().atomic().publication());
    Ok(())
}
#[test]
fn finite_generic_permutation_and_closed_edges_terminate() -> Result<(), String> {
    let source = "struct Swap<A,B> {value:A\nnext:atomic<Swap<B,A>> = null}";
    let ty = resolve(source, "Swap<str,i64>")?;
    let batch = batch(&ty)?;
    assert_eq!(batch.definitions().len(), 2);
    assert_eq!(
        batch.definitions()[0].identity().source_name(),
        "recursive::Swap<i64,str>"
    );
    assert_eq!(
        batch.definitions()[1].identity().source_name(),
        "recursive::Swap<str,i64>"
    );
    let closed = resolve(
        "struct Link<T> {value:T\nnext:atomic<Link<i64>> = null}",
        "Link<str>",
    )?;
    assert_eq!(self::batch(&closed)?.definitions().len(), 2);
    Ok(())
}
#[test]
fn malformed_recursive_edges_are_rejected_before_expansion() {
    for field in [
        "next:Node",
        "next:atomic<Node>",
        "next:Node = null",
        "next:atomic<list<Node>> = null",
        "next:list<atomic<Node>> = null",
        "next:atomic<Box<Node>> = null",
    ] {
        let source = format!("struct Box<T> {{value:T}}\nstruct Node {{value:i64\n{field}}}");
        assert!(resolve(&source, "Node").is_err(), "{field}");
    }
    assert!(
        resolve(
            "struct Link<T> {value:T\nnext:atomic<Link<list<T>>> = null}",
            "Link<i64>"
        )
        .is_err()
    );
    assert!(
        resolve(
            "struct Node {value:i64=false\nnext:atomic<Node> = null}",
            "Node"
        )
        .is_err()
    );
}
#[test]
fn declaration_owned_import_aliases_and_generic_obligations_are_exact() -> Result<(), String> {
    let library = hgl_semantics::library::load(&[
        (
            "types.hgl".into(),
            "module shapes\nexport struct Link<T> {value:T\nnext:atomic<Link<T>> = null}".into(),
        ),
        (
            "caller.hgl".into(),
            "module caller\nuse shapes as first\nuse shapes as second".into(),
        ),
    ])?;
    let first =
        hgl_semantics::value_types::resolve_ordinary(&library, "caller", "first::Link<i64>")?;
    let second =
        hgl_semantics::value_types::resolve_ordinary(&library, "caller", "second::Link<i64>")?;
    assert_eq!(first, second);
    assert_ne!(
        first,
        hgl_semantics::value_types::resolve_ordinary(&library, "caller", "first::Link<str>")?
    );
    let source = "struct Value {n:i64}\nstruct Phantom<T> {next:atomic<Phantom<T>> = null}";
    assert!(resolve(source, "Phantom<atomic<Value>>").is_err());
    let source = "struct Value {n:i64}\nstruct Swap<A,B> {value:A\nnext:atomic<Swap<B,A>> = null}";
    assert!(resolve(source, "Swap<i64,atomic<Value>>").is_err());
    Ok(())
}
#[test]
fn cycles_cannot_cross_module_boundaries() -> Result<(), String> {
    let library=hgl_semantics::library::load(&[
        ("left.hgl".into(),"module left\nuse right as other\nexport struct Left {next:atomic<other::Right> = null}".into()),
        ("right.hgl".into(),"module right\nuse left as other\nexport struct Right {next:atomic<other::Left> = null}".into()),
    ])?;
    let error = hgl_semantics::value_types::resolve_ordinary(&library, "left", "Left")
        .err()
        .ok_or("cross-module cycle accepted")?;
    assert!(error.contains("one module"), "{error}");
    Ok(())
}
