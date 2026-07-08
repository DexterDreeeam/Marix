use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignatureKey(pub uuid::Uuid);

pub trait Signature {
    fn id(&self) -> uuid::Uuid;

    fn key(&self) -> SignatureKey {
        SignatureKey(self.id())
    }
}
