use crate::PlanDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Trigger(PlanDraft),
    Complete,
    Fail,
}
