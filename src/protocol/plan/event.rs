use crate::Plan;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Trigger(Plan),
}
