use crate::inheritance::Pattern;
use crate::library::Library;
use crate::value_types::substitute;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
/// Resolve one inherited field in its declaration scope.
pub fn field_type(
    library: &Library,
    pattern: &Pattern,
    bindings: &BTreeMap<String, Ty>,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    crate::family_types::field_type(pattern, bindings, |module, name, bindings| {
        Ok(substitute(library, module, name, bindings, active)?)
    })
}
