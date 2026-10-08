//! Typed observation presence without consuming or defaulting retained children.
use crate::layouts::global_type;
use hgl_semantics::ir::{Kind, Value};
use hgl_source::Ty;
fn child(ty: &Ty, optional: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut _ctx.store().global_values().prepared_list(({optional}).fields()).first().ok_or_else(||hgl_types::NodeError::new(\"unprepared snapshot child\"))?.as_slice())",
        global_type(&crate::snapshots::storage(ty))
    )
}
/// Project typed storage and presence; unset children do not require payloads.
pub fn view(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    projected(value, &emit)
}
fn projected(value: &Value, emit: &dyn Fn(&Value) -> String) -> String {
    match &value.kind {
        Kind::Local(id) | Kind::MutableLocal(id) => format!("local{id}"),
        Kind::Field(parent, index) => format!(
            "{{let parent={};let optional=parent.0.fields().{index};let present=parent.1&&!_ctx.store().global_values().list(optional.fields()).is_empty();({},present)}}",
            projected(parent, emit),
            child(&value.ty, "optional")
        ),
        Kind::Index(parent, index) => {
            let Ty::List(wrapper, _) = crate::snapshots::storage(&parent.ty) else {
                unreachable!("list snapshot");
            };
            format!(
                "{{let parent={};let index={};let item={{let columns=_ctx.store().global_values();let positions=hgl_store::list_index(columns.list(parent.fields()),index)?;hgl_store::ValueSlot::<{}>::bind(&mut positions.as_slice())}};let optional=item.fields().0;let present=!_ctx.store().global_values().list(optional.fields()).is_empty();({},present)}}",
                payload(parent, emit),
                emit(index),
                global_type(&wrapper),
                child(&value.ty, "optional")
            )
        }
        Kind::Construct(_)
        | Kind::Captured(..)
        | Kind::Delta(_)
        | Kind::List(_)
        | Kind::Native(..)
        | Kind::Query(..)
        | Kind::ValueCall(..)
        | Kind::Push(..)
        | Kind::Binary(..)
        | Kind::Length(_)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Unary(..)
        | Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => unreachable!("prepared snapshot view"),
    }
}
fn required(value: &Value, emit: impl Fn(&Value) -> String, coded: bool) -> String {
    let failure = if coded {
        "hgl_types::NodeError::coded(\"required ordinary observation is unset\",\"value.unset_read\")"
    } else {
        "hgl_types::NodeError::new(\"ordinary tuple input is invalid\")"
    };
    format!(
        "{{let view={};if !view.1 {{return Err({failure});}}view.0}}",
        view(value, emit)
    )
}
/// Consume presence through the catalogued required-read failure channel.
pub fn payload(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    required(value, emit, true)
}
/// Keep whole structural publication exclusions separate from required scalar reads.
pub fn publication(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    required(
        value,
        emit,
        !matches!(
            value.ty,
            Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Map(..)
        ),
    )
}
/// Independently retain presence and copy only a present payload into prepared storage.
pub fn copy(value: &Value, destination: &str, emit: impl Fn(&Value) -> String) -> String {
    let marker = global_type(&crate::snapshots::storage(&value.ty));
    format!(
        "{{let source={};if source.1 {{let columns=_ctx.global_state().values_mut();<{marker} as hgl_store::PreparedValue>::check_slots(columns,source.0,columns,{destination})?;<{marker} as hgl_store::PreparedValue>::copy_within(columns,source.0,{destination});}}source.1}}",
        view(value, emit)
    )
}
/// Assemble an optional child without consuming an absent observation.
pub fn constructor(value: &Value, optional: &str, emit: impl Fn(&Value) -> String) -> String {
    format!(
        "let destination={};let present={};_ctx.global_state().values_mut().set_list_len(({optional}).fields(),usize::from(present));",
        child(&value.ty, optional),
        copy(value, "destination", emit)
    )
}
/// Require List presence before reading its prepared length, including fixed Lists.
pub fn length(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    format!(
        "{{let source={};hgl_store::list_len(_ctx.store().global_values().list(source.fields()))?}}",
        payload(value, emit)
    )
}
