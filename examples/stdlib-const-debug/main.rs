mod generated;
include!("native.rs");

fn main() -> Result<(), String> {
    let mut registry = hgl_describe::Registry::new();
    generated::register(&mut registry).map_err(|e| format!("{e:?}"))?;
    let description = generated::main(&registry).map_err(|e| format!("{e:?}"))?;
    let mut store = hgl_store::Store::new();
    let mut graph = hgl_describe::instantiate(&description, &registry, &mut store)
        .map_err(|e| format!("{e:?}"))?;
    let config = hgl_kernel::RunConfig {
        start_time: hgl_types::EngineTime::MIN_START,
        end_time: hgl_types::EngineTime::MAX_END,
    };
    hgl_kernel::run_simulation(&mut graph, &mut store, &config)
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}
