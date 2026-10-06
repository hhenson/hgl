//! Static rolling shape and arrival transport emission.
use hgl_rust_layouts::global_type;
use hgl_source::{Ty, WindowKind};
/// Exact runtime marker, independent of the ordinary arrival representation.
pub fn marker(ty: &Ty) -> String {
    let Ty::Rolling(payload, window) = ty else {
        unreachable!("checked rolling shape")
    };
    format!(
        "hgl_store::Rolling<{}, {}, {}, {}>",
        global_type(payload),
        window.kind() == WindowKind::Duration,
        window.maximum(),
        window.minimum()
    )
}
/// Read a retained arrival without substituting the whole held window.
pub fn read(ty: &Ty, input: &str) -> String {
    format!(
        "_ctx.store().rolling.borrow::<{}>(_ctx.store().bindings(),{input})?.read(_ctx.store().rolling.values())?",
        marker(ty)
    )
}
/// Native arrival publication into independent reserved output storage.
pub fn apply(ty: &Ty, output: &str, payload: &str) -> String {
    format!(
        "_ctx.prepared().rolling::<{}>({output},&({payload}))?;",
        marker(ty)
    )
}
/// Prepared source-to-output arrival copy.
pub fn from(ty: &Ty, output: &str, source: &str, slot: &str) -> String {
    format!(
        "_ctx.prepared().rolling_from::<{}>({source},{slot},{output})?;",
        marker(ty)
    )
}
/// Direct independently retained arrival forwarding.
pub fn pass(ty: &Ty, input: &str, output: &str) -> String {
    format!(
        "_ctx.prepared().pass_rolling::<{}>({input},{output})?;",
        marker(ty)
    )
}
/// Capture the latest arrival into an independent prepared ordinary recording slot.
pub fn capture(ty: &Ty, input: &str, slot: &str) -> String {
    let Ty::Rolling(payload, _) = ty else {
        unreachable!("checked rolling shape")
    };
    let value = global_type(payload);
    format!(
        "{{let source=observation.rolling.borrow::<{}>(observation.bindings,{input})?;<{value} as hgl_store::PreparedValue>::check_slots(observation.rolling.values(),source,columns,{slot})?;<{value} as hgl_store::PreparedValue>::copy_between(observation.rolling.values(),source,columns,{slot});}}",
        marker(ty)
    )
}
/// Current retained-window readiness, with no idle eviction.
pub fn ready(ty: &Ty, input: &str) -> String {
    format!(
        "_ctx.store().rolling.ready::<{}>(_ctx.store().bindings(),{input})",
        marker(ty)
    )
}

/// Window-specific endpoint queries; other queries use ordinary endpoint metadata.
pub fn query(ty: &Ty, op: &str, input: &str) -> Option<String> {
    if !matches!(ty, Ty::Rolling(..)) {
        return None;
    }
    match op {
        "delta_value" => Some(read(ty, input)),
        "all_valid" => Some(ready(ty, input)),
        _ => None,
    }
}
