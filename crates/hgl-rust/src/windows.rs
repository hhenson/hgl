//! Static rolling shape and arrival transport emission.
use crate::layouts::global_type;
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
/// Borrow a scalar source into an independently retained rolling arrival.
pub fn scalar(ty: &Ty, input: &str, output: &str) -> String {
    format!(
        "_ctx.prepared().rolling_scalar::<{}>({input},{output})?;",
        marker(ty)
    )
}
/// Prepared source-to-output arrival copy.
pub fn from(ty: &Ty, output: &str, source: &str, slot: &str) -> String {
    format!(
        "_ctx.prepared().rolling_from::<{}>(Some({source}),{slot},{output})?;",
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

/// Publish an arrival using the checked destination representation and policy.
pub fn forward(source: &Ty, result: &Ty, input: &str, output: &str) -> Option<String> {
    let Ty::Rolling(payload, _) = source else {
        return None;
    };
    let method = if let Ty::Rolling(target, _) = result {
        if target != payload {
            return None;
        }
        "pass_rolling_as"
    } else if let Some(target) = crate::layouts::whole_payload(result) {
        if target != payload.as_ref() {
            return None;
        }
        "atomic_from_rolling"
    } else if result == payload.as_ref() && result.scalar() {
        "scalar_from_rolling"
    } else {
        return None;
    };
    let shape = marker(source);
    let arguments = if method == "pass_rolling_as" {
        format!("{shape},{}", marker(result))
    } else {
        shape
    };
    Some(format!(
        "_ctx.prepared().{method}::<{arguments}>({input},{output})?;"
    ))
}
