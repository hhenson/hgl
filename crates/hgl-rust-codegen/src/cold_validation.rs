//! Cold descriptions validate their complete typed encoding before trusted codecs run.
use crate::any_values::name;
use hgl_source::Ty;

pub(crate) fn codec(ty: &Ty) -> String {
    if let Ty::Native(scalar) = ty {
        let marker = crate::native_values::marker(scalar);
        return format!(
            "<{marker} as hgl_store::native_values::NativeType>::preflight(&data,data.len())?;"
        );
    }
    if *ty == Ty::Any {
        return "let value=data.checked_boxed()?;if let Some(plan)=value.plan {let validate=plan.validate.ok_or_else(||hgl_types::NodeError::new(\"missing cold descriptor validation mapping\"))?;validate(value.data)?;}".into();
    }
    if let Ty::Delta(origin) = ty {
        return format!(
            "any_validate_{}(data)?;",
            name(&crate::layouts::delta_storage(origin))
        );
    }
    if let Ty::Family(family) = ty {
        let arms = family
            .members()
            .iter()
            .enumerate()
            .map(|(i, (_, ty))| {
                format!(
                    "{i}=>any_validate_{}(data.slice(8..data.len()))?,",
                    name(ty)
                )
            })
            .collect::<Vec<_>>()
            .concat();
        return format!(
            "match cold_number(data.get(..8).ok_or_else(cold_error)?)? {{{arms}_=>return Err(cold_error())}};"
        );
    }
    if let Some(child) = crate::collections::element(ty) {
        return format!(
            "let mut rest=data;while !rest.is_empty() {{any_validate_{}(cold_frame(&mut rest)?)?;}}",
            name(&child)
        );
    }
    if let Some((fields, optional)) = hgl_semantics::any_values::fields(ty) {
        let mut code = "let mut rest=data;".to_string();
        for (i, ty) in fields.iter().enumerate() {
            let n = name(ty);
            if optional.contains(&i) {
                crate::append(
                    &mut code,
                    format_args!(
                        "{{let field=cold_frame(&mut rest)?;match field.first() {{Some(0) if field.len()==1=>(),Some(1)=>any_validate_{n}(field.slice(1..field.len()))?,_=>return Err(cold_error())}}}}"
                    ),
                );
            } else {
                crate::append(
                    &mut code,
                    format_args!("any_validate_{n}(cold_frame(&mut rest)?)?;"),
                );
            }
        }
        code.push_str("if !rest.is_empty(){return Err(cold_error())}");
        return code;
    }
    scalar(ty)
}

fn scalar(ty: &Ty) -> String {
    let temporal = match ty.scalar_kind() {
        Some(hgl_types::ScalarType::TimeZone) => {
            Some((0, "hgl_source::TemporalLiteral::TimeZone(zone.into())"))
        }
        Some(hgl_types::ScalarType::ZonedTime) => Some((
            8,
            "hgl_source::TemporalLiteral::ZonedTime{time_micros:cold_number(&data[..8])? as i64,zone:zone.into()}",
        )),
        Some(hgl_types::ScalarType::ZonedDateTime) => Some((
            16,
            "hgl_source::TemporalLiteral::ZonedDateTime{instant_micros:cold_number(&data[..8])? as i64,offset_seconds:i32::try_from(cold_number(&data[8..16])? as i64).map_err(|_|cold_error())?,zone:zone.into()}",
        )),
        _ => None,
    };
    if let Some((width, recipe)) = temporal {
        return format!(
            "let zone=std::str::from_utf8(data.get({width}..).ok_or_else(cold_error)?).map_err(|_|cold_error())?;cold_temporal(&{recipe})?;"
        );
    }
    match ty.scalar_kind() {
        Some(hgl_types::ScalarType::Bool) => "if data.len()!=1 || data[0]>1 {return Err(cold_error())}".into(),
        Some(hgl_types::ScalarType::Text) => "std::str::from_utf8(&data).map_err(|_|cold_error())?;".into(),
        Some(hgl_types::ScalarType::Bytes) => String::new(),
        Some(hgl_types::ScalarType::Time) => "if !(0..hgl_types::calendar::DAY).contains(&(cold_number(&data)? as i64)){return Err(cold_error())}".into(),
        _ => "if data.len()!=8 {return Err(cold_error())}".into(),
    }
}

pub(crate) fn support() -> &'static str {
    r#"
fn cold_temporal(recipe:&hgl_source::TemporalLiteral)->hgl_types::NodeResult{
 let mut context=hgl_source::time_context::RunContext::from_bundled().map_err(hgl_types::NodeError::new)?;
 context.materialize(recipe).map_err(hgl_types::NodeError::new)?;Ok(())
}
fn cold_error()->Box<hgl_types::NodeError>{hgl_types::NodeError::new("invalid cold payload encoding")}
fn cold_number(bytes:&[u8])->hgl_types::NodeResult<u64>{Ok(u64::from_le_bytes(bytes.try_into().map_err(|_|cold_error())?))}
fn cold_frame<'a>(rest:&mut hgl_types::any_values::AnyView<'a>)->hgl_types::NodeResult<hgl_types::any_values::AnyView<'a>>{
 let width=usize::try_from(cold_number(rest.get(..8).ok_or_else(cold_error)?)?).map_err(|_|cold_error())?;
 let end=width.checked_add(8).filter(|end|*end<=rest.len()).ok_or_else(cold_error)?;
 let field=rest.slice(8..end);*rest=rest.slice(end..rest.len());Ok(field)
}
"#
}
