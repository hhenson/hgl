use hgl_store::{InputId, Kind, OutputId, Store};
use hgl_types::{EngineTime, ScalarType, ScalarValue};
use std::collections::BTreeMap;
pub(crate) fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n + 1)
}
pub(crate) type Observed = BTreeMap<String, String>;
fn object(fields: &BTreeMap<String, String>) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(k, v)| format!("{k:?}:{v}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn keys(names: impl Iterator<Item = String>) -> String {
    format!(
        "[{}]",
        names
            .map(|k| format!("{k:?}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn key(n: i64) -> String {
    n.to_string()
}
#[expect(
    clippy::too_many_lines,
    reason = "one recursive diagnostic snapshot records the complete independently specified observation surface"
)]
pub(crate) fn snapshot(
    store: &Store,
    input: InputId,
    t: i64,
    path: &str,
    rows: &mut Observed,
) -> (String, String) {
    let b = store.bindings();
    let valid = b.valid(input);
    let modified = b.modified(input, at(t));
    let mut fields = BTreeMap::new();
    let mut delta_fields = BTreeMap::new();
    let kind = &b.input(input).kind;
    let dictionary = matches!(
        kind,
        Kind::Growing(_) | Kind::Dictionary(_) | Kind::KeyedDictionary(..)
    );
    let children: Vec<_> = if dictionary {
        b.keys(input)
            .map(|k| {
                (
                    key(k),
                    b.child_input(input, k).unwrap_or_else(|| unreachable!()),
                )
            })
            .collect()
    } else {
        (0..kind.len())
            .map(|n| {
                (
                    if let Kind::Bundle(fs) = kind {
                        fs[n].0.clone()
                    } else {
                        n.to_string()
                    },
                    b.fixed_input(input, n),
                )
            })
            .collect()
    };
    for (name, child) in &children {
        let (value, delta) = snapshot(store, *child, t, &format!("{path}/children/{name}"), rows);
        fields.insert(name.clone(), value);
        if b.valid(*child) && b.modified(*child, at(t)) {
            delta_fields.insert(name.clone(), delta);
        }
    }
    if dictionary {
        let removed: Vec<_> = b.removed_keys(input).collect();
        let mut retired = BTreeMap::new();
        for &k in &removed {
            delta_fields.insert(key(k), "{\"$remove\":true}".into());
            if let Some(child) = b.removed_input(input, k) {
                snapshot(store, child, t, &format!("{path}/retired/{}", key(k)), rows);
                retired.insert(key(k), String::new());
            }
        }
        rows.insert(
            format!("{path}/retired"),
            if retired.is_empty() {
                "[]".into()
            } else {
                keys(retired.into_keys())
            },
        );
        rows.insert(format!("{path}/keys"), keys(b.keys(input).map(key)));
        rows.insert(format!("{path}/added"), keys(b.added_keys(input).map(key)));
        rows.insert(
            format!("{path}/removed"),
            keys(removed.into_iter().map(key)),
        );
    }
    let value = if valid {
        match kind {
            Kind::Ts(ScalarType::I64) => store
                .get(
                    store
                        .scalar_input::<i64>(input)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .to_string(),
            Kind::Ts(ScalarType::Bool) => store
                .get(
                    store
                        .scalar_input::<bool>(input)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .to_string(),
            Kind::Ts(ScalarType::F64) => store
                .get(
                    store
                        .scalar_input::<f64>(input)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .to_string(),
            Kind::List(..) => format!(
                "[{}]",
                fields.values().cloned().collect::<Vec<_>>().join(",")
            ),
            Kind::Bundle(_)
            | Kind::Growing(_)
            | Kind::Dictionary(_)
            | Kind::KeyedDictionary(..) => object(&fields),
            Kind::Atomic(_)
            | Kind::Rolling(..)
            | Kind::Reference(_)
            | Kind::KeyedSet(_)
            | Kind::Set(_)
            | Kind::Ts(
                ScalarType::Text
                | ScalarType::Date
                | ScalarType::Time
                | ScalarType::DateTime
                | ScalarType::Duration
                | ScalarType::CivilDateTime
                | ScalarType::TimeZone
                | ScalarType::ZonedTime
                | ScalarType::ZonedDateTime,
            ) => unreachable!("observed separately"),
        }
    } else {
        "null".into()
    };
    let delta = if !valid || !modified {
        "null".into()
    } else if matches!(kind, Kind::Ts(_)) {
        value.clone()
    } else {
        object(&delta_fields)
    };
    for (name, value) in [
        ("valid", valid.to_string()),
        ("all_valid", b.all_valid(input).to_string()),
        ("modified", modified.to_string()),
        (
            "last",
            if b.last_modified(input) == EngineTime::NEVER {
                "\"never\"".into()
            } else {
                (b.last_modified(input).micros() - 1).to_string()
            },
        ),
        ("value", value.clone()),
        ("delta", delta.clone()),
        ("peer", b.has_peer(input).to_string()),
    ] {
        rows.insert(format!("{path}/{name}"), value);
    }
    rows.insert(
        format!("{path}/children"),
        if fields.is_empty() {
            "[]".into()
        } else {
            keys(fields.into_keys())
        },
    );
    (value, delta)
}
pub(crate) fn output(store: &Store, id: OutputId, t: i64, path: &str, rows: &mut Observed) {
    let b = store.bindings();
    let out = b.output(id);
    let valid = out.modified_at != EngineTime::NEVER;
    let modified = store.output_modified(id, at(t));
    let mut fields = BTreeMap::new();
    let mut deltas = BTreeMap::new();
    for (n, &child) in out.fixed.iter().enumerate() {
        let name = if let Kind::Bundle(fs) = &out.kind {
            fs[n].0.clone()
        } else {
            n.to_string()
        };
        let p = format!("{path}/children/{name}");
        output(store, child, t, &p, rows);
        fields.insert(name.clone(), rows[&format!("{p}/value")].clone());
        if (b.output(child).modified_at != EngineTime::NEVER) && store.output_modified(child, at(t))
        {
            deltas.insert(name, rows[&format!("{p}/delta")].clone());
        }
    }
    let value = if !valid {
        "null".into()
    } else if out.kind.fixed() {
        if matches!(out.kind, Kind::List(..)) {
            format!(
                "[{}]",
                fields.values().cloned().collect::<Vec<_>>().join(",")
            )
        } else {
            object(&fields)
        }
    } else {
        match store.output_value_erased(id).unwrap() {
            ScalarValue::I64(v) => v.to_string(),
            ScalarValue::Bool(v) => v.to_string(),
            ScalarValue::F64(v) => v.to_string(),
            ScalarValue::Text(v) => v,
            ScalarValue::Date(v) => v.0.to_string(),
            ScalarValue::Time(v) => v.0.to_string(),
            ScalarValue::DateTime(v) => v.micros().to_string(),
            ScalarValue::Duration(v) => v.micros().to_string(),
            ScalarValue::CivilDateTime(v) => v.micros().to_string(),
            ScalarValue::TimeZone(v) => v.as_str().into(),
            ScalarValue::ZonedTime(v) => format!("{}@{}", v.time().0, v.zone().as_str()),
            ScalarValue::ZonedDateTime(v) => format!(
                "{}@{}:{}",
                v.instant().micros(),
                v.zone().as_str(),
                v.offset_seconds()
            ),
        }
    };
    let delta = if !valid || !modified {
        "null".into()
    } else if out.kind.fixed() {
        object(&deltas)
    } else {
        value.clone()
    };
    for (name, value) in [
        ("valid", valid.to_string()),
        ("modified", modified.to_string()),
        (
            "last",
            if out.modified_at == EngineTime::NEVER {
                "\"never\"".into()
            } else {
                (out.modified_at.micros() - 1).to_string()
            },
        ),
        ("value", value),
        ("delta", delta),
        ("all_valid", all_valid(store, id).to_string()),
        ("children", keys(fields.into_keys())),
    ] {
        rows.insert(format!("{path}/{name}"), value);
    }
}
fn all_valid(s: &Store, id: OutputId) -> bool {
    let o = s.bindings().output(id);
    o.modified_at != EngineTime::NEVER
        && o.fixed
            .iter()
            .all(|&c| s.bindings().output(c).modified_at != EngineTime::NEVER)
}
