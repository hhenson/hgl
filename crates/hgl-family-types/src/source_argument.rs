use hgl_library::{Decl, Library};
use hgl_source::Ty;
/// Restore exact source arguments before inherited or direct generic inference.
pub fn source_argument(
    library: &Library,
    ty: &Ty,
    specialize: &mut impl FnMut(&Decl, Vec<Ty>) -> Result<Ty, String>,
) -> Result<Ty, String> {
    if let Ok((identity, _, _)) = ty.structure() {
        let missing = "unresolved nominal source argument";
        let (module, name) = identity.origin.rsplit_once("::").ok_or(missing)?;
        let decl = hgl_struct_names::declaration(library, module, name)?.ok_or(missing)?;
        return specialize(decl, identity.arguments.clone());
    }
    if let Ty::List(child, size) = ty {
        return Ok(Ty::List(
            Box::new(source_argument(library, child, specialize)?),
            *size,
        ));
    }
    if let Ty::Tuple(children) = ty {
        return Ok(Ty::Tuple(
            children
                .iter()
                .map(|child| source_argument(library, child, specialize))
                .collect::<Result<_, _>>()?,
        ));
    }
    if let Ty::Set(child) = ty {
        return Ok(Ty::Set(Box::new(source_argument(
            library, child, specialize,
        )?)));
    }
    if let Ty::Map(key, child) = ty {
        return Ok(Ty::Map(
            Box::new(source_argument(library, key, specialize)?),
            Box::new(source_argument(library, child, specialize)?),
        ));
    }
    Ok(ty.clone())
}
