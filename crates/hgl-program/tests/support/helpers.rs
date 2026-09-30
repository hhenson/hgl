use std::path::{Path, PathBuf};

pub(crate) fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub(crate) fn sources(main: &str) -> Vec<(String, String)> {
    let root = root();
    let mut out = vec![
        ("main.hgl".into(), main.into()),
        (
            "rust.hgl".into(),
            std::fs::read_to_string(root.join("examples/stdlib-const-debug/rust.hgl")).unwrap(),
        ),
    ];
    collect(&root.join("external/hgraph_std/hgl/hgraph"), &mut out);
    out
}
fn collect(path: &Path, out: &mut Vec<(String, String)>) {
    let mut entries = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().unwrap() != "tests" {
                collect(&p, out);
            }
        } else if p.extension().is_some_and(|s| s == "hgl") {
            out.push((p.display().to_string(), std::fs::read_to_string(p).unwrap()));
        }
    }
}
pub(crate) fn main_source(body: &str) -> String {
    format!(
        "module example\nuse hgraph.std::{{const, debug_print}}\nexport fn main() {{\n{body}\n}}\n"
    )
}
