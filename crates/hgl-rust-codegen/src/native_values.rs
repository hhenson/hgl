//! Static native scalar markers and cold owning conversion at checked provider boundaries.
use hgl_source::native_contracts::Scalar;
/// Private marker name depends on canonical identity, never on an HGL import alias.
pub fn marker(scalar: &Scalar) -> String {
    format!(
        "NativeScalar{}",
        crate::identifiers::identifier(&scalar.identity)
    )
}
/// Serialize inert canonical scalar metadata without evaluating provider code.
pub(crate) fn metadata(scalar: &Scalar) -> String {
    let capabilities = scalar
        .capabilities
        .iter()
        .map(|c| format!("{c:?}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "hgl_source::native_contracts::Scalar {{identity:{:?}.into(),capabilities:[{capabilities}].into_iter().map(String::from).collect()}}",
        scalar.identity
    )
}
/// Retain cold opaque data through the statically selected typed provider operation.
pub(crate) fn decode(scalar: &Scalar) -> String {
    format!(
        "let hgl_semantics::ir::Kind::Opaque(value)=&v.kind else {{return Err(\"prepared native scalar required\".into())}}; hgl_store::native_values::NativeView::<{}>::from_provider(value.data()).retain().map_err(|e|format!(\"{{:?}}: {{}}\",e.code,e.message))?",
        marker(scalar)
    )
}
/// Capture an independently owning cold value with retained provider callback lifetime.
pub(crate) fn encode(scalar: &Scalar) -> String {
    let marker = marker(scalar);
    format!(
        "hgl_semantics::ir::Kind::Opaque(hgl_source::native_value::NativeValue::new(v.view().retain().unwrap_or_else(|_|unreachable!(\"validated native capture\")).into_owned(),&hgl_source::native_value::Operations {{copy:|data,out|<{marker} as hgl_store::native_values::NativeType>::copy(data,out),equal:<{marker} as hgl_store::native_values::NativeType>::equal,hash:<{marker} as hgl_store::native_values::NativeType>::hash,compare:<{marker} as hgl_store::native_values::NativeType>::compare}}))"
    )
}

/// Private key domain identifier for an exact canonical scalar.
pub fn domain(scalar: &Scalar) -> String {
    crate::identifiers::identifier(&scalar.identity)
}

/// Append an exact opaque provider leaf into an owning or sizing sink.
pub(crate) fn append(scalar: &Scalar, data: &str) -> String {
    let marker = marker(scalar);
    format!("out.native({data},<{marker} as hgl_store::native_values::NativeType>::copy);")
}
