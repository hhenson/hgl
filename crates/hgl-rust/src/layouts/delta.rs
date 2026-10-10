use hgl_source::Ty;
/// Private typed ordinary storage for sparse data of an exact structural origin.
pub fn delta_storage(origin: &Ty) -> Ty {
    let list = |ty| Ty::List(Box::new(ty), None);
    let fields = if let Ty::Set(key) = origin {
        vec![
            ("added".into(), list((**key).clone())),
            ("removed".into(), list((**key).clone())),
        ]
    } else if let Ty::List(child, size) = origin {
        let mut fields = vec![
            ("keys".into(), list(Ty::I64)),
            ("values".into(), list(delta_type(child))),
        ];
        if size.is_none() {
            fields.push(("removed".into(), list(Ty::I64)));
        }
        fields
    } else if let Ty::Map(key, child) = origin {
        vec![
            ("keys".into(), list((**key).clone())),
            ("values".into(), list(delta_type(child))),
            ("removed".into(), list((**key).clone())),
        ]
    } else if let Ty::Tuple(children) = origin {
        children
            .iter()
            .enumerate()
            .map(|(i, child)| (i.to_string(), list(delta_type(child))))
            .collect()
    } else if let Ty::Struct(_, fields, _) = origin {
        fields
            .iter()
            .map(|(name, child)| (name.clone(), list(delta_type(child))))
            .collect()
    } else {
        unreachable!("checked structural delta origin")
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
