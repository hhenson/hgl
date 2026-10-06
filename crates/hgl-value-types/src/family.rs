use crate::substitute;
use hgl_inheritance::Pattern;
use hgl_library::Library;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
/// Resolve one inherited field in its declaration scope.
pub fn field_type(
    library: &Library,
    pattern: &Pattern,
    bindings: &BTreeMap<String, Ty>,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    hgl_family_types::field_type(pattern, bindings, |module, name, bindings| {
        substitute(library, module, name, bindings, active)
    })
}
