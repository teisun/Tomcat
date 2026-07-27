//! `visible_tools_for_mode` — 按 PlanState 过滤 LLM 可见工具集。
//!
//! 与 `core/tools/contract/catalog.rs` 的 `build_function_definitions` 全集（**含**
//! plan_only 工具）配对使用：chat_loop 装配 `tool_definitions` 时调用本函数，避免在
//! CHAT 期把 `create_plan` / `ask_question` 暴露给 LLM。
//!
//! 规则（plan-runtime.md §4.1 R6 / 2026-05 调整）：
//! - **Chat / Pending / Completed**：保留 `todos` / `update_plan` / `ask_question`；
//!   **排除** `create_plan`（仅 PLAN 可创建新计划）
//! - **Planning**：包含 `create_plan` / `ask_question` / `todos` / `update_plan`；
//!   写工具（`write`/`edit`/`hashline_edit`/`delete`/`bash`）**全部保留**——写盘路径由
//!   [`safety::enforce_write_path_policy`] 在 `tool_exec` 路径层拦截到 `~/.tomcat/plans/*.plan.md`。
//! - **Executing { plan_id }**：包含 `update_plan`；**排除** `create_plan` / `ask_question` / `todos`；
//!   plan 文件全禁写由 `safety` 在路径层守护，推进任务仅走 `update_plan`。

use serde_json::Value;

use super::state::PlanState;
use crate::core::tools::contract::catalog::BUILTIN_TOOL_CATALOG;

/// EXEC 模式排除的工具（plan-runtime.md §4.1 R6：EXEC 不允许 create_plan / ask_question）。
///
/// `todos` 一并隐藏：EXEC 期有两份进度清单（会话 scratchpad todos 与计划文件 todos）时，
/// 模型会挑一份写、另一份留在旧状态，最后谁也说不清做到哪了。EXEC 下进度只有一个权威 —— 计划文件，
/// 也就是只经 `update_plan`。
const HIDDEN_IN_EXECUTING: &[&str] = &["create_plan", "ask_question", "todos"];

/// CHAT / Pending / Completed 视图排除的 plan 工具（仅 `create_plan`；`todos` / `update_plan` /
/// `ask_question` 在这些模式保留）。
const HIDDEN_IN_CHAT_VIEW: &[&str] = &["create_plan"];

/// PLAN 模式排除的写工具。整文件落盘（`write`）与删除（`delete`）在计划阶段没有正当用途，
/// 直接从工具清单里拿掉比"看得见但会被拒"更省一次往返、也更不容易被模型误解为"可以动手了"。
/// 改计划正文仍需要 `edit` / `hashline_edit`，所以保留；
/// `safety::enforce_write_path_policy` 继续作为第二道防线兜住路径。
const HIDDEN_IN_PLANNING: &[&str] = &["write", "delete"];

/// 按 PlanState 过滤生成 LLM 可见工具的 OpenAI function definition 列表。
///
/// 与 `build_function_definitions` 同 serde shape：
/// ```json
/// [{ "type": "function", "function": { "name": ..., "description": ..., "parameters": {...} } }]
/// ```
pub fn visible_tools_for_mode(mode: &PlanState) -> Vec<Value> {
    visible_tools_for_mode_with_policy(mode, true)
}

pub fn visible_tools_for_mode_with_policy(mode: &PlanState, allow_load_skill: bool) -> Vec<Value> {
    BUILTIN_TOOL_CATALOG
        .iter()
        .filter(|entry| {
            filter_for_mode(entry.name, entry.plan_only, mode)
                && (allow_load_skill || entry.name != "load_skill")
        })
        .map(|entry| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": entry.name,
                    "description": entry.description,
                    "parameters": (entry.parameters)(),
                }
            })
        })
        .collect()
}

fn filter_for_mode(name: &str, _plan_only: bool, mode: &PlanState) -> bool {
    match mode {
        PlanState::Chat | PlanState::Pending { .. } | PlanState::Completed { .. } => {
            // CHAT 视图：仅排除 create_plan；保留 todos / update_plan / ask_question
            !HIDDEN_IN_CHAT_VIEW.contains(&name)
        }
        PlanState::Planning => !HIDDEN_IN_PLANNING.contains(&name),
        PlanState::Executing { .. } => !HIDDEN_IN_EXECUTING.contains(&name),
    }
}
