use std::fmt;

use crate::external::*;
use crate::{IntentContext, IntentResult, PlanContext};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Context {
    Intent(IntentContext),
    Plan(PlanContext),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextChain {
    pub contexts: Vec<Context>,
}

impl fmt::Display for ContextChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, context) in self.contexts.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            Self::write_context(f, context)?;
        }
        Ok(())
    }
}

// -- Private -- //

impl ContextChain {
    fn write_context(f: &mut fmt::Formatter<'_>, context: &Context) -> fmt::Result {
        match context {
            Context::Intent(context) => {
                let label = if context.signature.parent.is_none() {
                    "intent(root)"
                } else {
                    "intent"
                };
                let content = Self::json(&context.content)?;
                write!(
                    f,
                    "{label} {} | content: {content} | result: ",
                    context.signature,
                )?;
                Self::write_result(f, context.result.as_ref())?;
                write!(f, " | step_results: {}", Self::json(&context.step_results)?,)
            }
            Context::Plan(context) => Self::write_plan(f, context),
        }
    }

    fn write_plan(f: &mut fmt::Formatter<'_>, context: &PlanContext) -> fmt::Result {
        write!(
            f,
            "plan {} | intents: {} | failures: {}",
            context.signature,
            Self::json(&context.intents)?,
            Self::json(&context.failures)?,
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
