//! Cold key identities use provider equality and hash without inventing native ordering.
use crate::{
    composite_keys::{Key, known_with},
    ir::{Kind, Value},
    value_eval::EvalError,
};
use hgl_value_model::native_value::NativeValue;
/// Independent cold operation scope; identifiers never escape into source or runtime values.
#[derive(Debug, Default)]
pub struct Context {
    native: std::collections::HashMap<(String, u64), Vec<(usize, NativeValue)>>,
    next: usize,
}
impl Context {
    /// Materialize a complete key, retaining each new provider identity independently.
    pub fn key(&mut self, value: &Value) -> Result<Key, EvalError> {
        known_with(value, &mut |value| {
            let Kind::Opaque(data) = &value.kind else {
                unreachable!("opaque key")
            };
            let hash = (data.operations().hash)(data.data())?;
            let bucket = self
                .native
                .entry((value.ty.source_name(), hash))
                .or_default();
            for (id, prior) in bucket.iter() {
                if (prior.operations().equal)(prior.data(), data.data())? {
                    return Ok(Some(Key::Native(*id)));
                }
            }
            let id = self.next;
            self.next += 1;
            bucket.push((id, data.clone()));
            Ok(Some(Key::Native(id)))
        })?
        .ok_or_else(|| {
            EvalError::Unsupported("collection key requires cold materialization".into())
        })
    }
}
