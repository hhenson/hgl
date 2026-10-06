//! Exact checked type metadata for cold Rust source serialization.
use crate::enums::metadata as enum_data;
use hgl_source::Ty;
fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
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
    let inner = match t {
        Ty::Family(f) => format!(
            "Family(hgl_source::FamilyType::new({},{},{}).unwrap_or_else(|_|unreachable!(\"checked family\")))",
            nominal(f.identity()),
            list(f.ancestors(), nominal),
            list(f.members(), |(id, t)| format!(
                "({},{})",
                nominal(id),
                ty(t)
            ))
        ),
        Ty::Recursive(batch) => format!("Recursive({})", recursive(batch)),
        Ty::Enum(e) => format!("Enum({})", enum_data(e)),
        Ty::Rolling(t, window) => format!(
            "Rolling(Box::new({}),hgl_source::Window::new(hgl_source::WindowKind::{:?},{},{}).expect(\"checked window\"))",
            ty(t),
            window.kind(),
            window.maximum(),
            window.minimum()
        ),
        Ty::Atomic(t) => format!("Atomic(Box::new({}))", ty(t)),
        Ty::Ref(t) => format!("Ref(Box::new({}))", ty(t)),
        Ty::Nullable(t) => format!("Nullable(Box::new({}))", ty(t)),
        Ty::Set(t) => format!("Set(Box::new({}))", ty(t)),
        Ty::Delta(t) => format!("Delta(Box::new({}))", ty(t)),
        Ty::Map(k, v) => format!("Map(Box::new({}),Box::new({}))", ty(k), ty(v)),
        Ty::List(t, n) => format!("List(Box::new({}),{n:?})", ty(t)),
        Ty::Tuple(ts) => format!("Tuple({})", list(ts, ty)),
        Ty::Struct(n, fields, optional) => format!(
            "Struct({},{},vec!{optional:?})",
            nominal(n),
            list(fields, |(name, t)| format!("({name:?}.into(),{})", ty(t)))
        ),
        Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Duration
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => format!("{t:?}"),
    };
    format!("hgl_source::Ty::{inner}")
}
