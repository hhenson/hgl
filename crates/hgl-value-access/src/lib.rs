//! Ordinary owning, lexical global and evaluation-local observation authority.
use hgl_rust_ir::{Kind, Value};
use hgl_source::Ty;
/// Whether the checked type admits ordinary owning retention.
pub fn ordinary(ty: &Ty) -> bool {
    if let Ty::Struct(_, fields) = ty {
        return fields.iter().all(|(_, ty)| ordinary(&project(ty)));
    }
    if let Ty::Tuple(children) = ty {
        return children.iter().all(ordinary);
    }
    if let Ty::List(element, _) = ty {
        return ordinary(element);
    }
    matches!(
        ty,
        Ty::Bool
            | Ty::I64
            | Ty::F64
            | Ty::Str
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Duration
            | Ty::Struct(..)
            | Ty::Delta(_)
    )
}
/// Whether a checked place carries recursive write authority.
pub fn writable(value: &Value) -> bool {
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return writable(parent);
    }
    matches!(
        value.kind,
        Kind::MutableLocal(_) | Kind::BorrowedLocal(_, _, true)
    )
}
/// Resolve a declared ordinary field without changing its parent's authority.
pub fn field(parent: Value, name: &str) -> Result<Value, String> {
    let Ty::Struct(_, fields) = &parent.ty else {
        return Err("field access requires an ordinary struct or direct injected clock".into());
    };
    if matches!(parent.kind, Kind::Input(..) | Kind::Wire(_) | Kind::Output) {
        return Err(
            "temporal child projection is outside the admitted publication-delta profile".into(),
        );
    }
    let (index, (_, ty)) = fields
        .iter()
        .enumerate()
        .find(|(_, (field, _))| field == name)
        .ok_or_else(|| format!("unknown struct field {name}"))?;
    Ok(Value::new(ty.clone(), Kind::Field(Box::new(parent), index)))
}
/// Return the entry and access mode carried by an aggregate view.
pub fn provenance(value: &Value) -> Option<(usize, bool)> {
    if !matches!(
        value.ty,
        Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Delta(_)
    ) {
        return None;
    }
    if let Kind::BorrowedLocal(_, entry, writable) = value.kind {
        return Some((entry, writable));
    }
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return provenance(parent);
    }
    None
}
/// Bind an owning value or preserve an explicitly borrowed initializer.
pub fn binding(id: usize, value: &Value, mutable: bool, annotated: bool) -> Result<Value, String> {
    let kind = if observed(value) {
        if mutable {
            return Err("structural delta observation cannot initialize writable access".into());
        }
        Kind::ObservedLocal(id)
    } else if let Kind::GlobalGet(entry) = value.kind
        && matches!(
            value.ty,
            Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Delta(_)
        )
    {
        if !annotated {
            return Err("aggregate get requires a typed let or var binding".into());
        }
        Kind::BorrowedLocal(id, entry, mutable)
    } else if let Some((entry, source_mutable)) = provenance(value) {
        if source_mutable {
            return Err("cannot alias an exclusive writable global borrow".into());
        }
        if mutable {
            return Err("cannot upgrade a read-only global borrow to writable access".into());
        }
        Kind::BorrowedLocal(id, entry, false)
    } else if mutable {
        Kind::MutableLocal(id)
    } else {
        Kind::Local(id)
    };
    Ok(Value::new(value.ty.clone(), kind))
}
/// Reject passing a borrowed aggregate through an ordinary helper boundary.
pub fn helper_argument(value: &Value) -> Result<(), String> {
    if observed(value) {
        return Err(
            "structural delta observation cannot escape through an ordinary helper call".into(),
        );
    }
    if provenance(value).is_some() {
        return Err(
            "borrowed global aggregate cannot escape through an ordinary helper call".into(),
        );
    }
    Ok(())
}
/// Whether a structural delta is an evaluation-local input observation.
pub fn observed(value: &Value) -> bool {
    if !matches!(
        value.ty,
        Ty::Delta(_) | Ty::Struct(..) | Ty::List(..) | Ty::Tuple(_)
    ) {
        return false;
    }
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return observed(parent);
    }
    matches!(value.kind, Kind::ObservedLocal(_))
        || matches!(&value.kind, Kind::Query(name,args) if name == "delta_value" && (matches!(value.ty, Ty::Delta(_)) || args.first().is_some_and(|value| matches!(value.ty, Ty::Atomic(_)))))
}

/// Ordinary scalar projection; nominal arguments and delta origins remain exact.
pub fn project(ty: &Ty) -> Ty {
    if let Ty::Atomic(payload) = ty {
        return project(payload);
    }
    if let Ty::Struct(identity, fields) = ty {
        return Ty::Struct(
            identity.clone(),
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), project(ty)))
                .collect(),
        );
    }
    if let Ty::List(child, size) = ty {
        return Ty::List(Box::new(project(child)), *size);
    }
    if let Ty::Tuple(children) = ty {
        return Ty::Tuple(children.iter().map(project).collect());
    }
    if let Ty::Map(key, child) = ty {
        return Ty::Map(Box::new(project(key)), Box::new(project(child)));
    }
    if let Ty::Set(child) = ty {
        return Ty::Set(Box::new(project(child)));
    }
    ty.clone()
}

/// Compiler-only provenance for values known before graph topology selection.
#[derive(Debug, Default)]
pub struct StaticValues {
    /// Original checked expressions behind cold eval argument bindings.
    pub prepared: Vec<Value>,
    /// Current node's readonly ordinary configurations.
    pub configuration: Vec<Value>,
    /// Static ordinary constant parameter metadata in the current lexical scope.
    pub locals: std::collections::BTreeMap<usize, Value>,
}
impl StaticValues {
    /// Follow known static origins without evaluating their expressions.
    pub fn resolve<'a>(&'a self, value: &'a Value) -> &'a Value {
        if let Kind::Prepared(id) = value.kind {
            return self.resolve(&self.prepared[id]);
        }
        if let Kind::Configuration(id) = value.kind {
            return self.resolve(&self.configuration[id]);
        }
        if let Kind::Local(id) = value.kind {
            return self.locals.get(&id).map_or(value, |v| self.resolve(v));
        }
        value
    }
    /// Map checked constant parameter origins into the source-order local slots.
    pub fn arguments(
        &self,
        signature: &hgl_library::Signature,
        args: &[Value],
        positions: &[usize],
    ) -> std::collections::BTreeMap<usize, Value> {
        signature
            .parameters
            .iter()
            .zip(positions)
            .filter(|(p, _)| p.constant)
            .filter_map(|(_, id)| {
                let value = self.resolve(&args[*id]);
                (!matches!(value.kind, Kind::Local(_) | Kind::MutableLocal(_)))
                    .then(|| (*id, value.clone()))
            })
            .collect()
    }
}
