#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    Common {
        signature_id: String,
        message: String,
    },
    Done {
        signature_id: String,
        message: Option<String>,
    },
}
