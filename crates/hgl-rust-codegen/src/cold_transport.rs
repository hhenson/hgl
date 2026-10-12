//! Cold transport reuses typed owning codecs, with inert descriptor identities in bytes.
use hgl_source::Ty;

/// Encode one complete prepared owner after the existing codec's fallible width pass.
pub fn encode(ty: &Ty, value: &str) -> String {
    let name = crate::any_values::name(ty);
    format!(
        "{{let value={value};let size=any_size_{name}(&value)?;let mut out=ColdBuffer::new(size,registry);any_write_{name}(&value,&mut out);out.finish()?}}"
    )
}
/// Resolve inert descriptors and decode through the exact compiled ordinary type.
pub fn decode(ty: &Ty, bytes: &str, registry: &str) -> String {
    let name = crate::any_values::name(ty);
    format!(
        "{{let bytes={bytes};let (data,descriptors)=cold_parts(&bytes,{registry})?;let data=hgl_types::any_values::AnyView::from_encoded(data,&descriptors);any_validate_{name}(data)?;any_read_{name}(data)?}}"
    )
}
/// Emit cold-only framing support; no callbacks or provider handles enter descriptions.
pub fn support() -> String {
    r#"
struct ColdBuffer<'a> {registry:&'a hgl_describe::Registry,bytes:Vec<u8>,descriptors:Vec<u64>,error:Option<Box<hgl_types::NodeError>>}
impl<'a> ColdBuffer<'a> {
 fn new(size:usize,registry:&'a hgl_describe::Registry)->Self {Self {registry,bytes:Vec::with_capacity(size),descriptors:Vec::new(),error:None}}
 fn finish(self)->hgl_types::NodeResult<Vec<u8>> {
  if let Some(error)=self.error {return Err(error)}
  let mut bytes=Vec::new();bytes.extend_from_slice(&(self.descriptors.len() as u64).to_le_bytes());
  for id in self.descriptors {bytes.extend_from_slice(&id.to_le_bytes());}
  bytes.extend(self.bytes);Ok(bytes)
 }
}
impl hgl_types::any_values::AnyBuffer for ColdBuffer<'_> {
 fn len(&self)->usize {self.bytes.len()}
 fn write(&mut self,bytes:&[u8]) {self.bytes.extend_from_slice(bytes);}
 fn native(&mut self,data:&[u8],copy:hgl_types::any_values::NativeCopy,_:hgl_types::any_values::NativeCheck) {copy(data,self);}
 fn width(&mut self,index:usize,width:usize) {self.bytes[index..index+8].copy_from_slice(&(width as u64).to_le_bytes());}
 fn descriptor(&mut self,plan:Option<&'static hgl_types::AnyPlan>) {
  let id=if let Some(plan)=plan {
   match self.registry.retain_descriptor(plan) {Ok(id)=>{self.descriptors.push(id);self.descriptors.len() as u64},Err(error)=>{self.error=Some(error);0}}
  } else {0};
  self.bytes.extend_from_slice(&id.to_le_bytes());
 }
}
fn cold_parts<'a>(bytes:&'a [u8],registry:&hgl_describe::Registry)->hgl_types::NodeResult<(&'a [u8],Vec<&'static hgl_types::AnyPlan>)> {
 let error=||hgl_types::NodeError::new("invalid cold descriptor frame");
 let number=|bytes:&[u8]|->hgl_types::NodeResult<usize>{usize::try_from(u64::from_le_bytes(bytes.try_into().map_err(|_|error())?)).map_err(|_|error())};
 let count=number(bytes.get(..8).ok_or_else(error)?)?;
 let end=count.checked_mul(8).and_then(|n|n.checked_add(8)).ok_or_else(error)?;
 let header=bytes.get(8..end).ok_or_else(error)?;
 let descriptors=header.chunks_exact(8).map(|id|registry.descriptor(u64::from_le_bytes(id.try_into().map_err(|_|error())?))).collect::<hgl_types::NodeResult<Vec<_>>>()?;
 Ok((&bytes[end..],descriptors))
}
"#.into()
}
