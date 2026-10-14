//! Owning outcome codecs retain selected payloads and complete diagnostic provenance.
use crate::any_values::name;
use hgl_source::Ty;

pub(crate) fn fields(ty: &Ty) -> Option<(Vec<Ty>, Vec<usize>)> {
    (*ty == Ty::Error).then(|| {
        let schema = crate::constructor_fields::schema(ty);
        hgl_semantics::any_values::fields(&schema)
            .unwrap_or_else(|| unreachable!("diagnostic schema"))
    })
}
fn context(ty: &Ty) -> bool {
    matches!(ty,Ty::Struct(id,..) if id.source_name()=="\0error.context")
}
pub(crate) fn operation(ty: &Ty) -> Option<String> {
    if let Ty::Result(ok, err) = ty {
        Some(format!(
            "hgl_types::AnyOperation::Result(&ANY_{},&ANY_{})",
            name(ok),
            name(err)
        ))
    } else if *ty == Ty::Error {
        Some(format!(
            "hgl_types::AnyOperation::Error(&ANY_{})",
            name(&Ty::Str)
        ))
    } else {
        None
    }
}
pub(crate) fn codecs(ty: &Ty) -> Option<(String, String)> {
    if let Ty::Result(ok, err) = ty {
        return Some((
            format!(
                "match v {{Ok(v)=>{{out.write(&[0]);out.frame(|out|any_write_{}(v,out));}},Err(v)=>{{out.write(&[1]);out.frame(|out|any_write_{}(v,out));}}}}",
                name(ok),
                name(err)
            ),
            format!(
                "{{let payload=hgl_types::any_values::Frames(data.slice(1..data.len())).next().expect(\"outcome payload\");if data[0]==0 {{Ok(any_read_{}(payload)?)}}else{{Err(any_read_{}(payload)?)}}}}",
                name(ok),
                name(err)
            ),
        ));
    }
    let properties = if *ty == Ty::Error {
        vec![
            ("message", Ty::Str, false),
            ("code", Ty::Str, true),
            ("context", fields(ty)?.0[2].clone(), true),
        ]
    } else if context(ty) {
        vec![
            ("node", Ty::Str, false),
            ("phase", Ty::Str, false),
            ("evaluation_time", Ty::DateTime, false),
        ]
    } else {
        return None;
    };
    let mut write = String::new();
    let mut read = String::new();
    for (field, child, optional) in properties {
        let child = name(&child);
        if optional {
            crate::append(
                &mut write,
                format_args!(
                    "out.frame(|out|if let Some(v)=&v.{field} {{out.write(&[1]);any_write_{child}(v,out);}}else{{out.write(&[0]);}});"
                ),
            );
            crate::append(
                &mut read,
                format_args!(
                    "{field}:{{let data=frames.next().expect(\"diagnostic field\");if data[0]==0 {{None}}else{{Some(any_read_{child}(data.slice(1..data.len()))?)}}}},"
                ),
            );
        } else {
            crate::append(
                &mut write,
                format_args!("out.frame(|out|any_write_{child}(&v.{field},out));"),
            );
            crate::append(
                &mut read,
                format_args!(
                    "{field}:any_read_{child}(frames.next().expect(\"diagnostic field\"))?,"
                ),
            );
        }
    }
    let native = if *ty == Ty::Error {
        "ErrorValue"
    } else {
        "ErrorContext"
    };
    Some((
        write,
        format!(
            "{{let mut frames=hgl_types::any_values::Frames(data);hgl_types::{native} {{ {read} }} }}"
        ),
    ))
}
pub(crate) fn slots(ty: &Ty) -> Option<String> {
    let Ty::Result(ok, err) = ty else { return None };
    let mut arms = String::new();
    for (index, child) in [(1, ok), (2, err)] {
        let marker = crate::layouts::global_type(child);
        crate::append(
            &mut arms,
            format_args!(
                "{index}=>{{let positions=&columns.list(slot.fields().{index}.fields())[0];let child=hgl_store::ValueSlot::<{marker}>::bind(&mut positions.as_slice());out.frame(|out|any_slots_{}(columns,child,out));}},",
                name(child)
            ),
        );
    }
    Some(format!(
        "let success=*columns.scalar::<bool>(slot.fields().0.fields());out.write(&[u8::from(!success)]);match if success {{1}}else{{2}} {{{arms}_=>unreachable!(\"outcome tag\")}}"
    ))
}
pub(crate) fn copy(ty: &Ty) -> Option<String> {
    let Ty::Result(ok, err) = ty else { return None };
    Some(format!(
        "out.write(&data[..1]);let payload=hgl_types::any_values::Frames(data.slice(1..data.len())).next().expect(\"outcome payload\");any_copy_frame(out,|out|if data[0]==0 {{any_copy_{}(payload,out)}}else{{any_copy_{}(payload,out)}});",
        name(ok),
        name(err)
    ))
}
pub(crate) fn validate(ty: &Ty) -> Option<String> {
    let Ty::Result(ok, err) = ty else { return None };
    Some(format!(
        "let tag=*data.first().ok_or_else(cold_error)?;let mut rest=data.slice(1..data.len());let payload=cold_frame(&mut rest)?;if !rest.is_empty() {{return Err(cold_error())}}match tag {{0=>any_validate_{}(payload)?,1=>any_validate_{}(payload)?,_=>return Err(cold_error())}};",
        name(ok),
        name(err)
    ))
}
pub(crate) fn bound(ty: &Ty) -> Option<String> {
    let Ty::Result(ok, err) = ty else { return None };
    Some(format!(
        "9usize.saturating_add(v.field1.as_ref().map_or(0,|v|any_bound_{}(v)).max(v.field2.as_ref().map_or(0,|v|any_bound_{}(v))))",
        name(ok),
        name(err)
    ))
}
