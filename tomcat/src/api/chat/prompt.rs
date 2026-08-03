use crate::core::plan_runtime::{file_store::PlanFileState, ActivePlan};
use crate::core::session::AgentMode;

fn user_prompt_label(mode: AgentMode, active_plan: Option<&ActivePlan>) -> String {
    let mode = match mode {
        AgentMode::Chat => "Chat",
        AgentMode::Plan => "Plan",
    };
    match active_plan.map(|plan| plan.state) {
        Some(PlanFileState::Executing) => format!("{mode}·plan:executing"),
        Some(PlanFileState::Pending) => format!("{mode}·plan:pending"),
        _ => mode.to_string(),
    }
}

fn agent_prompt_label(mode: AgentMode, active_plan: Option<&ActivePlan>) -> Option<String> {
    let label = user_prompt_label(mode, active_plan);
    match label.as_str() {
        "Chat" => None,
        _ => Some(label),
    }
}

pub(crate) fn user_prompt_for_mode(mode: AgentMode, active_plan: Option<&ActivePlan>) -> String {
    format!("u[{}]> ", user_prompt_label(mode, active_plan))
}

pub(crate) fn user_prompt_for_mode_with_model(
    mode: AgentMode,
    active_plan: Option<&ActivePlan>,
    model: &str,
) -> String {
    let model = model.trim();
    if model.is_empty() {
        return user_prompt_for_mode(mode, active_plan);
    }
    format!("u[{}|{}]> ", user_prompt_label(mode, active_plan), model)
}

pub(crate) fn agent_prompt_for_mode(
    agent_id: &str,
    mode: AgentMode,
    active_plan: Option<&ActivePlan>,
) -> String {
    match agent_prompt_label(mode, active_plan) {
        Some(label) => format!("agent.{agent_id}[{label}]> "),
        None => format!("agent.{agent_id}> "),
    }
}
