//! Accepted fixed scenarios replayed against Rust; fixtures never use Rust output.
mod fixed_support;
use fixed_support::{Observed, Wakes, assembled, at, leaf, pair, scalar, set, snapshot, verify};
use hgl_store::{BindError, Kind, Reference, Store};
use hgl_types::{EngineTime, NodeId, ScalarType};

#[expect(
    clippy::too_many_lines,
    reason = "explicit independent scenario actions remain together for comparison with the accepted timelines"
)]
fn collection_case(case: &str) -> Result<(), BindError> {
    let parts: Vec<_> = case.split('_').collect();
    let nested = parts.get(1).is_some_and(|p| *p == "tsl" || *p == "tsb");
    let dictionary = case.contains("dictionary");
    let heterogeneous = case.contains("heterogeneous");
    let owned = case.contains("owned");
    let passive = case.contains("passive");
    let latest = case.contains("latest_invalid");
    let invalidation = case.ends_with("_invalidate");
    let mut shape = pair(
        parts[0] == "tsb",
        if nested {
            pair(parts[1] == "tsb", scalar())
        } else if dictionary {
            Kind::Dictionary(Box::new(scalar()))
        } else {
            scalar()
        },
    );
    if heterogeneous {
        shape = Kind::Bundle(vec![
            ("left".into(), scalar()),
            ("right".into(), Kind::Scalar(ScalarType::Bool)),
        ]);
    }
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let out = store.add_shaped_output(NodeId(0), shape.clone());
    let input = store.add_shaped_input(NodeId(2), shape, !passive);
    if owned {
        store.bind(input, out)?;
    } else {
        let r = assembled(&mut store, out, case.ends_with("mixed"))?;
        store.sample(input, r, EngineTime::NEVER, &mut wakes)?;
    }
    let length = if dictionary || latest || heterogeneous {
        6
    } else if passive {
        7
    } else if invalidation {
        9
    } else if nested {
        8
    } else {
        10
    };
    let mut rows = Observed::new();
    for t in 0..length {
        store.begin_cycle(at(t));
        if dictionary {
            let side = usize::from(t == 1);
            let dict = leaf(&store, out, &[side]);
            if t == 0 || t == 1 || t == 4 {
                let child = store.get_or_create_shaped(dict, i64::from(t == 1), at(t), &mut wakes);
                set(
                    &mut store,
                    child,
                    &[],
                    if t == 0 {
                        7
                    } else if t == 1 {
                        9
                    } else {
                        11
                    },
                    t,
                    &mut wakes,
                )?;
            } else if t == 2 {
                store.remove_shaped(dict, 0, at(t), &mut wakes);
            }
        } else if nested {
            for (tick, path, value) in [
                (1, [0, 0], 7),
                (2, [1, 0], 9),
                (3, [0, 1], 8),
                (5, [1, 1], 10),
                (6, [0, 0], 11),
            ] {
                if t == tick {
                    set(&mut store, out, &path, value, t, &mut wakes)?;
                }
            }
            if invalidation && t == 7 {
                if owned {
                    store.invalidate(out, at(t), &mut wakes);
                } else {
                    invalidate_leaves(&mut store, out, t, &mut wakes);
                }
            }
        } else if heterogeneous {
            if t == 1 || t == 3 {
                set(
                    &mut store,
                    out,
                    &[0],
                    if t == 1 { 7 } else { 8 },
                    t,
                    &mut wakes,
                )?;
            }
            if t == 2 || t == 4 {
                let child = store.scalar_output::<bool>(leaf(&store, out, &[1]))?;
                store.set(child, t == 4, at(t), NodeId(0), &mut wakes);
            }
        } else if latest {
            for (tick, pos, v) in [(0, 0, 7), (1, 1, 9), (4, 1, 11)] {
                if t == tick {
                    set(&mut store, out, &[pos], v, t, &mut wakes)?;
                }
            }
            if t == 2 {
                store.invalidate(leaf(&store, out, &[1]), at(t), &mut wakes);
            }
        } else if passive {
            for (tick, pos, v) in [(1, 0, 7), (2, 1, 9), (3, 0, 8), (5, 1, 10)] {
                if t == tick {
                    set(&mut store, out, &[pos], v, t, &mut wakes)?;
                }
            }
        } else {
            for (tick, pos, v) in [(1, 0, 7), (2, 1, 9), (6, 0, 7), (8, 1, 11)] {
                if t == tick {
                    set(&mut store, out, &[pos], v, t, &mut wakes)?;
                }
            }
            if t == 4 || t == 5 || t == 7 {
                let id = if t == 7 && owned {
                    out
                } else {
                    leaf(&store, out, &[usize::from(t == 5)])
                };
                store.invalidate(id, at(t), &mut wakes);
            }
        }
        let path = format!("/ticks/{t}");
        if passive && ![1, 4, 6].contains(&t) {
            rows.insert(path, "null".into());
        } else {
            snapshot(&store, input, t, &path, &mut rows);
        }
    }
    verify(
        case,
        &rows,
        usize::try_from(length).unwrap_or_else(|_| unreachable!()),
    );
    Ok(())
}
#[test]
fn accepted_collection_traces() -> Result<(), BindError> {
    for outer in ["tsl", "tsb"] {
        for mode in ["owned", "assembled"] {
            collection_case(&format!("{outer}_{mode}"))?;
            collection_case(&format!("{outer}_dictionary_{mode}"))?;
            for inner in ["tsl", "tsb"] {
                collection_case(&format!("{outer}_{inner}_{mode}"))?;
                collection_case(&format!("{outer}_{inner}_{mode}_invalidate"))?;
            }
        }
        collection_case(&format!("{outer}_latest_invalid"))?;
        collection_case(&format!("{outer}_passive"))?;
    }
    for case in [
        "tsl_tsb_mixed",
        "tsb_tsl_mixed",
        "tsb_heterogeneous_owned",
        "tsb_heterogeneous_assembled",
    ] {
        collection_case(case)?;
    }
    Ok(())
}
#[test]
fn accepted_reference_traces() -> Result<(), BindError> {
    for case in [
        "tsl_ref_whole",
        "tsl_ref_items",
        "tsb_ref_whole",
        "tsb_ref_items",
        "tsl_tsb_ref",
        "tsb_tsl_ref",
    ] {
        reference_case(case)?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "keeps the REF action sequence and all three observations auditable together"
)]
fn reference_case(case: &str) -> Result<(), BindError> {
    let parts: Vec<_> = case.split('_').collect();
    let nested = parts[1] != "ref";
    let shape = pair(
        parts[0] == "tsb",
        if nested {
            pair(parts[1] == "tsb", scalar())
        } else {
            scalar()
        },
    );
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let a = store.add_shaped_output(NodeId(0), shape.clone());
    let b = store.add_shaped_output(NodeId(0), shape.clone());
    let r = store.add_shaped_output(NodeId(1), Kind::Reference(Box::new(shape.clone())));
    let data = store.add_shaped_input(NodeId(2), shape.clone(), true);
    let ai = store.add_shaped_input(NodeId(2), shape.clone(), false);
    let bi = store.add_shaped_input(NodeId(2), shape.clone(), false);
    store.bind(ai, a)?;
    store.bind(bi, b)?;
    store.follow(data, r, at(0), &mut wakes)?;
    let ab = store.items_reference(
        shape.clone(),
        vec![
            store.reference(leaf(&store, a, &[0])),
            store.reference(leaf(&store, b, &[1])),
        ],
    )?;
    let empty_b = store.items_reference(
        shape,
        vec![Reference::default(), store.reference(leaf(&store, b, &[1]))],
    )?;
    let mut rows = Observed::new();
    for t in 0..9 {
        store.begin_cycle(at(t));
        if t == 0 {
            if nested {
                for (out, path, v) in [
                    (a, [0, 0], 7),
                    (a, [1, 0], 9),
                    (b, [0, 0], 20),
                    (b, [0, 1], 21),
                ] {
                    set(&mut store, out, &path, v, t, &mut wakes)?;
                }
            } else {
                for (out, pos, v) in [(a, 0, 7), (a, 1, 9), (b, 0, 20), (b, 1, 30)] {
                    set(&mut store, out, &[pos], v, t, &mut wakes)?;
                }
            }
        }
        if [0, 1, 2, 4, 6, 7].contains(&t) {
            let target = match t {
                0 => Reference::default(),
                1 | 2 | 7 => store.reference(a),
                4 => {
                    if case.ends_with("items") {
                        ab
                    } else {
                        store.reference(b)
                    }
                }
                6 => {
                    if case.ends_with("items") {
                        empty_b
                    } else {
                        Reference::default()
                    }
                }
                _ => unreachable!(),
            };
            store.set_reference(r, target, at(t), &mut wakes)?;
        }
        if t == 3 {
            set(
                &mut store,
                a,
                if nested { &[0, 1] } else { &[0] },
                8,
                t,
                &mut wakes,
            )?;
        }
        if t == 5 {
            set(
                &mut store,
                b,
                if nested { &[1, 0] } else { &[1] },
                if nested { 30 } else { 31 },
                t,
                &mut wakes,
            )?;
        }
        let path = format!("/ticks/{t}");
        if nested {
            snapshot(&store, data, t, &path, &mut rows);
        } else {
            for (name, input) in [("data", data), ("a", ai), ("b", bi)] {
                snapshot(&store, input, t, &format!("{path}/{name}"), &mut rows);
            }
            rows.insert(
                format!("{path}/ref_valid"),
                (store.bindings().output(r).modified_at != EngineTime::NEVER).to_string(),
            );
            rows.insert(
                format!("{path}/ref_modified"),
                store.output_modified(r, at(t)).to_string(),
            );
        }
    }
    verify(case, &rows, 9);
    Ok(())
}
#[test]
fn accepted_compound_dictionary_retirement() -> Result<(), BindError> {
    for case in ["tsd_tsl_tsb", "tsd_tsb_tsl"] {
        let bundle = case == "tsd_tsb_tsl";
        let shape = Kind::Dictionary(Box::new(pair(bundle, pair(!bundle, scalar()))));
        let mut store = Store::new();
        let mut wakes = Wakes::default();
        let out = store.add_shaped_output(NodeId(0), shape.clone());
        let input = store.add_shaped_input(NodeId(2), shape, true);
        store.bind(input, out)?;
        let mut rows = Observed::new();
        let mut saved = Reference::default();
        for t in 0..6 {
            store.begin_cycle(at(t));
            if [0, 1, 4].contains(&t) {
                let child = store.get_or_create_shaped(out, 0, at(t), &mut wakes);
                set(
                    &mut store,
                    child,
                    &[usize::from(t == 1), 0],
                    if t == 0 {
                        7
                    } else if t == 1 {
                        9
                    } else {
                        11
                    },
                    t,
                    &mut wakes,
                )?;
                if t == 0 {
                    saved = store.reference(leaf(&store, child, &[0, 0]));
                }
            }
            if t == 2 {
                store.remove_shaped(out, 0, at(t), &mut wakes);
                assert!(store.bindings().resolve(saved).is_some());
            }
            if t >= 3 {
                assert!(store.bindings().resolve(saved).is_none());
            }
            snapshot(&store, input, t, &format!("/ticks/{t}"), &mut rows);
        }
        verify(case, &rows, 6);
    }
    Ok(())
}

fn invalidate_leaves(store: &mut Store, output: hgl_store::OutputId, t: i64, wakes: &mut Wakes) {
    for path in [[0, 0], [0, 1], [1, 0], [1, 1]] {
        store.invalidate(leaf(store, output, &path), at(t), wakes);
    }
}
