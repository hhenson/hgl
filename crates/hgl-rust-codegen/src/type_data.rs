//! Exact checked type metadata for cold Rust source serialization.
use crate::enums::metadata as enum_data;
use hgl_source::Ty;
pub(crate) fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    format!(
        "vec![{}]",
        items.iter().map(f).collect::<Vec<_>>().join(",")
    )
}
fn nominal(n: &hgl_source::Nominal) -> String {
    format!(
        "hgl_source::Nominal {{origin:{:?}.into(),arguments:{}}}",
        n.origin,
        list(&n.arguments, ty)
    )
}
fn recursive(batch: &hgl_source::RecursiveType) -> String {
    if batch.definitions().is_empty() {
        return format!(
            "hgl_source::RecursiveType::reference({})",
            nominal(batch.identity())
        );
    }
    let definitions = list(batch.definitions(), |d| {
        format!(
            "hgl_source::NominalDefinition::new({},{},vec!{:?})",
            nominal(d.identity()),
            list(d.fields(), |(name, t)| format!(
                "({name:?}.into(),{})",
                ty(t)
            )),
            d.optional()
        )
    });
    format!(
        "hgl_source::RecursiveType::new({},{definitions}).unwrap_or_else(|_|unreachable!(\"checked nominal batch\"))",
        nominal(batch.identity())
    )
}
/// Emit exact checked source type metadata.
pub fn ty(t: &Ty) -> String {
    let inner = if let Ty::Family(f) = t {
        format!(
            "Family(hgl_source::FamilyType::new({},{},{}).unwrap_or_else(|_|unreachable!(\"checked family\")))",
            nominal(f.identity()),
            list(f.ancestors(), nominal),
            list(f.members(), |(id, t)| format!(
                "({},{})",
                nominal(id),
                ty(t)
            ))
        )
    } else if let Ty::Recursive(batch) = t {
        format!("Recursive({})", recursive(batch))
    } else if let Ty::Enum(e) = t {
        format!("Enum({})", enum_data(e))
    } else if let Ty::Rolling(t, window) = t {
        format!(
            "Rolling(Box::new({}),hgl_source::Window::new(hgl_source::WindowKind::{:?},{},{}).expect(\"checked window\"))",
            ty(t),
            window.kind(),
            window.maximum(),
            window.minimum()
        )
    } else if let Ty::Atomic(t) = t {
        format!("Atomic(Box::new({}))", ty(t))
    } else if let Ty::Ref(t) = t {
        format!("Ref(Box::new({}))", ty(t))
    } else if let Ty::Nullable(t) = t {
        format!("Nullable(Box::new({}))", ty(t))
    } else if let Ty::Set(t) = t {
        format!("Set(Box::new({}))", ty(t))
    } else if let Ty::Delta(t) = t {
        format!("Delta(Box::new({}))", ty(t))
    } else if let Ty::Map(k, v) = t {
        format!("Map(Box::new({}),Box::new({}))", ty(k), ty(v))
    } else if let Ty::List(t, n) = t {
        format!("List(Box::new({}),{n:?})", ty(t))
    } else if let Ty::Tuple(ts) = t {
        format!("Tuple({})", list(ts, ty))
    } else if let Ty::Struct(n, fields, optional) = t {
        format!(
            "Struct({},{},vec!{optional:?})",
            nominal(n),
            list(fields, |(name, t)| format!("({name:?}.into(),{})", ty(t)))
        )
    } else {
        format!("{t:?}")
    };
    format!("hgl_source::Ty::{inner}")
}
