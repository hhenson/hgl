//! Native type storage selects its image independently of semantic helper calls.
use std::{fmt::Write as _, path::Path, process::Command};
#[path = "support/generated_cache.rs"]
mod generated_cache;
#[path = "support/regression_cache.rs"]
mod regression_cache;

fn root() -> Result<std::path::PathBuf, std::io::Error> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
}

fn sources(body: &str) -> Result<Vec<(String, String)>, String> {
    let root = root().map_err(|error| error.to_string())?;
    let mut sources = hgl_program::library_files::sources(
        &[
            root.join("external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl"),
            root.join("native/stdlib/atomic_values.hgl-native"),
            root.join("external/hgraph_std/hgl/hgraph/replay_record.hgl"),
            root.join("external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
        ],
        &[],
    )?;
    sources.insert(0, ("native-type-images.hgl".into(), body.into()));
    Ok(sources)
}

#[test]
fn native_type_storage_without_helpers_executes_in_prepared_suites()
-> Result<(), Box<dyn std::error::Error>> {
    let sources = sources(include_str!("fixtures/native_type_images.hgl"))?;
    let mut suite = hgl_program::compile_module_suite(&sources)?;
    assert!(suite.plans.iter().all(|plan| plan.natives.is_empty()));
    hgl_semantics::ir_visit::harness(&suite.tests, &mut |value| {
        assert!(!matches!(value.kind, hgl_semantics::ir::Kind::Native(..)));
        true
    });
    let generated = hgl_program::emit_tests(&suite);
    assert_eq!(generated.matches("#[path=").count(), 1);
    assert_eq!(generated.matches("type NativeScalar").count(), 2);
    assert!(execute(&generated)?.status.success());
    suite.select(&["native_type_ordinary_any_without_helpers"])?;
    let generated = hgl_program::emit_tests(&suite);
    assert!(suite.plans.is_empty());
    assert_eq!(generated.matches("type NativeScalar").count(), 1);
    assert!(!generated.contains("::TokenOther;"));
    assert!(execute(&generated)?.status.success());
    Ok(())
}

#[test]
fn standalone_empty_native_configuration_runs_without_helper_bindings()
-> Result<(), Box<dyn std::error::Error>> {
    let sources = sources(
        "module native_type_images\nuse hgraph.std::{Token}\nfn source()->i64 {yield 0us:1}\nfn consume(value:i64,const values:list<Token>)->i64 {when{return value+len(values)}}\nexport fn main()->i64 {let values:list<Token> =[]\nconsume(source(),values)}",
    )?;
    let program = hgl_program::compile(&sources, "main")?;
    let mut code = hgl_program::emit_rust(&program);
    assert!(code.contains("::Token;"));
    assert!(!code.contains("::TokenOther;"));
    code.push_str("fn main(){let mut registry=hgl_describe::Registry::new();register(&mut registry).unwrap();let description=main_graph(&registry).unwrap();let mut store=hgl_store::Store::new();let mut built=hgl_describe::instantiate_complete(&description,&registry,&mut store).unwrap();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig {start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END}).unwrap();}");
    code = code.replacen("pub fn main(", "pub fn main_graph(", 1);
    assert!(execute(&code)?.status.success());
    Ok(())
}

#[test]
fn type_only_aliases_and_modules_share_one_provider_image() -> Result<(), Box<dyn std::error::Error>>
{
    let mut sources = sources(
        "module native_type_images\nuse hgraph.std::{Token}\nuse aliases::{Alias}\nuse other::{Other}\ntest aliases {let original:list<Token> =[]\nlet same:list<Alias> =original\nlet separate:list<Other> =[]\nassert len(same)+len(separate)==0}",
    )?;
    let image = root()?.join("native/stdlib/atomic_values.rs");
    for (module, name, canonical, marker) in [
        ("aliases", "Alias", "hgraph.std::Token", "Token"),
        ("other", "Other", "hgraph.std::TokenOther", "TokenOther"),
    ] {
        sources.push((
            format!("{module}.hgl"),
            format!("module {module}\nexport native type {name}\n"),
        ));
        sources.push((format!("{module}.hgl-native"),format!("native_descriptor 1 target \"hgl-rust-1\"\nmodule {module}\nprovider \"hgl.native_atomic_fixture\" fingerprint \"native-atomic-values-v1\" image {:?}\nscalar {name} canonical \"{canonical}\" capabilities {{copy,text,equality,hash,order,serialization}} marker \"{marker}\"\n",image.to_string_lossy())));
    }
    let suite = hgl_program::compile_module_suite(&sources)?;
    assert!(suite.plans.is_empty());
    let generated = hgl_program::emit_tests(&suite);
    assert_eq!(generated.matches("#[path=").count(), 1);
    assert_eq!(generated.matches("type NativeScalar").count(), 2);
    execute(&generated)?;
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(name, _)| name == "aliases.hgl-native")
        .ok_or("missing alias descriptor")?;
    *descriptor = descriptor.replace("marker \"Token\"", "marker \"TokenOther\"");
    assert!(hgl_program::compile_module_suite(&sources).is_err());
    Ok(())
}

#[test]
fn type_only_provider_fingerprint_is_checked_before_execution()
-> Result<(), Box<dyn std::error::Error>> {
    let mut sources = sources(
        "module native_type_images\nuse hgraph.std::{Token}\ntest only {let values:list<Token> =[]\nassert len(values)==0}",
    )?;
    let (_, descriptor) = sources
        .iter_mut()
        .find(|(name, _)| name.ends_with(".hgl-native"))
        .ok_or("missing native descriptor")?;
    *descriptor = descriptor.replace("native-atomic-values-v1", "wrong-image-v1");
    let suite = hgl_program::compile_module_suite(&sources)?;
    let output = execute_expected(&hgl_program::emit_tests(&suite), false)?;
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("native descriptor/provider identity or fingerprint mismatch")
    );
    Ok(())
}

#[test]
fn selected_helper_images_cover_canonical_types_beside_unmapped_aliases()
-> Result<(), Box<dyn std::error::Error>> {
    for helper in [false, true] {
        let ordinary = if helper {
            "let token_value=token(\"a\")\nassert any(token_value)==any(token_value)"
        } else {
            ""
        };
        let mut sources = sources(&format!(
            "module native_type_images\nuse hgraph.std::{{token}}\nuse aaa_aliases::{{Alias}}\ntest selected {{let empty:list<Alias> =[]\nassert len(empty)==0\n{ordinary}}}"
        ))?;
        sources.push((
            "aaa_aliases.hgl".into(),
            "module aaa_aliases\nexport native type Alias\n".into(),
        ));
        sources.push(("aaa_aliases.hgl-native".into(),"native_descriptor 1 target \"hgl-rust-1\"\nmodule aaa_aliases\nprovider \"portable-alias\" fingerprint \"portable-v1\"\nscalar Alias canonical \"hgraph.std::Token\" capabilities {copy,text,equality,hash,order,serialization}\n".into()));
        let suite = hgl_program::compile_module_suite(&sources)?;
        assert_eq!(suite.native_images.len(), 1);
        assert_eq!(suite.native_images[0].scalars[0].1, "Token");
        let generated = hgl_program::emit_tests(&suite);
        assert_eq!(generated.matches("#[path=").count(), 1);
        assert_eq!(generated.matches("type NativeScalar").count(), 1);
        assert!(!generated.contains("portable-alias"));
        execute(&generated)?;
    }
    Ok(())
}

#[test]
fn unused_native_images_and_unmapped_types_stay_unlinked() -> Result<(), Box<dyn std::error::Error>>
{
    let mut sources = sources(
        "module native_type_images\nuse hgraph.std::{Token}\nuse unlinked::{Never}\ntest only {let values:list<Token> =[]\nassert len(values)==0}\ntest dropped {let values:list<Never> =[]\nassert len(values)==0}",
    )?;
    sources.push((
        "unused.hgl".into(),
        "module unused\nexport native type Never\n".into(),
    ));
    sources.push(("unused.hgl-native".into(),"native_descriptor 1 target \"hgl-rust-1\"\nmodule unused\nprovider \"unused-provider\" fingerprint \"unused-v1\"\nscalar Never canonical \"unused::Never\" capabilities {copy,text}\n".into()));
    sources.push((
        "unlinked.hgl".into(),
        "module unlinked\nexport native type Never\n".into(),
    ));
    sources.push(("unlinked.hgl-native".into(),"native_descriptor 1 target \"hgl-rust-1\"\nmodule unlinked\nprovider \"unlinked-provider\" fingerprint \"unlinked-v1\" image \"absent-provider.rs\"\nscalar Never canonical \"unlinked::Never\" capabilities {copy,text} marker \"Never\"\n".into()));
    let mut suite = hgl_program::compile_module_suite(&sources)?;
    suite.select(&["only"])?;
    let generated = hgl_program::emit_tests(&suite);
    assert!(!generated.contains("unused-provider"));
    assert!(!generated.contains("absent-provider.rs"));
    assert!(execute(&generated)?.status.success());
    Ok(())
}

fn execute(code: &str) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    execute_expected(code, true)
}

fn execute_expected(
    code: &str,
    success: bool,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let root = root()?;
    let project = regression_cache::open(&root, "native-type-images")?;
    let directory = &project.path;
    let code = code.replace("hgl_kernel::run_simulation(", "crate::measured_simulation(") + RUNTIME;
    generated_cache::write(directory.join("src/main.rs"), code)?;
    let package = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid generated project name")?;
    let mut manifest = format!(
        "[package]\nname={package:?}\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n"
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-alloc-count",
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
        output.status.success() == success,
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

const RUNTIME: &str = r#"
#[global_allocator] static ALLOCATOR:hgl_alloc_count::CountingAllocator=hgl_alloc_count::CountingAllocator;
fn measured_simulation(graph:&mut hgl_kernel::Graph,store:&mut hgl_store::Store,config:&hgl_kernel::RunConfig)->Result<u64,hgl_kernel::EngineError>{
    graph.start(store,config.start_time).map_err(hgl_kernel::EngineError::Node)?;
    let mut cycles=0;let mut total=0;let mut now=config.start_time;let mut outcome=Ok(());
    while outcome.is_ok()&&!graph.stop_requested(){let next=graph.next_scheduled_time();if next>=config.end_time{break;}now=next;
        let(result,count)=hgl_alloc_count::count_in(||graph.evaluate(store,now));
        if result.is_ok(){total+=count;}outcome=result;cycles+=1;
    }
    let stop=graph.stop(store,now);outcome=hgl_types::finish(outcome,stop);
    assert_eq!(total,0,"native type-only successful ticks allocated");
    outcome.map_err(hgl_kernel::EngineError::Node)?;Ok(cycles)
}
"#;
