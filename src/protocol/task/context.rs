use std::fmt;

use crate::external::*;
use crate::{IntentContext, IntentResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextChain {
    pub intents: Vec<IntentContext>,
}

impl fmt::Display for ContextChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, context) in self.intents.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            Self::write_intent(f, context)?;
        }
        Ok(())
    }
}

// -- Private -- //

impl ContextChain {
    fn write_intent(f: &mut fmt::Formatter<'_>, context: &IntentContext) -> fmt::Result {
        let label = if context.signature.parent.is_none() {
            "intent(root)"
        } else {
            "intent"
        };
        let content = Self::json(&context.content)?;
        let subintents = Self::json(&context.subintents)?;
        write!(
            f,
            "{label} {} | content: {content} | result: ",
            context.signature,
        )?;
        Self::write_result(f, context.result.as_ref())?;
        write!(
            f,
            " | step_results: {} | subintents: {subintents} | plan_failures: {}",
            Self::json(&context.step_results)?,
            Self::json(&context.plan_failures)?,
        )
    }

    fn write_result(f: &mut fmt::Formatter<'_>, result: Option<&IntentResult>) -> fmt::Result {
        match result {
            Some(result) => f.write_str(&Self::json(result)?),
            None => f.write_str("pending"),
        }
    }

    fn json<T>(value: &T) -> Result<String, fmt::Error>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_string(value).map_err(|_| fmt::Error)
    }
}
