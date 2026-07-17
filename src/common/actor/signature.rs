use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignatureKey(pub uuid::Uuid);

pub trait Signature: std::fmt::Display {
    fn type_name(&self) -> &'static str;

    fn id(&self) -> uuid::Uuid;

    fn key(&self) -> SignatureKey {
        SignatureKey(self.id())
    }
}
