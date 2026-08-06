use std::collections::BTreeMap;

use marix_protocol::{
    IntentContext, IntentResultKind, RelaySignature,
};

use crate::prompt::PromptInjection;
use crate::task::TaskAccess;

const INTENT_PARAMETERS: &[&str] = &["intent", "tool_call_count"];
const PLAN_PARAMETERS: &[&str] =
    &["plan_goal", "plan_status", "plan_result"];
const TOOL_PARAMETERS: &[&str] =
    &["tool_name", "tool_arguments", "tool_result"];
const SUBINTENT_PARAMETERS: &[&str] =
    &["subintent_goal", "subintent_result"];
const FAILURE_PARAMETERS: &[&str] =
    &["failed_plan_goal", "failed_plan_reason"];

pub(super) fn inject_context(
    access: &TaskAccess,
    relay: &RelaySignature,
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
) -> Result<(), String> {
    let inject_intent = accepts_any(injections, INTENT_PARAMETERS);
    let inject_plan = accepts_any(injections, PLAN_PARAMETERS);
    let inject_tools = accepts_any(injections, TOOL_PARAMETERS);
    let inject_subintents =
        accepts_any(injections, SUBINTENT_PARAMETERS);
    let inject_failures =
        accepts_any(injections, FAILURE_PARAMETERS);
    if !(inject_intent
        || inject_plan
        || inject_tools
        || inject_subintents
        || inject_failures)
    {
        return Ok(());
    }
    let chain = access.get_context_chain(&relay.intent)?;
    let current = chain
        .intents
        .last()
        .ok_or_else(|| {
            "cannot assemble an empty context chain".to_owned()
        })?;
    if inject_intent {
        inject_single(
            injections,
            "intent",
            current.content.clone(),
        );
        inject_single(
            injections,
            "tool_call_count",
            current.tool_call_count.to_string(),
        );
    }
    for (intent_index, intent) in chain.intents.iter().enumerate() {
        if inject_plan {
            inject_plan_items(
                access,
                intent,
                intent_index,
                injections,
            )?;
        }
        if inject_tools {
            inject_tool_calls(intent, intent_index, injections);
        }
    }
    if inject_subintents {
        inject_subintent_results(access, current, injections)?;
    }
    if inject_failures {
        inject_plan_failures(current, injections);
    }
    Ok(())
}

// -- Private -- //

fn accepts_any(
    injections: &BTreeMap<String, Vec<PromptInjection>>,
    names: &[&str],
) -> bool {
    names.iter().any(|name| injections.contains_key(*name))
}

fn inject_plan_items(
    access: &TaskAccess,
    intent: &IntentContext,
    intent_index: usize,
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
) -> Result<(), String> {
    for (index, signature) in intent.subintents.iter().enumerate() {
        let subintent = access.get_intent_context(signature)?;
        let tag = format!("{intent_index:08}:{index:08}");
        let (status, result) = match subintent.result {
            Some(result) => {
                let status = match result.kind {
                    IntentResultKind::Succeed => "complete",
                    IntentResultKind::Canceled => "canceled",
                    IntentResultKind::Failed => "failed",
                    IntentResultKind::Infeasible => "infeasible",
                };
                (status.to_owned(), result.output)
            }
            None => ("executing".to_owned(), String::new()),
        };
        inject_tagged(
            injections,
            "plan_goal",
            subintent.content,
            &tag,
        );
        inject_tagged(
            injections,
            "plan_status",
            status,
            &tag,
        );
        inject_tagged(
            injections,
            "plan_result",
            result,
            &tag,
        );
    }
    Ok(())
}

fn inject_tool_calls(
    intent: &IntentContext,
    intent_index: usize,
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
) {
    let mut call_index = 0;
    for step in &intent.step_results {
        for call in &step.calls {
            let tag =
                format!("{intent_index:08}:{call_index:08}");
            inject_tagged(
                injections,
                "tool_name",
                call.tool.clone(),
                &tag,
            );
            inject_tagged(
                injections,
                "tool_arguments",
                call.input.clone(),
                &tag,
            );
            inject_tagged(
                injections,
                "tool_result",
                call.result.output.clone(),
                &tag,
            );
            call_index += 1;
        }
    }
}

fn inject_subintent_results(
    access: &TaskAccess,
    intent: &IntentContext,
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
) -> Result<(), String> {
    for (index, signature) in intent.subintents.iter().enumerate() {
        let subintent = access.get_intent_context(signature)?;
        let Some(result) = subintent.result else {
            continue;
        };
        let tag = format!("{index:08}");
        inject_tagged(
            injections,
            "subintent_goal",
            subintent.content,
            &tag,
        );
        inject_tagged(
            injections,
            "subintent_result",
            result.output,
            &tag,
        );
    }
    Ok(())
}

fn inject_plan_failures(
    intent: &IntentContext,
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
) {
    for (plan_index, failure) in
        intent.plan_failures.iter().enumerate()
    {
        for (goal_index, goal) in
            failure.goals.iter().enumerate()
        {
            let tag =
                format!("{plan_index:08}:{goal_index:08}");
            inject_tagged(
                injections,
                "failed_plan_goal",
                goal.clone(),
                &tag,
            );
            inject_tagged(
                injections,
                "failed_plan_reason",
                failure.reason.clone(),
                &tag,
            );
        }
    }
}

fn inject_single(
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    name: &str,
    value: String,
) {
    let Some(values) = injections.get_mut(name) else {
        return;
    };
    if values.is_empty() {
        values.push(PromptInjection { value, tag: None });
    }
}

fn inject_tagged(
    injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    name: &str,
    value: String,
    tag: &str,
) {
    let Some(values) = injections.get_mut(name) else {
        return;
    };
    values.push(PromptInjection {
        value,
        tag: Some(tag.to_owned()),
    });
}
