#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessagePrompt {
    IntentAnalyze,
    ToolExecution,
    PlanVerdict,
}

impl MessagePrompt {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::IntentAnalyze => "IntentAnalyze",
            Self::ToolExecution => "ToolExecution",
            Self::PlanVerdict => "PlanVerdict",
        }
    }

    pub(crate) fn system(self) -> SystemPrompt {
        match self {
            Self::IntentAnalyze => SystemPrompt::SystemTools,
            Self::ToolExecution | Self::PlanVerdict => SystemPrompt::System,
        }
    }

    pub(crate) fn from_relay_name(relay_name: &str) -> Result<Self, String> {
        match relay_name {
            "intent-verdict" => Ok(Self::IntentAnalyze),
            "intent-tool-execution" => Ok(Self::ToolExecution),
            "plan-verdict" => Ok(Self::PlanVerdict),
            _ => Err(format!(
                "relay `{relay_name}` has no message prompt mapping"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SystemPrompt {
    System,
    SystemTools,
}

impl SystemPrompt {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::SystemTools => "System_Tools",
        }
    }
}
