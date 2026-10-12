//! Incompatible canonical identities fail before shared layout or provider text emission.
use hgl_semantics::{
    harness_ir::{Step, Test},
    ir::{Kind, Plan, Statement, Value},
    native_images,
};
use hgl_source::{
    EnumType, Literal, Ty,
    native_contracts::{Image, Scalar},
};

fn native_plan(identity: &str) -> Plan {
    let scalar = Scalar {
        identity: identity.into(),
        capabilities: ["copy".into(), "text".into()].into(),
    };
    Plan {
        ordinary_types: vec![Ty::Native(scalar.clone())],
        native_images: vec![Image {
            provider: "identity-provider".into(),
            fingerprint: "identity-v1".into(),
            path: "unavailable_identity_provider.rs".into(),
            bounds: std::collections::BTreeMap::new(),
            scalars: vec![(scalar, "Token".into())],
        }],
        ..Plan::default()
    }
}
fn rejected(source: &str) {
    assert!(source.starts_with("compile_error!("), "{source}");
    assert!(source.contains("native canonical identity conflicts with non-native HGL type"));
    assert!(!source.contains("unavailable_identity_provider.rs"));
    assert!(!source.contains("pub fn register") && !source.contains("struct Global"));
}

#[test]
fn primitive_collision_returns_only_a_target_diagnostic() {
    let mut plan = native_plan("i64");
    plan.ordinary_types.push(Ty::I64);
    assert!(native_images::scalars(&plan).is_err());
    rejected(&hgl_rust::emit(&plan));
    rejected(&hgl_rust::emit_shared(&plan));
}

#[test]
fn separately_checked_enum_and_native_plans_cannot_share_one_codec_identity() {
    let native = native_plan("ordinary::Label");
    let ordinary = Plan {
        ordinary_types: vec![Ty::Enum(EnumType {
            origin: "ordinary::Label".into(),
            members: vec![("ready".into(), 1)],
        })],
        ..Plan::default()
    };
    assert!(native_images::scalars(&native).is_ok());
    assert!(native_images::scalars(&ordinary).is_ok());
    assert!(native_images::combined_scalars(&[native.clone(), ordinary.clone()], &[]).is_err());
    rejected(&hgl_rust::shared_layouts(&[native, ordinary]));
}

#[test]
fn graph_native_storage_and_lexical_nominals_are_checked_together() {
    let plan = native_plan("ordinary::Row");
    let row = Ty::Struct(
        "ordinary::Row".into(),
        vec![("value".into(), Ty::I64)],
        vec![],
    );
    let test = Test {
        name: "ordinary::fixture".into(),
        steps: vec![Step::Ordinary(Statement::Let(
            0,
            Value::new(
                row,
                Kind::Construct(vec![(
                    0,
                    Value::new(Ty::I64, Kind::Literal(Literal::Int(1))),
                )]),
            ),
        ))],
    };
    assert!(native_images::scalars(&plan).is_ok());
    assert!(native_images::harness_scalars(std::slice::from_ref(&test)).is_ok());
    assert!(
        native_images::combined_scalars(std::slice::from_ref(&plan), std::slice::from_ref(&test))
            .is_err()
    );
    rejected(&hgl_rust::shared_test_layouts(&[plan], &[test]));
    let mut distinct = native_plan("atomic<i64>");
    distinct.ordinary_types.extend([Ty::I64, Ty::I64.atomic()]);
    assert_eq!(native_images::scalars(&distinct).unwrap().len(), 1);
}
