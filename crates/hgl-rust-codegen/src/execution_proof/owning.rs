use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
fn owning(ty: &Ty) -> bool {
    !(ty.scalar_kind().is_some() && !ty.owning_scalar() || matches!(ty, Ty::Enum(_) | Ty::Void))
}

/// Whether ordinary helper argument retention can allocate native owning storage.
pub fn owning_argument(ty: &Ty) -> bool {
    if let Ty::Tuple(children) = ty {
        return children.iter().any(owning_argument);
    }
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().any(|(_, child)| owning_argument(child));
    }
    if let Ty::Family(family) = ty {
        let members = family.members();
        return members.iter().any(|(_, child)| owning_argument(child));
    }
    if let Ty::Nullable(child) | Ty::Atomic(child) = ty {
        return owning_argument(child);
    }
    // Other layouts keep the conservative owning boundary; builtin metadata supplies fixed leaves.
    owning(ty)
}

fn value(input: &Value, looping: bool) -> bool {
    match &input.kind {
        Kind::Captured(..) => false,
        Kind::Unary(op, _) if op == "bytes" => false,
        Kind::Native(binding, args) => {
            (!owning(&input.ty) || matches!(input.ty,Ty::Result(..)|Ty::Error) || binding.name == "hgraph.native::as_str" || crate::native_results::supported(input))
                && args.iter().all(|arg| value(arg, looping))
        }
        Kind::ValueCall(args, body) => {
            (!owning(&input.ty)||matches!(input.ty,Ty::Result(..)|Ty::Error))
                && args.iter().all(|arg| value(arg, looping))
                && statements(body, looping, &input.ty)
        }
        Kind::Push(parent, item) => {
            !looping && value(parent, looping) && value(item, looping)
        }
        Kind::List(_) if !input.closed() => false,
        Kind::Query(op, args) if op.starts_with("collection_") => {
            !looping
                && matches!(
                    op.as_str(),
                    "collection_insert"
                        | "collection_update"
                        | "collection_upsert"
                        | "collection_remove"
                        | "collection_invalidate"
                        | "collection_contains"
                )
                && args.first().is_some_and(
                    |v| matches!(&v.ty,Ty::Map(key,child) if **key==Ty::I64 && (op=="collection_invalidate" || !owning(child))),
                )
                && args
                    .get(1)
                    .is_some_and(|v| matches!(v.kind, Kind::Literal(hgl_source::Literal::Int(_))))
                && args.iter().all(|arg| value(arg, looping))
        }
        Kind::Query(op, args) if op.starts_with("\0lookup_") => {
            let child = if let Ty::Nullable(child) = &input.ty { child.as_ref() } else { &input.ty };
            !owning_argument(child)
                && args.get(1).is_some_and(|key| !owning_argument(&key.ty) || key.ty == Ty::Str || matches!(key.ty,Ty::Native(_) | Ty::Any) && matches!(key.kind,Kind::Input(_,false) | Kind::Configuration(_)))
                && args.iter().all(|arg| value(arg, looping))
        }
        Kind::List(args) | Kind::Query(_, args) => args.iter().all(|arg| value(arg, looping)),
        Kind::Construct(fields) => fields.iter().all(|(_, child)| value(child, looping)),
        Kind::Delta(parts) => {
            input.closed()
                && parts
                    .iter()
                    .all(|part| part.operands().all(|child| value(child, looping)))
        }
        Kind::Index(left, right) | Kind::Binary(_, left, right) => {
            value(left, looping) && value(right, looping)
        }
        Kind::Field(child, _)
        | Kind::Length(child)
        | Kind::GlobalSet(_, child)
        | Kind::IsPresent(child)
        | Kind::Present(child)
        | Kind::Unary(_, child) => value(child, looping),
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::Opaque(_) | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => true,
    }
}
fn statements(body: &[Statement], looping: bool, result: &Ty) -> bool {
    body.iter().all(|statement| match statement {
        Statement::Assign(target, source) => {
            !(looping && owning(&target.ty)) && value(target, looping) && value(source, looping)
        }
        Statement::While(condition, body)
        | Statement::ForItems(_, _, _, condition, body)
        | Statement::For(_, condition, body) => {
            value(condition, looping) && statements(body, true, result)
        }
        Statement::If(condition, yes, no) => {
            value(condition, looping)
                && statements(yes, looping, result)
                && statements(no, looping, result)
        }
        Statement::Return(source)
            if crate::scalars::bytes_input(source).is_some()
                && result.clone().delta().is_ok_and(|ty| ty == Ty::Bytes) =>
        {
            true
        }
        Statement::Return(source) if crate::direct_deltas::supported(source) => true,
        Statement::Let(_, source)
        | Statement::Var(_, source)
        | Statement::Borrow(_, source, _)
        | Statement::Return(source)
        | Statement::Yield(source)
        | Statement::Call(source) => value(source, looping),
        Statement::TimedYield(time, payload) => value(time, looping) && value(payload, looping),
        Statement::Exit => true,
    })
}
pub(super) fn finite(plan: &Plan) -> bool {
    plan.nodes.iter().all(|node| {
        node.handlers
            .iter()
            .all(|(guard, _)| guard.as_ref().is_none_or(|guard| value(guard, false)))
            && [&node.start, &node.stop]
                .into_iter()
                .chain(node.generator.iter())
                .chain(node.handlers.iter().map(|(_, body)| body))
                .all(|body| statements(body, false, &node.result))
    })
}
