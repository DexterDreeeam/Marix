use crate::IntentContext;
use crate::external::*;

use super::{PlanResult, PlanSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanContext {
    pub signature: PlanSignature,
    pub intents: Vec<IntentContext>,
    pub failures: Vec<PlanResult>,
}
