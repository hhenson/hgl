//! Complete ordinary publication, separate from sparse delta application.
use crate::layouts::{global_type, rust_type};
use hgl_semantics::ir::Value;
use hgl_source::Ty;
use std::fmt::Write as _;
fn append(code: &mut String, args: std::fmt::Arguments<'_>) {
    code.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
fn slot(ty: &Ty, fields: &str, columns: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut {columns}.prepared_list(({fields}).fields()).first().ok_or_else(||hgl_types::NodeError::new(\"unprepared snapshot child\"))?.as_slice())",
        global_type(&crate::snapshots::storage(ty))
    )
}
fn element(ty: &Ty, parent: &str, index: &str, columns: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut {columns}.prepared_list(({parent}).fields())[{index}].as_slice())",
        global_type(ty)
    )
}
fn native_present(ty: &Ty, value: &str) -> String {
    crate::snapshots::children(ty).map_or_else(
        || "true".into(),
        |children| {
            children
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    format!(
                        "({value}).{i}.as_ref().is_some_and(|value|{})",
                        native_present(child, "value")
                    )
                })
                .collect::<Vec<_>>()
                .join("||")
        },
    )
}
fn required(present: &str) -> String {
    format!(
        "if !({present}) {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}"
    )
}
/// Publish the complete held value, using existing child invalidation semantics.
pub fn native(ty: &Ty, output: &str, value: &str) -> String {
    if let Some(children) = crate::snapshots::children(ty) {
        let mut code = format!(
            "{{let output={output};let value={value};{}",
            required(&native_present(ty, "value"))
        );
        for (i, ty) in children.iter().enumerate() {
            append(
                &mut code,
                format_args!(
                    "{{let output=output.field::<{i}>(_ctx.store().bindings());if let Some(value)=value.{i} {{{}}} else {{_ctx.invalidate(output.id());}}}}",
                    native(ty, "output", "value")
                ),
            );
        }
        return code + "}";
    }
    match ty {
        Ty::Tuple(_) | Ty::Struct(..) => unreachable!("positional children handled above"),
        Ty::List(..) | Ty::Map(..) => {
            unreachable!("native collection publication requires prepared observations")
        }
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => {
            if let Some(ty) = crate::layouts::whole_payload(ty) {
                format!("_ctx.set_atomic::<{}>({output},{value})?;", global_type(ty))
            } else {
                format!("_ctx.set(hgl_store::Store::prepared_output({output}),{value});")
            }
        }
    }
}
/// Apply complete retained children directly from prepared ordinary storage.
pub fn prepared(ty: &Ty, output: &str, source: &str) -> String {
    if let Some(children) = crate::snapshots::children(ty) {
        let any=children.iter().enumerate().map(|(i,_)|format!("!_ctx.store().global_values().list(({source}).fields().{i}.fields()).is_empty()")).collect::<Vec<_>>().join("||");
        return children.iter().enumerate().fold(required(&any),|mut code,(i,child)| {
            append(&mut code,format_args!("{{let optional=({source}).fields().{i};let output=({output}).field::<{i}>(_ctx.store().bindings());if !_ctx.store().global_values().list(optional.fields()).is_empty() {{let source={};{}}} else {{_ctx.invalidate(output.id());}}}}",slot(child,"optional","_ctx.store().global_values()"),prepared(child,"output","source")));code
        });
    }
    if let Ty::List(child, size) = ty {
        let n = size.map_or_else(
            || format!("_ctx.store().global_values().list(({source}).fields()).len()"),
            |n| n.to_string(),
        );
        let output_child = if size.is_some() {
            "output.index(_ctx.store().bindings(),index)".into()
        } else {
            String::from(
                "{let key=index as i64;_ctx.get_or_create_with(output.id(),key,|_,_|unreachable!(\"finite snapshot member prepared before start\"));output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing prepared member\"))?}",
            )
        };
        let Ty::List(wrapper, _) = crate::snapshots::storage(ty) else {
            unreachable!()
        };
        let check = if size.is_some() {
            required(&format!(
                "(0..{n}).any(|index|{{let item={};!_ctx.store().global_values().list(item.fields().0.fields()).is_empty()}})",
                element(&wrapper, "source", "index", "_ctx.store().global_values()")
            ))
        } else {
            String::new()
        };
        let absent = if size.is_some() {
            "else {_ctx.invalidate(output.id());}"
        } else {
            ""
        };
        return format!(
            "{{let source={source};let output={output};{check}for index in 0..{n} {{let item={};let optional=item.fields().0;let output={output_child};if !_ctx.store().global_values().list(optional.fields()).is_empty() {{let source={};{}}}{absent}}}}}",
            element(&wrapper, "source", "index", "_ctx.store().global_values()"),
            slot(child, "optional", "_ctx.store().global_values()"),
            prepared(child, "output", "source")
        );
    }
    if let Ty::Map(key, child) = ty {
        let Ty::Map(_, wrapper) = crate::snapshots::storage(ty) else {
            unreachable!()
        };
        let pair = Ty::Tuple(vec![(**key).clone(), *wrapper]);
        let marker = global_type(key);
        let item = element(&pair, "source", "next", "_ctx.store().global_values()");
        let key_read = format!(
            "{{let item={item};let prepared=_ctx.prepared();<{marker} as hgl_store::Key>::id(prepared.storage.keys,prepared.storage.globals.values().scalar::<{}>(item.fields().0.fields()))?}}",
            rust_type(key)
        );
        let child_slot = slot(child, "optional", "_ctx.store().global_values()");
        let copy = prepared(child, "child", "child_source");
        let check = required(&format!(
            "(0..count).any(|index|{{let item={};!_ctx.store().global_values().list(item.fields().1.fields().0.fields()).is_empty()}})",
            element(&pair, "source", "index", "_ctx.store().global_values()")
        ));
        return format!(
            "{{let source={source};let output={output};let count=_ctx.store().global_values().list(source.fields()).len();{check}let mut next=0;let mut current=if next<count {{Some({key_read})}}else{{None}};let domain_len=_ctx.store().bindings().output(output.id()).members.prepared.len();for position in 0..domain_len {{let key=_ctx.store().bindings().output(output.id()).members.prepared[position].0;if current==Some(key) {{let item={item};let optional=item.fields().1.fields().0;if !_ctx.store().global_values().list(optional.fields()).is_empty() {{_ctx.get_or_create_with(output.id(),key,|_,_|unreachable!(\"finite snapshot member prepared before start\"));let child=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing prepared member\"))?;let child_source={child_slot};{copy}}}else{{let child=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"invalid snapshot map child requires existing membership\"))?;_ctx.invalidate(child.id());}}next+=1;current=if next<count {{Some({key_read})}}else{{None}};}}else if output.member(_ctx.store().bindings(),key).is_some() {{_ctx.remove_shaped(output.id(),key);}}}}if next!=count {{return Err(hgl_types::NodeError::new(\"snapshot key outside prepared output domain\"));}}}}"
        );
    }
    format!(
        "_ctx.prepared().scalar_from_global::<{}>(({output}).id(),({output}).generation(),({source}).fields())?;",
        rust_type(ty)
    )
}
/// Publish to an existing own output without terminating the rest of the handler.
pub fn assignment(value: &Value, emit: impl Fn(&Value) -> String) -> Option<String> {
    if !value.snapshot && !matches!(value.ty, Ty::Tuple(_) | Ty::Struct(..)) {
        return None;
    }
    let expression = if value.snapshot {
        crate::snapshot_views::publication(value, &emit)
    } else {
        crate::snapshots::complete(&value.ty, &emit(value))
    };
    let publication = if value.snapshot {
        prepared(&value.ty, "self._output", "publication")
    } else {
        native(&value.ty, "self._output", "publication")
    };
    Some(format!("{{let publication={expression};{publication}}}\n"))
}
