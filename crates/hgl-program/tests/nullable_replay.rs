//! Checking acceptance for nullable replay slots, independent of execution traces.
use hgl_program::compile_tests;

const READ: &str = "let item = replay_input[current]\n        if item != null {\n            return item\n        }";

fn sources(body: &str, ty: &str, slots: &str) -> Vec<(String, String)> {
    let implementation =
        include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl");
    assert!(
        implementation.contains(READ),
        "standard replay read changed"
    );
    vec![
        (
            "example.hgl".into(),
            format!(
                "module example\nfn id(value: {ty}) -> {ty} {{ when {{ return delta_value(value) }} }}\ntest check {{ eval(id, value: {slots}) }}"
            ),
        ),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            implementation.replace(READ, body),
        ),
    ]
}

fn accepts(body: &str) {
    let result = compile_tests(&sources(body, "i64", "[1, _, 2]"));
    assert!(result.is_ok(), "expected acceptance: {body}\n{result:?}");
}

fn rejects(body: &str, diagnostic: &str) {
    let result = compile_tests(&sources(body, "i64", "[1, _, 2]"));
    assert!(
        matches!(&result, Err(error) if error.contains(diagnostic)),
        "expected rejection containing {diagnostic:?}: {body}\n{result:?}"
    );
}

#[test]
fn direct_null_comparisons_refine_both_branch_directions() {
    for condition in [
        "item != null",
        "null != item",
        "!(item == null)",
        "!((null == item))",
    ] {
        accepts(&format!(
            "let item = replay_input[current]\nif {condition} {{ return item }}"
        ));
    }
    for condition in [
        "item == null",
        "null == item",
        "!(item != null)",
        "!((null != item))",
    ] {
        accepts(&format!(
            "let item = replay_input[current]\nif {condition} {{ return }} else {{ return item }}"
        ));
    }
    accepts(
        "let item = replay_input[current]\nif item != null { if item != null { return item } }",
    );
}

#[test]
fn short_circuit_facts_follow_each_evaluation_path() {
    for body in [
        "let item = replay_input[current]\nif item != null && item > 0 { return item }",
        "let item = replay_input[current]\nif item == null || item > 0 { return } else { return item }",
        "let a = replay_input[current]\nlet b = replay_input[0]\nif a != null && b != null { return a + b }",
        "let a = replay_input[current]\nlet b = replay_input[0]\nif a == null || b == null { return } else { return a + b }",
    ] {
        accepts(body);
    }
    for body in [
        "let item = replay_input[current]\nif item != null || item > 0 { return 1 }",
        "let item = replay_input[current]\nif item == null && item > 0 { return 1 }",
        "let a = replay_input[current]\nlet b = replay_input[0]\nif a != null || b != null { return a + b }",
        "let item = replay_input[current]\nif item == null || item > 0 { return item + 1 }",
    ] {
        rejects(body, "presence proof");
    }
}

#[test]
fn joins_discard_terminated_paths_but_keep_possible_absence() {
    for body in [
        "let item = replay_input[current]\nif item == null { return }\nreturn item",
        "let item = replay_input[current]\nif item == null { if current > 0 { return } else { return } }\nreturn item",
        "let item = replay_input[current]\nif item == null { if item != null {} else { return } }\nreturn item",
        "let a = replay_input[current]\nlet b = replay_input[0]\nif a == null || b == null { return }\nreturn a + b",
        "let item = replay_input[current]\nif item == null { return } else { let ordinary = item }\nreturn item",
    ] {
        accepts(body);
    }
    for body in [
        "let item = replay_input[current]\nif item == null { if current > 0 { return } }\nreturn item + 1",
        "let item = replay_input[current]\nif item != null { let ordinary = item }\nreturn item + 1",
    ] {
        rejects(body, "presence proof");
    }
}

#[test]
fn alias_and_shadow_facts_belong_to_individual_bindings() {
    for body in [
        "let item = replay_input[current]\nlet alias = item\nif item != null { if alias != null { return alias } }",
        "let item = replay_input[current]\nif item != null { let alias = item\nreturn alias + item }",
        "let item = replay_input[current]\nif item != null { if current > 0 { let item = replay_input[0]\nif item != null { let copy = item } }\nreturn item }",
    ] {
        accepts(body);
    }
    for body in [
        "let item = replay_input[current]\nlet alias = item\nif item != null { return alias + 1 }",
        "let item = replay_input[current]\nlet alias = item\nif alias != null { return item + 1 }",
        "let item = replay_input[current]\nif item != null { let item = replay_input[0]\nreturn item + 1 }",
    ] {
        rejects(body, "presence proof");
    }
}

#[test]
fn repeated_reads_and_saved_comparisons_do_not_supply_facts() {
    for body in [
        "if replay_input[current] != null { return replay_input[current] + 1 }",
        "let item = replay_input[current]\nlet present = item != null\nif present { return item + 1 }",
        "let item = replay_input[current]\nlet alias = item\nif item == null { return }\nreturn alias + 1",
    ] {
        rejects(body, "presence proof");
    }
}

#[test]
fn handler_locals_are_not_visible_to_a_subsequent_handler() {
    rejects(
        "let previous = replay_input[current]\n}\nwhen {\nlet escaped = previous",
        "unknown value previous",
    );
}

#[test]
fn unproven_payloads_cannot_escape_or_be_used_as_values() {
    for (body, diagnostic) in [
        (
            "let item = replay_input[current]\nreturn item",
            "return type",
        ),
        (
            "let item = replay_input[current]\nindex = item",
            "assignment type",
        ),
        (
            "let item = replay_input[current]\nvar escaped = item",
            "presence proof",
        ),
        (
            "let item = replay_input[current]\nif item == 1 { return 1 }",
            "presence proof",
        ),
        (
            "let item = replay_input[current]\nlet alias = item\nif item == alias { return 1 }",
            "presence proof",
        ),
        (
            "let item = replay_input[current]\nif item { return 1 }",
            "bool",
        ),
        (
            "let item = replay_input[current]\nunknown_helper(item)",
            "presence proof",
        ),
        (
            "let item = replay_input[current]\nlet values = [item]",
            "harness sequences",
        ),
    ] {
        rejects(body, diagnostic);
    }
    let input = sources(
        "let item = replay_input[current]\nif item { return true }",
        "bool",
        "[false, _, true]",
    );
    let result = compile_tests(&input);
    assert!(
        matches!(&result, Err(error) if error.contains("bool")),
        "{result:?}"
    );
}

#[test]
fn presence_allows_false_and_repeated_owned_text_payload_use() {
    for (body, ty, slots) in [
        (
            "let item = replay_input[current]\nif item != null && item { return item }",
            "bool",
            "[false, _, true]",
        ),
        (
            "let item = replay_input[current]\nif item != null { let alias = item\nreturn alias + item }",
            "str",
            "[\"\", _, \"text\"]",
        ),
    ] {
        let result = compile_tests(&sources(body, ty, slots));
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn old_replay_operations_and_method_aliases_are_rejected() {
    for body in [
        "if replay_input.has_tick(current) { return replay_input.delta_at(current) }",
        "return replay_input.delta_at(current)",
        "if has_tick(replay_input, current) { return 1 }",
        "return delta_at(replay_input, current)",
        "let item = replay_input.get(current)",
        "let item = get(replay_input, current)",
        "let count = replay_input.length()",
        "let count = length(replay_input)",
        "alarm.schedule(0s)",
        "let now = clock.evaluation_time()",
    ] {
        let result = compile_tests(&sources(body, "i64", "[1, _, 2]"));
        assert!(result.is_err(), "old syntax accepted: {body}");
    }
}

#[test]
fn indexing_requires_i64_and_the_evaluation_phase() {
    for index in ["true", "1.0", "\"0\""] {
        rejects(
            &format!("let item = replay_input[{index}]"),
            "index requires i64",
        );
    }
    let mut input = sources(READ, "i64", "[1, _, 2]");
    input[2].1 = input[2].1.replace(
        "if len(replay_input) > 0",
        "let forbidden = replay_input[0]\nif len(replay_input) > 0",
    );
    let result = compile_tests(&input);
    assert!(
        matches!(&result, Err(error) if error.contains("in evaluation")),
        "{result:?}"
    );
}

#[test]
fn named_capability_arguments_require_proven_payloads() {
    for (body, accepted) in [
        (
            "let item = replay_input[current]\nif item != null { schedule(alarm, delay: item)\nreturn item }",
            true,
        ),
        (
            "let item = replay_input[current]\nschedule(alarm, delay: item)",
            false,
        ),
    ] {
        let result = compile_tests(&sources(body, "duration", "[0s, _, 1s]"));
        if accepted {
            assert!(result.is_ok(), "{body}: {result:?}");
        } else {
            assert!(
                matches!(&result, Err(error) if error.contains("presence proof")),
                "{body}: {result:?}"
            );
        }
    }
}

#[test]
fn owned_output_assignment_cannot_unwrap_a_nullable_local() {
    for (body, accepted) in [
        (
            "let item = replay_input[current]\nif item != null { out = item }",
            true,
        ),
        ("let item = replay_input[current]\nout = item", false),
    ] {
        let mut input = sources(body, "i64", "[1, _, 2]");
        input[2].1 = input[2].1.replace(
            "inject replay_input, alarm, clock",
            "inject replay_input, alarm, clock, out",
        );
        let result = compile_tests(&input);
        if accepted {
            assert!(result.is_ok(), "{body}: {result:?}");
        } else {
            assert!(
                matches!(&result, Err(error) if error.contains("assignment type")),
                "{body}: {result:?}"
            );
        }
    }
}
