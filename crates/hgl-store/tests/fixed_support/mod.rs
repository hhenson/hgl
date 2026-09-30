//! Observations compared with the independently accepted cross-runtime corpus.
use hgl_store::{BindError, InputId, Kind, OutputId, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Wakes(pub(crate) usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
pub(crate) fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n + 1)
}
pub(crate) fn scalar() -> Kind {
    Kind::Ts(ScalarType::I64)
}
pub(crate) fn pair(bundle: bool, child: Kind) -> Kind {
    if bundle {
        Kind::Bundle(vec![
            ("left".into(), child.clone()),
            ("right".into(), child),
        ])
    } else {
        Kind::List(Box::new(child), 2)
    }
}
pub(crate) fn leaf(store: &Store, mut output: OutputId, positions: &[usize]) -> OutputId {
    for &p in positions {
        output = store.bindings().fixed_output(output, p);
    }
    output
}
pub(crate) fn set(
    store: &mut Store,
    output: OutputId,
    positions: &[usize],
    value: i64,
    t: i64,
    wakes: &mut Wakes,
) -> Result<(), BindError> {
    let id = leaf(store, output, positions);
    let out = store.scalar_output::<i64>(id)?;
    store.set(out, value, at(t), NodeId(0), wakes);
    Ok(())
}
pub(crate) fn assembled(
    store: &mut Store,
    output: OutputId,
    mixed: bool,
) -> Result<Reference, BindError> {
    let kind = store.bindings().output(output).kind.clone();
    if !kind.fixed() {
        return Ok(store.reference(output));
    }
    let children = (0..kind.len())
        .map(|n| {
            let child = store.bindings().fixed_output(output, n);
            if mixed && n == 0 {
                Ok(store.reference(child))
            } else {
                assembled(store, child, false)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    store.items_reference(kind, children)
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
    if n == 0 { "X".into() } else { "Y".into() }
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
    let dictionary = matches!(kind, Kind::Dictionary(_));
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
                "{}".into()
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
            Kind::Bundle(_) | Kind::Dictionary(_) => object(&fields),
            Kind::Reference(_)
            | Kind::Set(_)
            | Kind::Ts(
                ScalarType::Text
                | ScalarType::Date
                | ScalarType::Time
                | ScalarType::DateTime
                | ScalarType::Duration,
            ) => {
                unreachable!("membership and REF observed separately")
            }
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
        ("peer", b.input(input).source.is_some().to_string()),
    ] {
        rows.insert(format!("{path}/{name}"), value);
    }
    rows.insert(
        format!("{path}/children"),
        if fields.is_empty() {
            "{}".into()
        } else {
            keys(fields.into_keys())
        },
    );
    (value, delta)
}
pub(crate) fn verify(case: &str, rows: &Observed, length: usize) {
    let mut checked = 0;
    for line in include_str!("accepted.tsv").lines() {
        let parts: Vec<_> = line.split('\t').collect();
        if parts[0] != case {
            continue;
        }
        let actual = if parts[2] == "length" {
            length.to_string()
        } else {
            rows.get(parts[1])
                .cloned()
                .unwrap_or_else(|| format!("missing {}", parts[1]))
        };
        assert_eq!(actual, parts[3], "{case} {}", parts[1]);
        checked += 1;
    }
    assert!(checked > 0, "missing accepted case {case}");
}
