//! Cold recursive endpoint allocation, independent of Store's typed tick API.
use hgl_bindings::{Bindings, Kind, OutputId};
use hgl_columns::{Columns, Scalar};
use hgl_types::{NodeId, ScalarType};
/// Allocate a scalar slot with the existing generation and column reuse policy.
pub fn scalar<T: Scalar>(
    bindings: &mut Bindings,
    columns: &mut Columns,
    owner: NodeId,
) -> OutputId {
    let (id, fresh) = bindings.add_output(
        owner,
        Kind::Ts(T::TYPE),
        u32::try_from(T::column(columns).len())
            .unwrap_or_else(|_| unreachable!("column capacity exceeded")),
    );
    if fresh {
        T::column_mut(columns).push(T::default());
    }
    id
}
/// Allocate an exact shape and all fixed descendants before graph execution.
pub fn output(
    bindings: &mut Bindings,
    columns: &mut Columns,
    atomic: &mut hgl_atomic::Arena,
    owner: NodeId,
    kind: Kind,
) -> OutputId {
    if let Kind::Atomic(ty) = kind {
        return atomic.add_output(bindings, owner, ty);
    }
    if let Kind::Ts(ty) = kind {
        return match ty {
            ScalarType::Bool => scalar::<bool>(bindings, columns, owner),
            ScalarType::I64 => scalar::<i64>(bindings, columns, owner),
            ScalarType::F64 => scalar::<f64>(bindings, columns, owner),
            ScalarType::Text => scalar::<String>(bindings, columns, owner),
            ScalarType::Date => scalar::<hgl_types::Date>(bindings, columns, owner),
            ScalarType::Time => scalar::<hgl_types::Time>(bindings, columns, owner),
            ScalarType::DateTime => scalar::<hgl_types::EngineTime>(bindings, columns, owner),
            ScalarType::Duration => scalar::<hgl_types::EngineDelta>(bindings, columns, owner),
            ScalarType::CivilDateTime => {
                scalar::<hgl_types::CivilDateTime>(bindings, columns, owner)
            }
            ScalarType::TimeZone => scalar::<hgl_types::ZoneId>(bindings, columns, owner),
            ScalarType::ZonedTime => scalar::<hgl_types::ZonedTime>(bindings, columns, owner),
            ScalarType::ZonedDateTime => {
                scalar::<hgl_types::ZonedDateTime>(bindings, columns, owner)
            }
        };
    }
    let children = (0..kind.len())
        .map(|n| output(bindings, columns, atomic, owner, kind.child(n).clone()))
        .collect::<Vec<_>>();
    let id = bindings.add_output(owner, kind, 0).0;
    for child in children {
        let result = bindings.append_fixed(id, child);
        debug_assert!(result.is_ok());
    }
    id
}
