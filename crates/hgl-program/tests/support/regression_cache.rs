//! Invocation slots stay stable across test scheduling and source revisions.
use super::generated_cache::Project;
use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    io,
    path::Path,
    sync::Mutex,
};

static CALLS: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

pub(super) fn open(root: &Path, group: &str) -> io::Result<Project> {
    let thread = std::thread::current();
    let test = thread
        .name()
        .ok_or_else(|| io::Error::other("generated regression requires a named test thread"))?;
    let key = format!("{group}:{test}");
    let index = {
        let mut calls = CALLS
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let next = calls.entry(key.clone()).or_default();
        let index = *next;
        *next += 1;
        index
    };
    let mut hash = DefaultHasher::new();
    key.hash(&mut hash);
    Project::open(root, &format!("{group}-{:016x}-{index:03}", hash.finish()))
}
