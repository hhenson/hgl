//! Syntax-neutral walks preserve operand order and explicit subtree pruning.
use hgl_value_model::Ty;
use hgl_value_model::ir::{DeltaEntry, Kind, Statement, Value};
use hgl_value_model::ir_visit;

fn local(id: usize) -> Value {
    Value::new(Ty::I64, Kind::Local(id))
}
#[test]
fn ordered_walks_preserve_delta_operands_and_helper_pruning() {
    let helper = Value::new(
        Ty::I64,
        Kind::ValueCall(vec![local(3)], vec![Statement::Yield(local(4))]),
    );
    let mut value = Value::new(
        Ty::I64,
        Kind::Delta(vec![
            DeltaEntry::Keyed(local(1), local(2)),
            DeltaEntry::Child(0, helper),
        ]),
    );
    let mut read = Vec::new();
    ir_visit::value(&value, &mut |v| {
        if let Kind::Local(id) = v.kind {
            read.push(id);
        }
        true
    });
    assert_eq!(read, [1, 2, 3, 4]);
    let mut rewritten = Vec::new();
    ir_visit::value_mut(&mut value, &mut |v| {
        if let Kind::Local(id) = &mut v.kind {
            rewritten.push(*id);
            *id += 10;
        }
        true
    });
    assert_eq!(rewritten, read);
    let mut pruned = Vec::new();
    ir_visit::value(&value, &mut |v| {
        if let Kind::Local(id) = v.kind {
            pruned.push(id);
        }
        !matches!(v.kind, Kind::ValueCall(..))
    });
    assert_eq!(pruned, [11, 12]);
}
