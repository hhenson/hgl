use hgl_source::Ty;
/// Private typed ordinary storage for sparse data of an exact structural origin.
pub fn delta_storage(origin: &Ty) -> Ty {
    let list = |ty| Ty::List(Box::new(ty), None);
    let fields = match origin {
        Ty::Set(key) => vec![
            ("added".into(), list((**key).clone())),
            ("removed".into(), list((**key).clone())),
        ],
        Ty::List(child, Some(_)) => vec![
            ("keys".into(), list(Ty::I64)),
            ("values".into(), list(delta_type(child))),
        ],
        Ty::Map(key, child) => vec![
            ("keys".into(), list((**key).clone())),
            ("values".into(), list(delta_type(child))),
            ("removed".into(), list((**key).clone())),
        ],
        Ty::Tuple(children) => children
            .iter()
            .enumerate()
            .map(|(i, child)| (i.to_string(), list(delta_type(child))))
            .collect(),
        Ty::Struct(_, fields, _) => fields
            .iter()
            .map(|(name, child)| (name.clone(), list(delta_type(child))))
            .collect(),
        Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::List(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Ref(_)
        | Ty::Nullable(_)
        | Ty::Recursive(_)
        | Ty::Family(_)
        | Ty::Void => unreachable!("checked structural delta origin"),
    };
    // A NUL cannot occur in an HGL declaration name; synthetic identities cannot alias user structs.
    Ty::Struct(
        format!("\0delta<{}>", origin.source_name()).into(),
        fields,
        Vec::new(),
    )
}

/// Ordinary publication payload of an admitted temporal shape.
pub fn delta_type(ty: &Ty) -> Ty {
    ty.clone()
        .delta()
        .unwrap_or_else(|_| unreachable!("checked delta origin"))
}
