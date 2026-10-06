//! Static owning family layouts and concrete-tag-preserving conversion emission.
use hgl_source::{FamilyType, Ty};
/// Fixed typed discriminator and member slots for a closed nominal family.
pub fn family_storage(family: &FamilyType) -> Ty {
    let mut fields = vec![("tag".into(), Ty::I64)];
    fields.extend(
        family
            .members()
            .iter()
            .map(|(id, ty)| (id.source_name(), ty.clone())),
    );
    Ty::Struct(
        format!("\0family<{}>", family.identity().source_name()).into(),
        fields,
        (1..=family.members().len()).collect(),
    )
}
/// Construct the fixed representation for one checked exact member.
pub fn family_member(family: &FamilyType, index: usize, expression: &str) -> String {
    let fields = (0..family.members().len())
        .map(|i| {
            if i == index {
                format!("Some({expression}),")
            } else {
                "None,".into()
            }
        })
        .collect::<Vec<_>>()
        .concat();
    format!("({index}i64,{fields})")
}
/// Widen an owning concrete value or declared subfamily without changing its tag.
pub fn family_coerce(target: &Ty, source: &Ty, expression: &str) -> String {
    let Ty::Family(target) = target else {
        unreachable!("checked family conversion")
    };
    if let Ty::Family(source) = source {
        let arms = source
            .members()
            .iter()
            .enumerate()
            .map(|(i, (id, _))| {
                let target_index = target
                    .members()
                    .iter()
                    .position(|(member, _)| member == id)
                    .unwrap_or_else(|| unreachable!("checked ancestor membership"));
                format!(
                    "{i}=>{},",
                    family_member(
                        target,
                        target_index,
                        &format!(
                            "value.{}.unwrap_or_else(||unreachable!(\"active family member\"))",
                            i + 1
                        )
                    )
                )
            })
            .collect::<Vec<_>>()
            .concat();
        format!(
            "{{let value={expression};match value.0 {{{arms}_=>unreachable!(\"checked family tag\")}}}}"
        )
    } else {
        let index = target
            .members()
            .iter()
            .position(|(_, member)| member == source)
            .unwrap_or_else(|| unreachable!("checked concrete family member"));
        family_member(target, index, expression)
    }
}

/// Emit cold checked family-to-native conversion.
pub fn decode(family: &FamilyType, expression: &str, decode: fn(&Ty, &str) -> String) -> String {
    let arms = family
        .members()
        .iter()
        .enumerate()
        .map(|(index, (_, member))| {
            let decoded = format!("{{if member.ty!={} {{return Err(\"prepared concrete family member mismatch\".into())}} {}}}",crate::type_data::ty(member),decode(member,"member"));
            format!("{index}=>{},", family_member(family, index, &decoded))
        })
        .collect::<Vec<_>>()
        .concat();
    let metadata = crate::type_data::ty(&Ty::Family(family.clone()));
    format!(
        "{{let v=&({expression});if v.ty!={metadata} {{return Err(\"prepared family mismatch\".into())}} let hgl_semantics::ir::Kind::Construct(fields)=&v.kind else {{return Err(\"prepared family member required\".into())}};if fields.len()!=1 {{return Err(\"family requires exactly one member\".into())}}let (tag,member)=&fields[0];match tag {{{arms}_=>return Err(\"family member outside declared closure\".into())}}}}"
    )
}
/// Emit cold native-to-checked family capture conversion.
pub fn encode(family: &FamilyType, expression: &str, encode: fn(&Ty, &str) -> String) -> String {
    let arms = family
        .members()
        .iter()
        .enumerate()
        .map(|(index, (_, member))| {
            let encoded = encode(
                member,
                &format!(
                    "v.{}.as_ref().unwrap_or_else(||unreachable!(\"active family member\"))",
                    index + 1
                ),
            );
            format!("{index}=>({index},{encoded}),")
        })
        .collect::<Vec<_>>()
        .concat();
    let metadata = crate::type_data::ty(&Ty::Family(family.clone()));
    format!(
        "{{let v=&({expression});let member=match v.0 {{{arms}_=>unreachable!(\"checked family tag\")}};hgl_semantics::ir::Value::new({metadata},hgl_semantics::ir::Kind::Construct(vec![member]))}}"
    )
}

mod constants;
pub use constants::prepare;
