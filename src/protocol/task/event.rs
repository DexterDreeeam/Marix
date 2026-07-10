use crate::external::*;

use crate::{PlanDraft, PlanEvent, PlanSignature, PlanStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Plan(PlanSignature, PlanEvent),
    PlanCreate(PlanDraft),
    Update(PlanSignature, PlanStatus),
    Cancel,
}
