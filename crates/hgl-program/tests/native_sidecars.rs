//! Source bundles and normal library discovery share the same inert native contracts.
use std::path::PathBuf;

#[test]
fn descriptor_discovery_and_suite_api_share_identical_checked_data() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib");
    let files = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl"),
    ];
    let sources = hgl_program::library_files::sources(&files, &[root])?;
    assert_eq!(
        sources
            .iter()
            .filter(|(source, _)| source.ends_with(".hgl-native"))
            .count(),
        1
    );
    assert!(
        sources
            .iter()
            .any(|(source, _)| source.ends_with(".hgl-native"))
    );
    let suite = hgl_program::compile_suite(&sources)?;
    assert!(suite.tests.is_empty());
    assert!(suite.plans.is_empty());
    Ok(())
}

#[test]
fn missing_sidecar_rejects_the_same_authored_interface_before_emission() {
    let interface =
        include_str!("../../../external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl");
    let sources = [("fixture.hgl".into(), interface.into())];
    let error = hgl_program::compile_suite(&sources).err();
    assert!(
        error.is_some_and(|message| message.contains("missing native scalar provider descriptor"))
    );
}
