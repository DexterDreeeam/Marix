use marix_protocol::IntentSignature;

#[derive(Debug, Clone)]
pub struct Plan {
    pub subintents: Vec<IntentSignature>,
}
