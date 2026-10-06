//! Declaration-fixed family membership, scoped inheritance and invariant identity.
use hgl_source::Ty;
fn resolve(sources: &[(&str, &str)], module: &str, name: &str) -> Result<Ty, String> {
    let library = hgl_semantics::library::load(
        &sources
            .iter()
            .map(|(name, text)| ((*name).into(), (*text).into()))
            .collect::<Vec<_>>(),
    )?;
    hgl_semantics::value_types::resolve_ordinary(&library, module, name)
}
#[test]
fn membership_includes_unobserved_declared_members_and_exact_ancestors() -> Result<(), String> {
    let sources = [(
        "family",
        "module family\nabstract struct Event {label:str}\nstruct First:Event {value:i64}\nstruct Second:Event {value:i64}\nabstract struct Sub:Event {}\nstruct Child:Sub {values:list<i64>}",
    )];
    let Ty::Family(root) = resolve(&sources, "family", "Event")? else {
        return Err("family required".into());
    };
    assert_eq!(root.members().len(), 3);
    assert_ne!(root.members()[1].0, root.members()[2].0);
    let Ty::Family(child) = resolve(&sources, "family", "Sub")? else {
        return Err("family required".into());
    };
    assert_eq!(child.members().len(), 1);
    assert_eq!(child.ancestors(), &[root.identity().clone()]);
    assert!(!Ty::Family(root.clone()).publication());
    assert!(Ty::Family(root).atomic().publication());
    Ok(())
}
#[test]
fn imported_generic_parent_fields_keep_original_scopes() -> Result<(), String> {
    let sources = [
        (
            "parent",
            "module parent\nexport struct ParentField {value:i64}\nexport abstract struct Event<T> {payload:T\nowned:ParentField}",
        ),
        (
            "child",
            "module child\nuse parent as p\nstruct ParentField {other:str}\nstruct Child<U>:p::Event<list<U>> {extra:U}",
        ),
    ];
    let child = resolve(&sources, "child", "Child<str>")?;
    let (_, fields, _) = child.structure()?;
    assert_eq!(fields[0].1, Ty::List(Box::new(Ty::Str), None));
    assert_eq!(fields[1].1.structure()?.0.origin, "parent::ParentField");
    let family = resolve(&sources, "child", "p::Event<list<str>>")?;
    let Ty::Family(family) = family else {
        return Err("family required".into());
    };
    assert_eq!(family.members().len(), 1);
    assert_eq!(family.members()[0].0.source_name(), "child::Child<str>");
    Ok(())
}
#[test]
fn invariant_specializations_and_invalid_declarations_are_distinct() -> Result<(), String> {
    let source = "module family\nabstract struct Event<T> {value:T}\nstruct Child<T>:Event<T> {}";
    let string = resolve(&[("family", source)], "family", "Event<str>")?;
    let integer = resolve(&[("family", source)], "family", "Event<i64>")?;
    assert_ne!(string, integer);
    for source in [
        "module family\nabstract struct Event {}\nstruct Child<T>:Event {value:T}",
        "module family\nabstract struct Event:Other {}\nabstract struct Other:Event {}",
        "module family\nabstract struct Event {}\nabstract struct Other {}\nstruct Child:Event,Other {}",
        "module family\nabstract struct Event {value:i64}\nstruct Child:Event {value:str}",
        "module family\nabstract struct Event {}\nstruct Child:Event {next:Event}",
    ] {
        assert!(
            resolve(&[("family", source)], "family", "Event").is_err(),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn ancestor_arguments_preserve_complete_nested_nominals_and_fixed_lists() -> Result<(), String> {
    let source = "module family\nstruct Box<T> {value:T}\nabstract struct Event<T> {value:T}\nabstract struct Sub<T>:Event<Box<T>> {}\nstruct Child<T>:Sub<T> {}\nstruct Fixed<T>:Event<list<T,2>> {}";
    let sources = [("family", source)];
    let Ty::Family(parent) = resolve(&sources, "family", "Event<Box<str>>")? else {
        return Err("family required".into());
    };
    let Ty::Family(child) = resolve(&sources, "family", "Sub<str>")? else {
        return Err("family required".into());
    };
    assert_eq!(child.ancestors(), &[parent.identity().clone()]);
    assert_eq!(parent.members().len(), 1);
    let Ty::Family(fixed) = resolve(&sources, "family", "Event<list<i64,2>>")? else {
        return Err("family required".into());
    };
    assert_eq!(fixed.members().len(), 1);
    assert_eq!(fixed.members()[0].0.source_name(), "family::Fixed<i64>");
    Ok(())
}
