//! Run the unmodified shared suite and probe the harness independently of it.
use hgl_program::{compile_tests, emit_tests};
use std::{
    fmt::Write,
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn source(body: &str) -> Vec<(String, String)> {
    vec![
        ("eval.hgl".into(), format!("module example\n{body}")),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}

#[test]
fn eval_reports_type_and_phase_errors_before_emission() {
    for (body, diagnostic) in [
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { assert eval(id, [true]) == [1] }",
            "expected i64",
        ),
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { assert eval(id, [1]) == [true] }",
            "expected output",
        ),
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { eval(id, 1) }",
            "requires a sequence",
        ),
        (
            "fn sink(x: i64) { when {} }\ntest bad { assert eval(sink, [1]) == [1] }",
            "outputless",
        ),
        (
            "fn bad(x: i64) -> i64 { when { return x / 2 } }\ntest bad_div { eval(bad, [3]) }",
            "return type",
        ),
        ("test { export fn helper() {} }", "test context"),
        ("test { use other }", "test context"),
        (
            "test { native const fn native(x: i64) -> i64 }",
            "test context",
        ),
        ("test { test { fn nested() {} } }", "test context"),
        (
            "fn bad(x: i64) -> i64 { inject out\n when { passivate(out)\n return x } }\ntest bad_call { eval(bad, [1]) }",
            "activity requires",
        ),
        (
            "fn bad(x: i64) -> datetime { when { return last_modified(x, x) } }\ntest bad_call { eval(bad, [1]) }",
            "exactly one",
        ),
    ] {
        let error = compile_tests(&source(body)).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
}

#[test]
fn capabilities_and_delta_metadata_are_checked_at_their_call_sites() {
    for (definition, diagnostic) in [
        (
            "fn f(x:i64)->i64 { when { return delta(x) } }",
            "example::delta: expected one matching declaration",
        ),
        (
            "fn f(replay_input:i64)->i64 { when { return len(replay_input) } }",
            "expected one matching declaration",
        ),
        (
            "fn f(capture:i64) { when { begin(capture) } }",
            "expected one matching declaration",
        ),
        (
            "fn f(clock:i64)->datetime { when { return clock.evaluation_time } }",
            "missing inject clock",
        ),
        (
            "fn f(x:i64) { inject capture\nwhen { let local=capture\nbegin(local) } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) -> i64 { when { return replay_input[0] } }",
            "unknown value replay_input",
        ),
        (
            "fn f(x: i64) -> i64 { inject replay_input\nwhen { return x } }",
            "unsupported injectable replay_input",
        ),
        (
            "fn f(x: i64) -> i64 { inject capture\nwhen { return x } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture, clock\nstart { append(capture, clock.evaluation_time, 1) }\nwhen {} }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture\nwhen { begin(capture) } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture\nstart { begin(capture) }\nwhen { append(capture, last_modified(x), true) } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture\nwhen { let escaped = capture } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture\nwhen { missing(capture) } }",
            "unsupported injectable capture",
        ),
        (
            "fn f(x: i64) { inject capture\nstart { let y = x }\nwhen {} }",
            "unsupported injectable capture",
        ),
    ] {
        assert_bad_definition(definition, diagnostic);
    }
}

fn assert_bad_definition(definition: &str, diagnostic: &str) {
    let result = compile_tests(&source(&format!(
        "{definition}\ntest bad {{ eval(f, [1]) }}"
    )));
    assert!(
        matches!(&result, Err(error) if error.contains(diagnostic)),
        "{definition}: {result:?}"
    );
}

#[test]
fn delta_metadata_preserves_endpoint_identity_and_presence_proof() {
    for (definition, diagnostic) in [
        (
            "fn f(x: i64) -> i64 { when { return delta_value(1) } }",
            "temporal input endpoint",
        ),
        (
            "fn f(x: i64) -> i64 { when { let copy = x\nreturn delta_value(copy) } }",
            "temporal input endpoint",
        ),
        (
            "fn f(x: i64) -> i64 { start { let y = delta_value(x) }\nwhen { return x } }",
            "in evaluation",
        ),
        (
            "const fn f(x: i64) -> i64 { if valid(x) && modified(x) { return delta_value(x) } }",
            "in evaluation",
        ),
        (
            "fn f(x: i64) -> i64 { when { return delta_value(x,x) } }",
            "one runtime input",
        ),
    ] {
        assert_bad_definition(definition, diagnostic);
    }
    for (body, diagnostic) in [
        ("return delta_value(a)", "valid and modified"),
        (
            "if valid(b) && modified(b) { return delta_value(a) }",
            "valid and modified",
        ),
        (
            "if valid(a) || modified(a) { return delta_value(a) }",
            "valid and modified",
        ),
    ] {
        let input = format!(
            "fn f(a:i64,b:i64)->i64 {{ when {{ {body} }} }}\ntest bad {{ eval(f,[1],[2]) }}"
        );
        let error = compile_tests(&source(&input)).unwrap_err();
        assert!(error.contains(diagnostic), "{input}: {error}");
    }
}

#[test]
fn scalar_producers_do_not_erase_formal_signal_admission() {
    for (ty, samples) in [
        ("bool", "[false,true]"),
        ("i64", "[1,2]"),
        ("f64", "[1.0,2.0]"),
        ("str", "[\"a\",\"b\"]"),
        ("date", "[@2026-01-01,@2026-01-02]"),
        ("time", "[@00:00:01,@00:00:02]"),
        ("datetime", "[@2026-01-01T00:00:01Z,@2026-01-01T00:00:02Z]"),
        ("duration", "[1us,2us]"),
    ] {
        for guard in ["", "valid(value) && modified(value)"] {
            let input = source(&format!(
                "fn f(value:signal)->{ty} {{ when {guard} {{ return delta_value(value) }} }}\ntest bad {{ eval(f,{samples}) }}"
            ));
            let error = compile_tests(&input).unwrap_err();
            assert!(
                error.contains("signal is not admitted"),
                "{ty}, {guard}: {error}"
            );
        }
        let input = source(&format!(
            "fn f(value:signal) {{ inject capture\nstart {{ begin(capture) }}\nwhen {{}} }}\ntest bad {{ eval(f,{samples}) }}"
        ));
        let error = compile_tests(&input).unwrap_err();
        assert!(
            error.contains("unsupported injectable capture"),
            "{ty}: {error}"
        );
    }
    let error = compile_tests(&source("fn f(trigger:signal,value:i64)->i64 { when modified(trigger) { return delta_value(trigger) } }\ntest bad { eval(f,[1],[2]) }")).unwrap_err();
    assert!(error.contains("signal is not admitted"), "{error}");
}

#[test]
fn type_domains_and_library_provisioning_are_explicit() {
    let input = "module example\nfn id(x:i64)->i64 { when { return x } }\ntest t { eval(id,[1]) }";
    let error = compile_tests(&[("example.hgl".into(), input.into())]).unwrap_err();
    assert!(
        error.contains("eval requires the ordinary TimedValue declaration"),
        "{error}"
    );
    let error = compile_tests(&source(
        "fn f<T>(x:T)->T requires T in {bool,str} { when { return x } }\ntest t { eval(f,[1]) }",
    ))
    .unwrap_err();
    assert!(error.contains("requires T in {bool, str}"), "{error}");
}

#[test]
fn production_cannot_see_test_helpers() {
    let input = source(
        "test { fn helper(x: i64) -> i64 { when { return x } } }\nexport fn main() { helper(1) }",
    );
    assert!(hgl_program::compile(&input, "main").is_err());
}

#[test]
fn actual_stdlib_and_harness_regressions_run_on_rust() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let library = root.join("external/hgraph_std/hgl/hgraph");
    let mut parts = fs::read_dir(library.join("tests"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    parts.retain(|p| p.extension().is_some_and(|e| e == "hgl"));
    parts.sort();
    parts.push(root.join("native/stdlib/rust.hgl"));
    parts.push(root.join("external/hgraph_spec/language/examples/contextual-local-bindings.hgl"));
    for fixture in [
        "graph_compound",
        "graph_scalar_local",
        "node_scalar_local",
        "ordinary_widening",
        "wire_rebind",
    ] {
        parts.push(root.join(format!(
            "external/hgraph_spec/compiler/contextual_bindings/{fixture}.hgl"
        )));
    }
    parts.push(root.join("crates/hgl-program/tests/fixtures/contextual_locals.hgl"));
    let sources = hgl_library_files::sources(&parts, &[library])?;
    let summary = expected_summary(&sources)?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-eval-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    manifest(&root, &dir)?;
    standard_batches(&dir, &sources, &summary)?;
    module(
        &dir,
        "regression",
        &emit_tests(&compile_tests(&source(REGRESSIONS))?),
    )?;
    failure_images(&dir)?;
    capability_failure_images(&dir)?;
    source_operator_image(&dir)?;
    replay_order_failure_image(&dir)?;
    nullable_images(&dir)?;
    recording_key_image(&dir)?;
    global_images(&dir)?;
    check_images(&dir)?;
    fs::remove_dir_all(dir)?;
    Ok(())
}
fn standard_batches(
    dir: &Path,
    sources: &[(String, String)],
    expected: &ExpectedSummary,
) -> Result<(), Box<dyn std::error::Error>> {
    let library = hgl_library::load(sources)?;
    let tests = library
        .declarations
        .iter()
        .filter(|decl| decl.role == hgl_library::Role::Test)
        .collect::<Vec<_>>();
    let mut observed = Vec::new();
    let mut evaluations = 0;
    for batch in tests.chunks(8) {
        let selected = batch
            .iter()
            .map(|decl| (decl.source.as_str(), decl.name.as_str()))
            .collect::<std::collections::BTreeSet<_>>();
        let inputs = sources
            .iter()
            .map(|(name, text)| Ok((name.clone(), selected_tests(text, &selected, name)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let summary = expected_summary(&inputs)?;
        module(dir, "standard", &emit_tests(&compile_tests(&inputs)?))?;
        image_main(dir, std::iter::once("standard"))?;
        let output = Command::new(build_binary(dir)?).arg("standard").output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "standard batch: {text}");
        evaluations += check_summary(&text, &summary)?;
        observed.extend(
            text.lines()
                .filter_map(|line| line.strip_suffix(" ... ok"))
                .map(str::to_owned),
        );
    }
    observed.sort();
    assert_eq!(
        observed, expected.names,
        "batched execution must preserve the full named inventory"
    );
    assert!((expected.minimum..=expected.maximum).contains(&evaluations));
    Ok(())
}
fn selected_tests(
    text: &str,
    selected: &std::collections::BTreeSet<(&str, &str)>,
    source: &str,
) -> Result<String, String> {
    let tokens = hgl_source::lex(text)?;
    let mut omitted = Vec::new();
    let mut index = 0;
    while index + 2 < tokens.len() {
        if tokens[index].text != "test" || tokens[index + 1].text == "{" {
            index += 1;
            continue;
        }
        let name = tokens[index + 1].text.as_str();
        let start = tokens[index].span.start;
        index += 2;
        while tokens.get(index).is_some_and(|token| token.text == "\n") {
            index += 1;
        }
        if tokens.get(index).is_none_or(|token| token.text != "{") {
            return Err("named test body missing".into());
        }
        let mut depth = 1;
        index += 1;
        while depth > 0 {
            let token = tokens.get(index).ok_or("unclosed named test")?;
            match token.text.as_str() {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
            index += 1;
        }
        if !selected.contains(&(source, name)) {
            omitted.push(start..tokens[index - 1].span.end);
        }
    }
    let mut filtered = text.to_owned();
    for range in omitted.into_iter().rev() {
        let whitespace = filtered[range.clone()]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect::<String>();
        filtered.replace_range(range, &whitespace);
    }
    Ok(filtered)
}
struct ExpectedSummary {
    names: Vec<String>,
    minimum: usize,
    maximum: usize,
}
fn step_bounds(steps: &[hgl_eval_data::TestStep]) -> (usize, usize) {
    use hgl_eval_data::TestStep;
    steps.iter().fold((0, 0), |(minimum, maximum), step| {
        let (low, high) = match step {
            TestStep::Ordinary(_) => (0, 0),
            TestStep::Eval(_) | TestStep::BindEval(..) | TestStep::Assert(_) => (1, 1),
            TestStep::If(_, yes, no) => {
                let (a, b) = step_bounds(yes);
                let (c, d) = step_bounds(no);
                (a.min(c), b.max(d))
            }
        };
        (minimum + low, maximum + high)
    })
}
fn expected_summary(sources: &[(String, String)]) -> Result<ExpectedSummary, String> {
    let library = hgl_library::load(sources)?;
    let mut summary = ExpectedSummary {
        names: Vec::new(),
        minimum: 0,
        maximum: 0,
    };
    for declaration in library.declarations {
        if declaration.role == hgl_library::Role::Test {
            summary
                .names
                .push(format!("{}::{}", declaration.module, declaration.name));
            let (low, high) = step_bounds(&hgl_eval_data::steps(&declaration.tokens)?);
            summary.minimum += low;
            summary.maximum += high;
        }
    }
    summary.names.sort();
    Ok(summary)
}
fn check_summary(text: &str, summary: &ExpectedSummary) -> Result<usize, String> {
    let mut reported = text
        .lines()
        .filter_map(|line| line.strip_suffix(" ... ok"))
        .collect::<Vec<_>>();
    reported.sort_unstable();
    assert_eq!(
        reported, summary.names,
        "every named test must execute exactly once"
    );
    let prefix = format!("{} tests, ", summary.names.len());
    let evaluations = text
        .lines()
        .find_map(|line| {
            line.strip_prefix(&prefix)?
                .strip_suffix(" evaluations, 0 failures")?
                .parse::<usize>()
                .ok()
        })
        .ok_or("complete test summary missing")?;
    assert!(
        (summary.minimum..=summary.maximum).contains(&evaluations),
        "executed steps {evaluations} outside {}..={}",
        summary.minimum,
        summary.maximum
    );
    Ok(evaluations)
}
fn check_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let images = [
        ("source_operators", true, "0 failures"),
        (
            "replay_order_failure",
            false,
            "generator yield times must strictly increase",
        ),
        ("nullable", true, "0 failures"),
        ("recording_keys", true, "0 failures"),
        ("globals", true, "9 tests, 17 evaluations, 0 failures"),
        ("globals_missing", false, "global_state: missing value"),
        ("bounds", false, "out of bounds"),
        ("past_end", false, "out of bounds"),
        ("repeated_begin", true, "0 failures"),
        ("append_unbegun", false, "global_state: missing value"),
        (
            "duplicate_time",
            false,
            "eval recording timestamps did not advance",
        ),
        ("wrong_time", false, "cycle 0"),
        ("start_failure", false, "deliberate start failure"),
        ("regression", true, "0 failures"),
        ("wrong", false, "cycle 0"),
        ("long", false, "cycle 1"),
        ("short", false, "cycle 0"),
        ("throwing", false, "deliberate node failure"),
        ("modulo_zero", false, "modulo by zero"),
        ("integer_zero", false, "division by zero"),
        ("float_zero", false, "division by zero"),
        (
            "late_output",
            false,
            "cycle 2: expected length 2, observed length 86400000001",
        ),
    ];
    for batch in images.chunks(4) {
        image_main(dir, batch.iter().map(|(name, _, _)| *name))?;
        let binary = build_binary(dir)?;
        for (name, success, message) in batch {
            let output = Command::new(&binary).arg(name).output()?;
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.status.success(), *success, "{name}: {text}");
            assert!(
                text.contains(message),
                "{name}: expected {message:?}: {text}"
            );
        }
    }
    Ok(())
}
fn image_main<'a>(dir: &Path, names: impl Iterator<Item = &'a str>) -> std::io::Result<()> {
    let mut modules = String::new();
    let mut calls = String::new();
    for name in names {
        writeln!(modules, "mod {name};").unwrap_or_else(|_| unreachable!("String formatting"));
        writeln!(calls, "Some({name:?})=>{name}::main(),")
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    fs::write(
        dir.join("src/main.rs"),
        format!(
            "struct Provider;\nmod native {{pub use hgl_std_native::*;}}\n{modules}fn main() {{match std::env::args().nth(1).as_deref() {{{calls}_=>panic!(\"unknown test image\")}}}}"
        ),
    )
}

fn nullable_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut modules = String::new();
    let mut calls = String::new();
    for (i, (body, ty, input, expected)) in [
        ("if null != item { return item }", "i64", "[10,_,12]", "[10,_,12]"),
        ("if null == item {} else { return item }", "i64", "[10,_,12]", "[10,_,12]"),
        ("if !(item == null) { return item }", "i64", "[10,_,12]", "[10,_,12]"),
        ("if item != null && item >= 0 { return item }", "i64", "[0,_,12]", "[0,_,12]"),
        ("if item == null || item < 0 {} else { return item }", "i64", "[0,_,12]", "[0,_,12]"),
        ("let alias = item\nif item != null && alias != null { return item + alias }", "i64", "[10,_,12]", "[20,_,24]"),
        ("let alias = item\nif item != null && alias != null { return item + alias }", "str", "[\"\",_,\"a\"]", "[\"\",_,\"aa\"]"),
        ("let alias = item\nif item == null || alias == null { return }\nreturn item + alias", "i64", "[10,_,12]", "[20,_,24]"),
        ("if item == null { return }\nreturn item", "i64", "[10,_,12]", "[10,_,12]"),
        ("if item == null { if current >= 0 { return } else { return } }\nreturn item", "i64", "[10,_,12]", "[10,_,12]"),
        ("if item != null { let alias = item\nreturn alias }", "i64", "[10,_,12]", "[10,_,12]"),
        ("if item != null { if current >= 0 { let item = replay_input[current]\nif item != null { let copy = item } }\nreturn item }", "i64", "[10,_,12]", "[10,_,12]"),
        ("if item != null && !item { return item }\nif item != null { return item }", "bool", "[false,_,true]", "[false,_,true]"),
        ("if item != null { let alias = item\nif alias == item { return item + alias } }", "str", "[\"\",_,\"a\"]", "[\"\",_,\"aa\"]"),
        ("if item != null { if item != null { return item } }", "str", "[\"\",_,\"a\"]", "[\"\",_,\"a\"]"),
    ].into_iter().enumerate() {
        let mut sources = source(&format!(
            "fn id(x:{ty})->{ty} {{ when {{ return delta_value(x) }} }}\ntest guarded {{ assert eval(id,{input}) == {expected} }}"
        ));
        sources[2].1 = format!("module hgraph.std part replay_record_impl\nimpl fn replay<T>(const values:list<TimedValue<T>>)->T {{ inject replay_input,alarm\nwhen {{let current=0\nlet item=replay_input[current]\n{body}}}}}\ninstantiate replay<{ty}>");
        let Err(error)=compile_tests(&sources) else {return Err("replay requires ordinary present entries".into());};
        assert!(error.contains("unsupported injectable replay_input"),"{body}: {error}");
        let present_body=body.replace("replay_input[current]","delta_value(x)")
            .replace("item != null","true").replace("null != item","true")
            .replace("item == null","false").replace("null == item","false")
            .replace("alias != null","true").replace("alias == null","false");
        let ordinary=source(&format!("fn id(x:{ty})->{ty} {{when {{let current=0\nlet item=delta_value(x)\n{present_body}}}}}\ntest present {{assert eval(id,{input}) == {expected}}}"));
        let generated=emit_tests(&compile_tests(&ordinary)?);
        writeln!(modules, "mod case{i} {{ {} }}", generated.replace("fn main() {", "pub fn main() {"))?;
        writeln!(calls, "case{i}::main();")?;
    }
    write!(modules, "pub fn main() {{ {calls} }}")?;
    fs::write(dir.join("src/nullable.rs"), modules)?;
    Ok(())
}

fn source_operator_image(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut operators = source(
        "fn id(x:i64)->i64 { when { return delta_value(x) } }\ntest source_handlers { assert eval(id,[1,_,1]) == [111,_,111] }",
    );
    operators[2].1 = operators[2]
        .1
        .replace(
            "yield values[index].time: values[index].value",
            "yield values[index].time: values[index].value + 100",
        )
        .replace("value: delta_value(ts)", "value: delta_value(ts) + 10");
    let generated = emit_tests(&compile_tests(&operators)?);
    assert!(generated.contains("hgraph.std::replay"));
    assert!(generated.contains("generator_pending"));
    assert!(generated.contains("append_slot") && generated.contains("commit_append"));
    assert!(!generated.contains("self.replay_input") && !generated.contains("self.capture"));
    assert!(!generated.contains("match self.next"));
    module(dir, "source_operators", &generated)?;
    Ok(())
}

fn replay_order_failure_image(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let fixture =
        include_str!("../../../external/hgraph_std/hgl/hgraph/tests/ordinary_replay_values.hgl");
    assert!(fixture.contains("eval(ordinary_replay_increasing, tick:"));
    let mut sources = source("");
    sources.push((
        "replay_order.hgl".into(),
        fixture.replace(
            "eval(ordinary_replay_increasing, tick:",
            "eval(ordinary_replay_descending, tick:",
        ),
    ));
    module(
        dir,
        "replay_order_failure",
        &emit_tests(&compile_tests(&sources)?),
    )?;
    Ok(())
}

fn build_binary(dir: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["build", "--offline", "--quiet"])
        .current_dir(dir);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let build = command.output()?;
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    Ok(dir
        .join(if cfg!(debug_assertions) {
            "target/debug"
        } else {
            "target/release"
        })
        .join(format!("eval-regressions{}", std::env::consts::EXE_SUFFIX)))
}

fn failure_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (name, expected) in [("wrong", "[2]"), ("long", "[1, _]"), ("short", "[]")] {
        let input = source(&format!(
            "fn id(x: i64) -> i64 {{ when {{ return x }} }}\ntest mismatch {{ assert eval(id, [1]) == {expected} }}"
        ));
        module(dir, name, &emit_tests(&compile_tests(&input)?))?;
    }
    for (name, definition, expected) in [
        (
            "modulo_zero",
            "fn divide(x: i64) -> i64 { when { return 7 % x } }",
            "[0]",
        ),
        (
            "integer_zero",
            "fn divide(x: i64) -> f64 { when { return 3 / x } }",
            "[0]",
        ),
        (
            "float_zero",
            "fn divide(x: f64) -> f64 { when { return 3.0 / x } }",
            "[0.0]",
        ),
        (
            "late_output",
            "fn divide(const delay: duration) -> i64 { inject alarm\nstart { schedule(alarm, delay) }\nwhen { return 7 } }",
            "1d",
        ),
    ] {
        let input = source(&format!(
            "{definition}\ntest mismatch {{ assert eval(divide, {expected}) == [_, _] }}"
        ));
        module(dir, name, &emit_tests(&compile_tests(&input)?))?;
    }
    let throwing = source(
        "native const fn raise_error(message: str) throws\nnative const fn raise_error(message: str) throws {}\nfn fail(ts: i64) -> i64 { when { raise_error(\"deliberate node failure\")\nreturn ts } }\ntest fails { eval(fail, [1]) }",
    );
    module(dir, "throwing", &emit_tests(&compile_tests(&throwing)?))?;
    let missing = source(
        "native const fn raise_error(message:str) throws\nnative const fn raise_error(message:str) throws {}\nfn sink(x:i64) { inject capture\nstart { raise_error(\"start must not run\") }\nwhen {} }\ntest fails { eval(sink,[1]) }",
    );
    let Err(error) = compile_tests(&missing) else {
        return Err("removed capture injectable must fail before start".into());
    };
    assert!(error.contains("unsupported injectable capture"), "{error}");
    let start = source(
        "native const fn raise_error(message:str) throws\nnative const fn raise_error(message:str) throws {}\nfn sink(x:i64) { start { if true { raise_error(\"deliberate start failure\") } }\nwhen {} }\ntest fails { eval(sink,[]) }",
    );
    module(dir, "start_failure", &emit_tests(&compile_tests(&start)?))?;
    Ok(())
}

fn capability_failure_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (name, from, to, input, expected) in [
        (
            "bounds",
            "values[index].value",
            "values[-1].value",
            "[1]",
            "[1]",
        ),
        (
            "past_end",
            "values[index].value",
            "values[len(values)].value",
            "[_,1]",
            "[_,1]",
        ),
        (
            "repeated_begin",
            "set(global_state, key, initial)",
            "set(global_state, key, initial)\nset(global_state, key, initial)",
            "[]",
            "[]",
        ),
        (
            "append_unbegun",
            "set(global_state, key, initial)",
            "let unused=initial",
            "[1]",
            "[1]",
        ),
        (
            "duplicate_time",
            "push(recording, TimedValue<T>(\n            time: clock.evaluation_time,\n            value: delta_value(ts)\n        ))",
            "let entry=TimedValue<T>(time:clock.evaluation_time,value:delta_value(ts))\npush(recording,entry)\npush(recording,entry)",
            "[1]",
            "[1]",
        ),
        (
            "wrong_time",
            "time: clock.evaluation_time",
            "time: clock.next_cycle_evaluation_time",
            "[1]",
            "[1]",
        ),
    ] {
        let mut sources = source(&format!(
            "fn id(x:i64)->i64 {{when {{return delta_value(x)}}}}\ntest fails {{assert eval(id,{input}) == {expected}}}"
        ));
        assert!(sources[2].1.contains(from));
        sources[2].1 = sources[2].1.replace(from, to);
        module(dir, name, &emit_tests(&compile_tests(&sources)?))?;
    }
    Ok(())
}

fn global_images(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut definitions = String::new();
    for (index, (ty, initial, changed)) in [
        ("bool", "false", "true"),
        ("i64", "0", "-7"),
        ("f64", "0.0", "1.5"),
        ("str", "\"\"", "\"changed\""),
        ("date", "@1970-01-01", "@1970-01-02"),
        ("time", "@00:00:00", "@00:00:01"),
        ("datetime", "@1970-01-01T00:00:00Z", "@1970-01-01T00:00:01Z"),
        ("duration", "0us", "1s"),
    ]
    .into_iter()
    .enumerate()
    {
        writeln!(
            definitions,
            r#"
fn stored{index}(value:{ty})->{ty} {{
    inject global_state
    start {{ set(global_state,"shared",{initial}) }}
    when {{
        let previous:{ty}=get(global_state,"shared")
        set(global_state,"shared",delta_value(value))
        return previous
    }}
    stop {{
        let final_value:{ty}=get(global_state,"shared")
        set(global_state,"stopped",final_value)
    }}
}}
test global_scalar{index} {{
    assert eval(stored{index},[{changed},_,{initial}]) == [{initial},_,{changed}]
    assert eval(stored{index},[{changed}]) == [{initial}]
}}
"#
        )?;
    }
    definitions.push_str(
        r#"
fn empty_global(value:i64) {
    inject global_state
    start { set(global_state,"empty",42) }
    when {}
    stop {
        let count:i64=get(global_state,"empty")
        set(global_state,"stopped",count)
    }
}
test global_empty { eval(empty_global,[]) }
"#,
    );
    module(
        dir,
        "globals",
        &emit_tests(&compile_tests(&source(&definitions))?),
    )?;
    let missing = source(
        r#"
fn seed(value:i64)->i64 {
    inject global_state
    start { set(global_state,"previous_run",42) }
    when { return delta_value(value) }
}
fn absent(value:i64)->i64 {
    inject global_state
    when { return get(global_state,"previous_run") }
}
test fresh_global_store {
    assert eval(seed,[1]) == [1]
    eval(absent,[1])
}
"#,
    );
    module(
        dir,
        "globals_missing",
        &emit_tests(&compile_tests(&missing)?),
    )?;
    Ok(())
}

fn module(dir: &Path, name: &str, code: &str) -> std::io::Result<()> {
    fs::write(
        dir.join(format!("src/{name}.rs")),
        code.replace("fn main() {", "pub fn main() {"),
    )
}
fn manifest(root: &Path, dir: &Path) -> std::io::Result<()> {
    let mut lines = vec!["[package]\nname=\"eval-regressions\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]".to_owned()];
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-harness",
        "hgl-harness-ir",
        "hgl-rust-ir",
        "hgl-source",
        "hgl-value-eval",
        "hgl-time-context",
        "hgl-testkit",
        "hgl-std-native",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        lines.push(format!("{name}={{path=\"{path}\"}}"));
    }
    fs::write(dir.join("Cargo.toml"), lines.join("\n"))
}
const REGRESSIONS: &str = r#"
fn clock_current(value:i64)->datetime {
    inject clock
    when { let snapshot = clock.evaluation_time
        if snapshot == clock.evaluation_time { return snapshot } }
}
fn clock_next(value:i64)->datetime {
    inject clock
    when { let next = clock.next_cycle_evaluation_time
        if next > clock.evaluation_time && next == clock.next_cycle_evaluation_time { return next } }
}
fn clock_start_snapshot(value:i64)->datetime {
    inject clock
    cache started:datetime = @1970-01-01T00:00:00Z
    start { started = clock.evaluation_time }
    when { let saved = started
        if clock.evaluation_time >= saved { return saved } }
}
fn clock_start_next(value:i64)->datetime {
    inject clock
    cache next:datetime = @1970-01-01T00:00:00Z
    start { next = clock.next_cycle_evaluation_time }
    when { return next }
}
test clock_property_timestamps {
    assert eval(clock_current,[1,_,1]) == [@1970-01-01T00:00:00.000001Z,_,@1970-01-01T00:00:00.000003Z]
    assert eval(clock_next,[1,_,1]) == [@1970-01-01T00:00:00.000002Z,_,@1970-01-01T00:00:00.000004Z]
    assert eval(clock_start_snapshot,[1,1,1]) == [@1970-01-01T00:00:00.000001Z,@1970-01-01T00:00:00.000001Z,@1970-01-01T00:00:00.000001Z]
    assert eval(clock_start_next,[1,1,1]) == [@1970-01-01T00:00:00.000002Z,@1970-01-01T00:00:00.000002Z,@1970-01-01T00:00:00.000002Z]
}

fn explicit_delta(a:i64,b:i64)->i64 {
    when valid(a) && modified(a) { return delta_value(a) }
    when valid(b) && modified(b) { return delta_value(b) }
}
fn guarded_delta(a:i64)->i64 { when valid(a) { return delta_value(a) } }
fn modified_delta(a:i64,b:i64)->i64 { when modified(a) { return delta_value(a) } }
fn empty_selectors(a:i64)->i64 { when valid() && modified() { return delta_value(a) } }
fn nested_delta(a:i64,b:i64)->i64 {
    when { if valid(a) && modified(a) && delta_value(a) > 0 { return delta_value(a) } }
}
test endpoint_delta_admission {
    assert eval(guarded_delta,[1,_,2]) == [1,_,2]
    assert eval(modified_delta,[1,_,3],[_,2,_]) == [_,_,3]
    assert eval(empty_selectors,[1,_,2]) == [1,_,2]
    assert eval(explicit_delta,[1,_,3],[_,2,_]) == [1,2,3]
    assert eval(nested_delta,[1,_,3],[1,2,_]) == [1,_,3]
}
fn plus(a: i64, b: i64) -> i64 { when { return a + b } }
fn minus(a: i64, b: i64) -> i64 { when { return a - b } }
fn times(a: i64, b: i64) -> i64 { when { return a * b } }
fn modulo(a: i64, b: i64) -> i64 { when { return a % b } }
fn negate(a: i64) -> i64 { when { return -a } }
fn wrap_state(ts: i64) -> i64 {
    state total: i64 = 9223372036854775807
    when { total += ts
        return total }
}
test integer_boundaries {
    assert eval(plus, [9223372036854775807, -9223372036854775808], [1, -1]) == [-9223372036854775808, 9223372036854775807]
    assert eval(minus, [-9223372036854775808, 9223372036854775807], [1, -1]) == [9223372036854775807, -9223372036854775808]
    assert eval(times, [9223372036854775807, -9223372036854775808], [2, -1]) == [-2, -9223372036854775808]
    assert eval(modulo, [-7, 7, -7, -9223372036854775808], [2, -2, -2, -1]) == [1, -1, -1, 0]
    assert eval(negate, [-9223372036854775808]) == [-9223372036854775808]
    assert eval(wrap_state, [1, -1]) == [-9223372036854775808, 9223372036854775807]
}

fn capture(ts: ref<i64>) -> ref<i64> { when { return ts } }
fn consume(ts: i64) -> i64 { when { return ts } }
fn generic_consume<T>(ts: T) -> T { when { return ts } }
fn follow_direct(ts: i64) -> i64 => consume(capture(ts))
fn follow_generic(ts: i64) -> i64 => generic_consume(capture(ts))
fn ref_passthrough(ts: ref<i64>) -> ref<i64> { when { return ts } }
fn follow_twice(ts: i64) -> i64 => consume(ref_passthrough(capture(ts)))
fn choose_reference(lhs: ref<i64>, rhs: ref<i64>, choice: bool) -> ref<i64> {
    when { if choice { return lhs } else { return rhs } }
}
fn signal_metadata(ts: signal) -> i64 {
    state count: i64 = 0
    when valid(ts) && modified(ts) { count += 1
        return count }
}
test signal_selector_unchanged {
    assert eval(signal_metadata, [1, _, 2]) == [1, _, 2]
    assert eval(signal_metadata, [false, _, true]) == [1, _, 2]
    assert eval(signal_metadata, ["a", _, "b"]) == [1, _, 2]
}
fn signal_count(ts: signal) -> i64 {
    state count: i64 = 0
    when { count += 1
        return count }
}
fn reference_signal(ts: i64) -> i64 => signal_count(capture(ts))
fn follow_rebind(lhs: i64, rhs: i64, choice: bool) -> i64 => consume(choose_reference(lhs, rhs, choice))
test reference_arguments_follow_the_target {
    assert eval(reference_signal, [_, 1, _, 2]) == [1, _, _, _]
    assert eval(follow_direct, [_, 1, _, 2, 2]) == [_, 1, _, 2, 2]
    assert eval(follow_generic, [_, 1, _, 2]) == [_, 1, _, 2]
    assert eval(follow_twice, [_, 1, _, 2]) == [_, 1, _, 2]
    assert eval(follow_rebind, [1, 2, _, _, 3], [10, _, 20, 30, _], [true, _, false, _, true]) == [1, 2, 20, 30, 3]
}
fn divide(a: i64, b: i64) -> f64 { when { return a / b } }
fn divide_mixed(a: i64, b: f64) -> f64 { when { return a / b } }
fn divide_float(a: f64, b: f64) -> f64 { when { return a / b } }
test true_division {
    assert eval(divide, [3, -3, 3, -9223372036854775808], [2, 2, -2, -1]) == [1.5, -1.5, -1.5, 9.223372036854776e18]
    assert eval(divide_mixed, [3, -3], [2.0, 2.0]) == [1.5, -1.5]
    assert eval(divide_float, [3.0, -3.0], [2.0, 2.0]) == [1.5, -1.5]
}
fn id(x: i64) -> i64 { when { return x } }
fn floating(x: f64) -> f64 { when { return x } }
fn counter(x: i64) -> i64 {
    state count: i64 = 0
    when { count += 1
        return count }
}
fn delayed(const value: i64, const delay: duration) -> i64 {
    inject alarm
    start { schedule(alarm, delay) }
    when { return value }
}
fn sink(x: i64) { when {} }
fn production(x: i64) -> i64 { return id(x) }
test {
    fn id(x: i64) -> i64 { when { return x + 10 } }
    fn forward(x: i64) -> i64 { return helper(x) }
}
test {
    fn helper(x: i64) -> i64 { when { return x } }
}
test silent { assert eval(helper, [_, _, _]) == [_, _, _] }
test empty { assert eval(helper, []) == [] }
test same_value_ticks { assert eval(helper, [1, _, 1, _]) == [1, _, 1, _] }
test promoted { assert eval(floating, [1, _, 2.5]) == [1, _, 2.5] }
test state_is_fresh {
    assert eval(counter, [1, 1, _, 1]) == [1, 2, _, 3]
    assert eval(counter, [1]) == [1]
}
test output_horizon { assert eval(delayed, 7, 3us) == [_, _, _, 7] }
test fixed_expression { assert eval(delayed, 2 + 3, 1us + 1us) == [_, _, 5] }
test helper_shadow { assert eval(id, [1]) == [11] }
test production_scope { assert eval(production, [1]) == [1] }
test forward_scope { assert eval(forward, [1]) == [1] }
test outputless { eval(sink, [1, _, 1]) }
fn text_state(ts: str) -> str {
    state saved: str = "initial"
    when { saved = ts
        return saved }
}
fn text_dedup(ts: str) -> str {
    inject out
    when { if !valid(out) || ts != out { return ts } }
}
test text_values { assert eval(text_state, ["a", _, "b"]) == ["a", _, "b"]
    assert eval(text_dedup, ["a", "a", _, "b"]) == ["a", _, _, "b"] }

"#;

fn recording_key_image(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let collision = source(include_str!("fixtures/eval_recording_keys.hgl"));
    module(
        dir,
        "recording_keys",
        &emit_tests(&compile_tests(&collision)?),
    )?;

    Ok(())
}
