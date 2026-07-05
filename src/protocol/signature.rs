pub trait Signature {
    fn id(&self) -> uuid::Uuid;
}
