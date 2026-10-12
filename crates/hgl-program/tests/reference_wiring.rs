//! Wiring projections preserve reference identity without granting node read-through.
use hgl_program::{compile, emit_rust};
use std::{fmt::Write as _, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

fn checked(body: &str, signature: &str, wrapper: &str) -> Result<String, String> {
    let source = format!(
        "module references\nstruct Pair {{left:i64\nright:i64}}\nfn source()->Pair {{yield 0us:delta<Pair>(left:7)}}\nfn project(value:{signature})->ref<i64> {{ {body} }}\nfn main()->ref<i64> => project({wrapper})"
    );
    compile(&[("references.hgl".into(), source)], "main").map(|program| emit_rust(&program))
}

#[test]
fn graph_reference_and_ordinary_nominal_projections_capture_child_identity() {
    for signature in ["Pair", "ref<Pair>"] {
        let code = checked("return value.left", signature, "source()").unwrap();
        assert!(code.contains("hgl.projection"));
        assert!(code.contains("input_reference("));
        assert!(code.contains("set_reference(self._output, publication)"));
    }
}

#[test]
fn graph_reference_projection_checks_field_and_fixed_index() {
    let error = checked("return value.unknown", "ref<Pair>", "source()").unwrap_err();
    assert!(error.contains("unknown struct field unknown"), "{error}");
    for index in ["-1", "2", "true"] {
        let source = format!(
            "module references\nfn source()->list<i64,2> {{yield 0us:delta<list<i64,2>>(items:[0:7])}}\nfn project(value:ref<list<i64,2>>)->ref<i64> => value[{index}]\nfn main()->ref<i64> => project(source())"
        );
        let error = compile(&[("references.hgl".into(), source)], "main").unwrap_err();
        assert!(error.contains("graph projection"), "{index}: {error}");
    }
}

#[test]
fn node_reference_traversal_stays_opaque() {
    let error = checked("when {return value.left}", "ref<Pair>", "source()").unwrap_err();
    assert!(
        error.contains("field access requires an ordinary struct"),
        "{error}"
    );
}

#[test]
fn reference_output_metadata_uses_designation_publication_time() {
    let source = "module references\nfn source()->i64 {yield 0us:7}\nfn hold(value:ref<i64>)->ref<i64> {inject out\nwhen {let changed=modified(out)\nlet time=last_modified(out)\nif !valid(out) {return value}}}\nfn main()->ref<i64> => hold(source())";
    let program = compile(&[("references.hgl".into(), source.into())], "main").unwrap();
    let code = emit_rust(&program);
    assert!(code.contains("output(self._output).modified_at != hgl_types::EngineTime::NEVER"));
    assert!(code.contains("output_modified(self._output,_ctx.evaluation_time())"));
}

#[test]
fn reference_all_valid_uses_designation_validity_for_input_and_output() {
    for endpoint in ["out", "value"] {
        let source = format!(
            "module references\nfn source()->i64 {{yield 0us:7}}\nfn observe(value:ref<i64>)->ref<i64> {{inject out\nwhen {{let ready=all_valid({endpoint})\nreturn value}}}}\nfn main()->ref<i64> => observe(source())"
        );
        let sources = [("references.hgl".into(), source)];
        hgl_program::check_sources(&sources).unwrap();
        let program = compile(&sources, "main").unwrap();
        let code = emit_rust(&program);
        let observation = if endpoint == "out" {
            "output(self._output).modified_at != hgl_types::EngineTime::NEVER"
        } else {
            "input_valid(self.input0)"
        };
        assert!(code.contains(observation), "{endpoint}: {code}");
    }
}

const REFERENCE_VALIDITY_SOURCE: &str = r"
module references
fn choose(index:i64,first:ref<i64>,second:ref<i64>)->ref<i64> {
 inject out
 when {
  if all_valid(out) != valid(out) {return first}
  if index == 0 {return first}
  return second
 }
}
fn observe(reference:ref<i64>,data:i64,step:i64)->atomic<tuple<bool,bool,bool,i64>> {
 when valid(step) && modified(step) {
  var observed:i64 = -1
  if valid(data) {observed = data}
  return (all_valid(reference),valid(reference),valid(data),observed)
 }
}
fn routed(index:i64,first:i64,second:i64,step:i64)->atomic<tuple<bool,bool,bool,i64>> {
 let selected = choose(index,first,second)
 observe(selected,selected,step)
}
test designation_validity {
 assert eval(routed,index:[_,0,1,1,_,0],first:[7,_,_,_,_,9],second:[_,_,_,_,20],step:[1,1,1,1,1,1]) == [
  (false,false,false,-1),(true,true,true,7),(true,true,false,-1),
  (true,true,false,-1),(true,true,true,20),(true,true,true,9)]
}
";

#[test]
fn reference_all_valid_does_not_observe_invalid_target_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        ("references.hgl".into(), REFERENCE_VALIDITY_SOURCE.into()),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ];
    let suite = hgl_program::compile_module_suite(&sources)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let project = regression_cache::open(&root, "reference-all-valid")?;
    let directory = &project.path;
    let code = format!(
        "struct Provider;mod native {{pub use hgl_stdlib::std_native::*;}}\n{}",
        hgl_program::emit_tests(&suite)
    );
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let mut manifest = String::from(
        "[package]\nname=\"reference-all-valid\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-stdlib",
    ] {
        writeln!(
            manifest,
            "{name}={{path={:?}}}",
            root.join("crates").join(name).to_string_lossy()
        )?;
    }
    generated_cache::write(directory.join("Cargo.toml"), manifest)?;
    generated_cache::workspace_lock(&root, directory)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(directory);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("1 executed tests, 1 evaluations, 0 failures")
    );
    Ok(())
}
