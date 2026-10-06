//! Cold family widening must retain exact tags and explicit ancestor identity.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{FamilyType, Literal, Nominal, Ty};
fn member(name: &str) -> Ty {
    Ty::Struct(name.into(), vec![("value".into(), Ty::I64)], Vec::new())
}
fn value(ty: Ty) -> Value {
    Value::new(
        ty,
        Kind::Construct(vec![(
            0,
            Value::new(Ty::I64, Kind::Literal(Literal::Int(1))),
        )]),
    )
}
fn family(id: &str, ancestors: Vec<Nominal>, members: &[Ty]) -> Result<Ty, String> {
    Ok(Ty::Family(FamilyType::new(
        id.into(),
        ancestors,
        members
            .iter()
            .map(|ty| Ok((ty.structure()?.0.clone(), ty.clone())))
            .collect::<Result<_, String>>()?,
    )?))
}
#[test]
fn equal_layout_members_are_distinct_and_subfamily_widening_remaps_tag() -> Result<(), String> {
    let (first, second) = (member("test::First"), member("test::Second"));
    let root = family("test::Event", Vec::new(), &[first.clone(), second.clone()])?;
    let sub = family(
        "test::Sub",
        vec!["test::Event".into()],
        std::slice::from_ref(&second),
    )?;
    let narrow = hgl_family_values::retain(&sub, value(second.clone()))?;
    assert!(hgl_family_values::coerce(Some(&root), narrow.clone()).is_ok());
    let ancestor = hgl_family_values::retain(&root, narrow)?;
    assert_eq!(hgl_family_values::concrete(&ancestor).ty, second);
    assert!(matches!(&ancestor.kind,Kind::Construct(fields) if fields[0].0==1));
    let a = hgl_family_values::retain(&root, value(first))?;
    assert!(!hgl_family_values::equal(&a, &ancestor));
    assert!(hgl_family_values::equal(&ancestor, &ancestor));
    Ok(())
}
#[test]
fn coincident_members_do_not_create_an_ancestor_relation() -> Result<(), String> {
    let concrete = member("test::Only");
    let left = family("test::Left", Vec::new(), std::slice::from_ref(&concrete))?;
    let right = family("test::Right", Vec::new(), std::slice::from_ref(&concrete))?;
    let value = hgl_family_values::retain(&left, value(concrete))?;
    assert!(hgl_family_values::coerce(Some(&right), value).is_err());
    Ok(())
}

#[test]
fn malformed_closed_family_tag_is_rejected_before_widening() -> Result<(), String> {
    let first = member("test::First");
    let second = member("test::Second");
    let root = family("test::Event", Vec::new(), &[first.clone(), second.clone()])?;
    let malformed = Value::new(root.clone(), Kind::Construct(vec![(0, value(second))]));
    assert!(hgl_family_values::retain(&root, malformed).is_err());
    let missing = Value::new(root.clone(), Kind::Construct(vec![(3, value(first))]));
    assert!(hgl_family_values::retain(&root, missing).is_err());
    Ok(())
}
