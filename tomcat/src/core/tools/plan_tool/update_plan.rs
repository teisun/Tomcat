//! `update_plan` 工具实现（plan-runtime.md §P2 / [update-plan.md] / G1+G2+N2 2026-05）。
//!
//! 语义：
//! - 任何模式可见；按 `plan_id` / `path` 路由（`plan_id` 优先，缺省取 active plan）。
//! - 入参 **仅认 `kind`**（D3 破坏性）：`upsert | set_status | remove`。
//! - State 矩阵闸门（G2 / `update-plan.md` §6.2）：
//!   - 目标 `plan.state == completed` → 全拒（N2）。
//!   - `set_status: in_progress` 仅 `executing` 允许；planning / pending 一律拒。
//! - 跨 session 编辑规则：
//!   - 目标 plan `state ∈ {planning, pending}`：允许（协作改稿）
//!   - 目标 plan `state == executing` 且 `session_key != current_session_key`：拒
//! - 写盘后 EXEC 自动派生：所有 todos completed → 先写 `Executing`，若 code review
//!   轮次未耗尽则先派发 code reviewer；`verdict=pass` 时同回合 verifier，否则把
//!   `code_review` 返回给主 Agent。code review 轮次耗尽后直接走 verifier。
//! - 返回 JSON（G1）：`plan_id` / `path` / `applied` / `items[]` /
//!   `active_in_progress` / `plan_state_before` / `plan_state_after` / `warnings[]` /
//!   `panel_snapshot_id` / `code_review` / `verify`（节流后 panel 刷新版本；目前与 timestamp 等价）。

use std::path::PathBuf;

use serde::Deserialize;

use crate::core::plan_runtime::{
    file_store::{
        read_plan, update_plan_locked, write_plan, GreenBuildEvidence, PlanFileState, TodoStatus,
    },
    ops,
    review::{Finding, SeverityTier},
    PlanRuntime,
};

use super::shared_todo_ops::{apply_shared_todo_ops, items_json};
use super::ToolError;

#[derive(Debug, Deserialize)]
pub struct UpdatePlanArgs {
    /// 目标 plan_id；执行中计划可省略（默认当前 active plan）。
    #[serde(default)]
    pub plan_id: Option<String>,
    /// 可选直接路径；仅在未传 plan_id 时生效。
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub replace: bool,
    /// 增量 ops；统一为 `kind` 标记的 enum（D3 破坏性）。
    #[serde(default)]
    pub ops: Vec<UpdateOp>,
    /// 主 Agent 对当轮 P1 finding 的书面申辩。修复不走此字段：改代码后重新收口，
    /// 让下一轮 reviewer 从代码复核。
    #[serde(default)]
    pub dispute_findings: Vec<DisputeFindingArg>,
    /// 仅在 verify skill 完成后置 true；运行时会用后台 bash 任务账本验证证据。
    #[serde(default)]
    pub green_build_pass: Option<bool>,
    #[serde(default)]
    pub green_build_evidence: Vec<GreenBuildEvidenceArg>,
}

pub use super::shared_todo_ops::SharedTodoOpArg as UpdateOp;

#[derive(Debug, Deserialize)]
pub struct DisputeFindingArg {
    /// JSON 仍使用 `ref`，避免把 Rust 关键字泄漏进工具契约。
    #[serde(rename = "ref")]
    pub reference: String,
    /// 只供人读和事后排查；匹配唯一认 `ref`。
    pub area: String,
    pub resolution: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct GreenBuildEvidenceArg {
    /// 人类可读的验收命令；必须与后台任务实际启动的命令一致，防止伪造证据描述。
    pub command: String,
    pub task_id: String,
}

impl UpdatePlanArgs {
    pub fn from_json(raw: &serde_json::Value) -> Result<Self, ToolError> {
        // D3 破坏性：旧字段名 `op` 已下线，遇到立即报错。
        if let Some(ops) = raw.get("ops").and_then(|v| v.as_array()) {
            for op in ops {
                if op.get("op").is_some() && op.get("kind").is_none() {
                    return Err(ToolError::BadArgs(
                        "update_plan ops: 字段 `op` 已下线，请改用 `kind`（kind: upsert | set_status | remove）".into(),
                    ));
                }
            }
        }
        if raw.get("replace_todos").is_some() || raw.get("replace_milestones").is_some() {
            return Err(ToolError::BadArgs(
                "update_plan 顶层字段 `replace_todos` / `replace_milestones` 已下线，请统一改用 `replace`"
                    .into(),
            ));
        }
        if raw.get("milestones_ops").is_some() {
            return Err(ToolError::BadArgs(
                "update_plan 不再支持 `milestones_ops`；当前仅支持 todo-only ops".into(),
            ));
        }
        serde_json::from_value(raw.clone())
            .map_err(|e| ToolError::BadArgs(format!("update_plan args: {e}")))
    }
}

pub async fn execute(
    runtime: &PlanRuntime,
    args: UpdatePlanArgs,
) -> Result<serde_json::Value, ToolError> {
    execute_for_tool(runtime, args, "legacy-update-plan").await
}

pub async fn execute_for_tool(
    runtime: &PlanRuntime,
    args: UpdatePlanArgs,
    tool_call_id: &str,
) -> Result<serde_json::Value, ToolError> {
    let path = resolve_target_plan_path(runtime, args.plan_id.clone(), args.path.clone())?;
    let target_plan_for_disputes = read_plan(&path)
        .map_err(|error| ToolError::BadArgs(format!("读取目标 plan 失败：{error}")))?
        .frontmatter
        .plan_id;
    let prepared_disputes =
        prepare_disputes(runtime, &target_plan_for_disputes, &args.dispute_findings)?;
    struct UpdateTxOutcome {
        plan: crate::core::plan_runtime::file_store::PlanFile,
        target_plan_id: String,
        plan_state_before: PlanFileState,
        warnings: Vec<String>,
        active_in_progress: Option<String>,
        derived_completed: bool,
    }

    let tx = match update_plan_locked(&path, runtime.lock_timeout_ms(), |plan| {
        let target_plan_id = plan.frontmatter.plan_id.clone();
        let plan_state_before = plan.frontmatter.state;

        enforce_cross_session_policy(runtime, &plan.frontmatter, plan_state_before)?;

        // G2 state 矩阵闸门：先做语义校验，再下沉到 ops 引擎。
        enforce_state_matrix(plan_state_before, &args.ops)?;

        apply_shared_todo_ops(&mut plan.frontmatter.todos, &args.ops, args.replace)?;

        let warnings: Vec<String> = Vec::new();
        let all_completed = ops::all_completed(&plan.frontmatter.todos);
        // 「所有 todo 已完成」本身不是一次新的收口尝试：主 Agent 可能只是在提交
        // P1 申辩。只有 todo 操作实际推进过计划时，才派发下一轮 reviewer。
        // 绿构建证据的无 todo 收口会在其门禁参数接入后单独纳入此条件。
        let derived_completed = matches!(plan_state_before, PlanFileState::Executing)
            && all_completed
            && (!args.ops.is_empty()
                || !args.dispute_findings.is_empty()
                || args.green_build_pass.is_some());

        if matches!(plan_state_before, PlanFileState::Completed) && !all_completed {
            plan.frontmatter.state = PlanFileState::Pending;
        }

        // E2：在 body 的 `## Todos Board` 标记区间内自动重写当前 todos 状态视图。
        rewrite_todos_board(&mut plan.body, &plan.frontmatter.todos);

        if derived_completed {
            // 第一写：todos 完成，但 state 保持 Executing，确保 verifier/code reviewer 看到的是
            // 「已做完 todos、尚未正式收工」的磁盘态。
            plan.frontmatter.state = PlanFileState::Executing;
        }

        let active_in_progress = plan
            .frontmatter
            .todos
            .iter()
            .find(|t| matches!(t.status, TodoStatus::InProgress))
            .map(|t| t.id.clone());

        Ok(UpdateTxOutcome {
            plan: plan.clone(),
            target_plan_id,
            plan_state_before,
            warnings,
            active_in_progress,
            derived_completed,
        })
    }) {
        Ok(v) => v,
        Err(crate::core::plan_runtime::file_store::LockedPlanMutationError::Plan(e)) => {
            return Err(e.into());
        }
        Err(crate::core::plan_runtime::file_store::LockedPlanMutationError::Callback(e)) => {
            return Err(e);
        }
    };

    let applied = args.ops.len();
    let mut plan = tx.plan;
    let target_plan_id = tx.target_plan_id;
    let plan_state_before = tx.plan_state_before;
    let active_in_progress = tx.active_in_progress;
    let mut warnings = tx.warnings;
    for (finding, reason) in prepared_disputes {
        let reference = finding.reference.clone();
        runtime.add_disputed_finding(&target_plan_id, finding, reason);
        warnings.push(format!(
            "P1 finding {reference} 已作为已知取舍记录；下一轮 reviewer 会收到“勿重报”说明"
        ));
    }

    let mut code_review_json = serde_json::Value::Null;
    let code_diff = match runtime.workspace_root() {
        Some(workspace_root) if tx.derived_completed => {
            crate::core::plan_runtime::code_reviewer::collect_code_diff_context(&workspace_root)
                .await
        }
        _ => crate::core::plan_runtime::code_reviewer::CodeDiffContext::default(),
    };
    let code_gate_required = tx.derived_completed
        && runtime.workspace_root().is_some()
        && !code_diff.changed_code_files.is_empty();
    let newest_edit_mtime_ms = code_diff.newest_edit_mtime_ms;
    let had_previous_full_gate =
        plan.frontmatter.code_review_pass && plan.frontmatter.green_build_pass;
    let review_was_fresh = newest_edit_mtime_ms.is_some_and(|mtime| {
        plan.frontmatter.code_review_pass
            && plan
                .frontmatter
                .code_review_pass_at_ms
                .is_some_and(|passed_at| passed_at >= mtime)
    });
    let gates_were_fresh =
        newest_edit_mtime_ms.is_some_and(|mtime| code_gates_are_fresh(&plan.frontmatter, mtime));
    if newest_edit_mtime_ms.is_some() && !review_was_fresh {
        invalidate_code_gates(&mut plan.frontmatter);
        write_plan(&path, &plan, runtime.lock_timeout_ms())?;
        runtime.refresh_active_plan_after_write(path.clone(), &plan);
    }
    if tx.derived_completed {
        write_plan_progress_transcript(runtime, &target_plan_id, &path, &plan);
    }
    let plan_state_after = if tx.derived_completed {
        if runtime.workspace_root().is_some() && code_diff.changed_code_files.is_empty() {
            // 文档、配置或纯计划类交付没有代码 diff：不让 reviewer / 绿构建凭空挡住收口。
            finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
            PlanFileState::Completed
        } else if gates_were_fresh {
            finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
            PlanFileState::Completed
        } else if review_was_fresh {
            require_green_build_pass(
                runtime,
                &args,
                newest_edit_mtime_ms.expect("code diff has mtime"),
                &path,
                &mut plan,
            )?;
            finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
            PlanFileState::Completed
        } else if prior_gate_cycles_exhausted(
            &plan.frontmatter,
            had_previous_full_gate,
            runtime.max_completion_gate_cycles(),
        ) {
            warnings.push(format!(
                "代码在已通过门禁后再次修改，但验收重跑已达到上限 {}；按上限放行收口，门禁通过标志已失效",
                runtime.max_completion_gate_cycles()
            ));
            finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
            PlanFileState::Completed
        } else if runtime.code_review_infra_retry_exhausted(&target_plan_id) {
            warnings.push(
                "code review 连续技术故障已超过 2 次，plan 保持 executing 并交还用户决定".into(),
            );
            write_code_review_handoff(
                runtime,
                &target_plan_id,
                runtime.code_review_rounds(&target_plan_id),
            );
            PlanFileState::Executing
        } else if let Some(round) = runtime.try_begin_code_review_round(&target_plan_id) {
            let review_attempt_id = format!("{target_plan_id}:{round}");
            let dispatch = crate::core::plan_runtime::CodeReviewDispatchInfo {
                round,
                review_attempt_id: review_attempt_id.clone(),
                tool_call_id: tool_call_id.to_string(),
            };
            let mut code_review_summary = runtime
                .dispatch_code_reviewer(&target_plan_id, &dispatch)
                .await;
            warnings.extend(code_review_summary.normalize_for_result());
            runtime.write_code_review_transcript(
                &target_plan_id,
                &code_review_summary,
                round,
                &review_attempt_id,
                tool_call_id,
            );
            code_review_json = code_review_summary.to_json();

            // 未注入 reviewer 是「没有这道门」，不能永久扣住计划。
            if code_review_summary.aborted
                && code_review_summary.reviewer_stop_reason == "not_dispatched"
            {
                warnings.push("未配置 code reviewer，记录为跳过复审；仍必须通过绿构建门禁".into());
                record_code_review_pass(&mut plan.frontmatter, had_previous_full_gate);
                write_plan(&path, &plan, runtime.lock_timeout_ms())?;
                runtime.refresh_active_plan_after_write(path.clone(), &plan);
                if code_gate_required {
                    require_green_build_pass(
                        runtime,
                        &args,
                        newest_edit_mtime_ms.expect("code diff has mtime"),
                        &path,
                        &mut plan,
                    )?;
                }
                finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
                PlanFileState::Completed
            } else if code_review_summary.aborted {
                // 子 Agent 没产出可解析结论：这不是发现问题，不应消耗正常 review 配额。
                runtime.refund_code_review_round(&target_plan_id);
                let retries = runtime.bump_review_infra_retry(&target_plan_id);
                if retries > 2 {
                    warnings.push(
                        "code review 连续技术故障已超过 2 次：本轮预算已退还，plan 保持 executing 并交还用户决定"
                            .into(),
                    );
                    write_code_review_handoff(runtime, &target_plan_id, round);
                } else {
                    warnings.push(format!(
                        "code review 技术故障（{}）：本轮预算已退还，将允许第 {}/2 次基础设施重试",
                        code_review_summary.reviewer_stop_reason, retries
                    ));
                }
                PlanFileState::Executing
            } else {
                let disputed = runtime.disputed_findings(&target_plan_id);
                let blocking = blocking_findings(&code_review_summary.findings, &disputed);
                let verdict_is_aborted = code_review_summary.verdict.as_deref() == Some("aborted");

                // reviewer 的 verdict 不能绕过运行时的 P0/P1 门禁：模型若写了 pass
                // 却仍列出 P0/P1，按 finding 的机器分级拒绝收口。
                if !blocking.is_empty() || verdict_is_aborted {
                    if code_review_summary.verdict.as_deref() == Some("pass")
                        && !blocking.is_empty()
                    {
                        warnings.push(
                            "code reviewer verdict=pass 但仍返回未裁决 P0/P1 finding；运行时按 finding 阻止收口"
                                .into(),
                        );
                    }
                    runtime.set_unresolved_findings(&target_plan_id, blocking);
                    plan.frontmatter.code_review_pass = false;
                    plan.frontmatter.code_review_pass_at_ms = None;
                    write_plan(&path, &plan, runtime.lock_timeout_ms())?;
                    runtime.refresh_active_plan_after_write(path.clone(), &plan);
                    warnings.extend(non_pass_code_review_guidance(
                        &code_review_summary,
                        runtime.max_code_review_rounds().saturating_sub(round),
                    ));
                    PlanFileState::Executing
                } else {
                    runtime.set_unresolved_findings(&target_plan_id, Vec::new());
                    record_code_review_pass(&mut plan.frontmatter, had_previous_full_gate);
                    write_plan(&path, &plan, runtime.lock_timeout_ms())?;
                    runtime.refresh_active_plan_after_write(path.clone(), &plan);
                    if code_gate_required {
                        require_green_build_pass(
                            runtime,
                            &args,
                            newest_edit_mtime_ms.expect("code diff has mtime"),
                            &path,
                            &mut plan,
                        )?;
                    }
                    finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
                    PlanFileState::Completed
                }
            }
        } else {
            let rounds = runtime.code_review_rounds(&target_plan_id);
            let unresolved = runtime.unresolved_finding_references(&target_plan_id);
            if rounds == 0 {
                // 一轮都没跑过 = 复审被关掉了（max_code_review_rounds = 0）。
                // 不存在的门禁不能扣住计划，但要说出来，别让用户以为它复审过了。
                warnings.push(format!(
                    "code review 未启用（max_code_review_rounds = {}），记录为跳过复审；仍必须通过绿构建门禁",
                    runtime.max_code_review_rounds()
                ));
                record_code_review_pass(&mut plan.frontmatter, had_previous_full_gate);
                write_plan(&path, &plan, runtime.lock_timeout_ms())?;
                runtime.refresh_active_plan_after_write(path.clone(), &plan);
                if code_gate_required {
                    require_green_build_pass(
                        runtime,
                        &args,
                        newest_edit_mtime_ms.expect("code diff has mtime"),
                        &path,
                        &mut plan,
                    )?;
                }
                finalize_plan_completed(runtime, &target_plan_id, &path, &mut plan)?;
                PlanFileState::Completed
            } else {
                // 跑过复审但一次都没拿到 pass，而轮次已经用完：交还用户，让人来决定放行还是继续。
                // reviewer 说了 fail 却没列出 finding，那是它没写清楚，不是问题不存在 ——
                // 按「没有已知问题」收口，等于让一句没有明细的 fail 直接变成交付。
                let unresolved_note = if unresolved.is_empty() {
                    "且最后一轮未返回通过结论".to_string()
                } else {
                    format!("仍有 {} 项未清 finding", unresolved.len())
                };
                warnings.push(format!(
                    "code review 轮次预算已用尽（{}/{}），{unresolved_note}；plan 保持 executing，交还用户决定",
                    rounds,
                    runtime.max_code_review_rounds(),
                ));
                runtime.write_code_review_exhausted_transcript(
                    &target_plan_id,
                    rounds,
                    &unresolved,
                );
                PlanFileState::Executing
            }
        }
    } else {
        plan.frontmatter.state
    };

    let panel_snapshot_id = crate::core::plan_runtime::panels::next_panel_snapshot_id();
    runtime.refresh_active_plan_after_write(path.clone(), &plan);

    // E：fanout UI 刷新——advisory lock 在 write_plan 内已 release，这里仅同步通知
    // 已注册 panel；panel 自行决定如何渲染（CLI/IDE/noop）。
    let snapshot = crate::core::plan_runtime::panels::TodosPanelSnapshot {
        panel_snapshot_id,
        scope: format!("plan:{target_plan_id}"),
        items: plan.frontmatter.todos.clone(),
        warnings: warnings.clone(),
    };
    runtime.refresh_notifier().notify(&snapshot);

    if matches!(plan_state_before, PlanFileState::Completed)
        && matches!(plan_state_after, PlanFileState::Pending)
    {
        runtime.write_transcript_custom(serde_json::json!({
            "event": crate::infra::wire::WIRE_PLAN_PENDING,
            "plan_id": target_plan_id,
            "path": crate::infra::platform::format_home_path(&path),
            "state": PlanFileState::Pending.as_str(),
        }));
    }

    let event_payload = crate::infra::events::PlanEventPayload {
        plan_id: target_plan_id.clone(),
        path: crate::infra::platform::format_home_path(&path),
        state: plan_state_after.as_str().to_string(),
    };
    if !(tx.derived_completed
        || matches!(plan_state_before, PlanFileState::Completed)
            && matches!(plan_state_after, PlanFileState::Pending))
    {
        runtime.write_transcript_custom(serde_json::json!({
            "event": crate::infra::wire::WIRE_PLAN_UPDATE,
            "plan_id": event_payload.plan_id,
            "path": event_payload.path,
            "state": event_payload.state,
        }));
    }
    if !tx.derived_completed {
        runtime.write_transcript_custom(serde_json::json!({
            "event": crate::infra::wire::WIRE_PLAN_TODOS,
            "plan_id": target_plan_id,
            "todos": items_json(&plan.frontmatter.todos),
        }));
    }

    Ok(serde_json::json!({
        "plan_id": target_plan_id,
        "path": crate::infra::platform::format_home_path(&path),
        "applied": applied,
        "replace": args.replace,
        "plan_state_before": plan_state_before.as_str(),
        "plan_state_after": plan_state_after.as_str(),
        "panel_snapshot_id": panel_snapshot_id,
        "warnings": warnings,
        "active_in_progress": active_in_progress,
        "items": items_json(&plan.frontmatter.todos),
        "code_review": code_review_json,
    }))
}

fn write_plan_progress_transcript(
    runtime: &PlanRuntime,
    target_plan_id: &str,
    path: &std::path::Path,
    plan: &crate::core::plan_runtime::file_store::PlanFile,
) {
    runtime.write_transcript_custom(serde_json::json!({
        "event": crate::infra::wire::WIRE_PLAN_UPDATE,
        "plan_id": target_plan_id,
        "path": crate::infra::platform::format_home_path(path),
        "state": PlanFileState::Executing.as_str(),
    }));
    runtime.write_transcript_custom(serde_json::json!({
        "event": crate::infra::wire::WIRE_PLAN_TODOS,
        "plan_id": target_plan_id,
        "todos": items_json(&plan.frontmatter.todos),
    }));
}

fn finalize_plan_completed(
    runtime: &PlanRuntime,
    target_plan_id: &str,
    path: &std::path::Path,
    plan: &mut crate::core::plan_runtime::file_store::PlanFile,
) -> Result<(), ToolError> {
    plan.frontmatter.state = PlanFileState::Completed;
    write_plan(path, plan, runtime.lock_timeout_ms())?;
    runtime.refresh_active_plan_after_write(path.to_path_buf(), plan);
    runtime.write_transcript_custom(serde_json::json!({
        "event": crate::infra::wire::WIRE_PLAN_COMPLETE,
        "plan_id": target_plan_id,
        "path": crate::infra::platform::format_home_path(path),
        "state": PlanFileState::Completed.as_str(),
    }));
    Ok(())
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn code_gates_are_fresh(
    frontmatter: &crate::core::plan_runtime::file_store::PlanFileFrontmatter,
    newest_edit_mtime_ms: u128,
) -> bool {
    frontmatter.code_review_pass
        && frontmatter
            .code_review_pass_at_ms
            .is_some_and(|passed_at| passed_at >= newest_edit_mtime_ms)
        && frontmatter.green_build_pass
        && frontmatter
            .green_build_evidence
            .iter()
            .any(|evidence| evidence.started_at_ms >= newest_edit_mtime_ms)
}

fn invalidate_code_gates(
    frontmatter: &mut crate::core::plan_runtime::file_store::PlanFileFrontmatter,
) {
    frontmatter.code_review_pass = false;
    frontmatter.code_review_pass_at_ms = None;
    frontmatter.green_build_pass = false;
    frontmatter.green_build_evidence.clear();
}

fn prior_gate_cycles_exhausted(
    frontmatter: &crate::core::plan_runtime::file_store::PlanFileFrontmatter,
    had_previous_full_gate: bool,
    max_cycles: u32,
) -> bool {
    had_previous_full_gate && frontmatter.completion_gate_cycles >= max_cycles
}

fn record_code_review_pass(
    frontmatter: &mut crate::core::plan_runtime::file_store::PlanFileFrontmatter,
    is_rerun_after_previous_full_gate: bool,
) {
    frontmatter.code_review_pass = true;
    frontmatter.code_review_pass_at_ms = Some(now_unix_ms());
    if is_rerun_after_previous_full_gate {
        frontmatter.completion_gate_cycles = frontmatter.completion_gate_cycles.saturating_add(1);
    }
}

fn green_build_guidance() -> ToolError {
    ToolError::BadArgs(
        "代码 diff 已通过（或跳过）code review，但绿构建验收尚未通过。请先调用 load_skill(name=\"verify\")，按 skill 发现并运行适合当前项目的 build/test/lint 或 UI smoke 命令；命令必须通过 bash(run_in_background=true) 启动。完成后调用 update_plan，传 green_build_pass:true 和 green_build_evidence:[{command:\"<实际 bash 命令>\",task_id:\"...\"}]。运行时会核验命令、任务 exit_code=0 且 started_at 不早于最新代码修改时间。"
            .into(),
    )
}

fn require_green_build_pass(
    runtime: &PlanRuntime,
    args: &UpdatePlanArgs,
    newest_edit_mtime_ms: u128,
    path: &std::path::Path,
    plan: &mut crate::core::plan_runtime::file_store::PlanFile,
) -> Result<(), ToolError> {
    if plan.frontmatter.green_build_pass
        && plan
            .frontmatter
            .green_build_evidence
            .iter()
            .any(|evidence| evidence.started_at_ms >= newest_edit_mtime_ms)
    {
        return Ok(());
    }

    if args.green_build_pass != Some(true) {
        if args.green_build_pass == Some(false) {
            plan.frontmatter.green_build_pass = false;
            plan.frontmatter.green_build_evidence.clear();
            write_plan(path, plan, runtime.lock_timeout_ms())?;
            runtime.refresh_active_plan_after_write(path.to_path_buf(), plan);
        }
        return Err(green_build_guidance());
    }
    if args.green_build_evidence.is_empty() {
        return Err(ToolError::BadArgs(
            "green_build_pass=true 必须同时传入至少一个 green_build_evidence.task_id".into(),
        ));
    }
    let Some(registry) = runtime.bash_task_registry() else {
        return Err(ToolError::BadArgs(
            "当前运行时没有后台 bash 任务账本，无法核验绿构建证据；请重新执行 verify skill 中的命令"
                .into(),
        ));
    };

    let mut seen_task_ids = std::collections::BTreeSet::new();
    let mut verified = Vec::with_capacity(args.green_build_evidence.len());
    for evidence in &args.green_build_evidence {
        if evidence.command.trim().is_empty() {
            return Err(ToolError::BadArgs(
                "green_build_evidence.command 不能为空；请填写对应后台 bash 的实际命令".into(),
            ));
        }
        if !seen_task_ids.insert(evidence.task_id.trim()) {
            return Err(ToolError::BadArgs(format!(
                "green_build_evidence.task_id 重复：{}",
                evidence.task_id
            )));
        }
        let task = registry.get_info(evidence.task_id.trim()).ok_or_else(|| {
            ToolError::BadArgs(format!(
                "找不到后台 bash 任务 `{}`；只能引用本会话 verify skill 实际启动的 task_id",
                evidence.task_id
            ))
        })?;
        let crate::core::tools::primitive::BashTaskStatus::Finished { exit_code } = task.status
        else {
            return Err(ToolError::BadArgs(format!(
                "后台任务 `{}` 尚未成功结束；绿构建证据必须是 exit_code=0 的 Finished 任务",
                evidence.task_id
            )));
        };
        if exit_code != 0 {
            return Err(ToolError::BadArgs(format!(
                "后台任务 `{}` 的 exit_code={}，不能作为绿构建通过证据",
                evidence.task_id, exit_code
            )));
        }
        if evidence.command.trim() != task.command.trim() {
            return Err(ToolError::BadArgs(format!(
                "green_build_evidence.command 与后台任务 `{}` 的实际命令不一致；请原样提交任务启动命令",
                evidence.task_id
            )));
        }
        if task.started_at_unix_ms < newest_edit_mtime_ms {
            return Err(ToolError::BadArgs(format!(
                "后台任务 `{}` 启动于最新代码修改之前，证据已过期；请重新运行验收命令",
                evidence.task_id
            )));
        }
        verified.push(GreenBuildEvidence {
            command: task.command,
            task_id: task.task_id.to_string(),
            started_at_ms: task.started_at_unix_ms,
            exit_code,
        });
    }

    plan.frontmatter.green_build_pass = true;
    plan.frontmatter.green_build_evidence = verified;
    write_plan(path, plan, runtime.lock_timeout_ms())?;
    runtime.refresh_active_plan_after_write(path.to_path_buf(), plan);
    runtime.write_transcript_custom(serde_json::json!({
        "event": "plan.green_build",
        "plan_id": &plan.frontmatter.plan_id,
        "pass": true,
        "evidence": &plan.frontmatter.green_build_evidence,
    }));
    Ok(())
}

fn prepare_disputes(
    runtime: &PlanRuntime,
    plan_id: &str,
    disputes: &[DisputeFindingArg],
) -> Result<Vec<(Finding, String)>, ToolError> {
    let unresolved = runtime.unresolved_findings(plan_id);
    let mut prepared = Vec::with_capacity(disputes.len());

    for dispute in disputes {
        if dispute.resolution != "wontfix" {
            return Err(ToolError::BadArgs(format!(
                "{} 的 resolution 只支持 \"wontfix\"（申辩）；标记已修请改代码后重新收口",
                dispute.reference
            )));
        }
        let Some(finding) = unresolved
            .iter()
            .find(|finding| finding.reference == dispute.reference)
            .cloned()
        else {
            return Err(ToolError::BadArgs(format!(
                "{}（area=\"{}\"）不在未决清单；它可能已修、已申辩，或只是不会阻塞的 P2",
                dispute.reference, dispute.area
            )));
        };
        match finding.tier() {
            SeverityTier::P0 => {
                return Err(ToolError::BadArgs(format!(
                    "{} 是 P0，不可申辩，必须修复或交还用户",
                    dispute.reference
                )));
            }
            SeverityTier::P1 if dispute.reason.trim().is_empty() => {
                return Err(ToolError::BadArgs(format!(
                    "申辩 {} 必须提供 reason",
                    dispute.reference
                )));
            }
            SeverityTier::P1 => prepared.push((finding, dispute.reason.trim().to_owned())),
            SeverityTier::P2 => {
                return Err(ToolError::BadArgs(format!(
                    "{} 是 P2，不会阻塞收口，无需申辩",
                    dispute.reference
                )));
            }
        }
    }

    Ok(prepared)
}

fn normalize_finding_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn disputed_matches(
    disputed: &[crate::core::plan_runtime::DisputedFinding],
    finding: &Finding,
) -> bool {
    let area = normalize_finding_text(&finding.area);
    let note = normalize_finding_text(&finding.note);
    disputed.iter().any(|accepted| {
        normalize_finding_text(&accepted.area) == area
            && normalize_finding_text(&accepted.note) == note
    })
}

fn blocking_findings(
    findings: &[Finding],
    disputed: &[crate::core::plan_runtime::DisputedFinding],
) -> Vec<Finding> {
    findings
        .iter()
        .filter(|finding| finding.blocks())
        .filter(|finding| {
            !(matches!(finding.tier(), SeverityTier::P1) && disputed_matches(disputed, finding))
        })
        .cloned()
        .collect()
}

fn write_code_review_handoff(runtime: &PlanRuntime, plan_id: &str, rounds: u32) {
    runtime.write_code_review_exhausted_transcript(
        plan_id,
        rounds,
        &runtime.unresolved_finding_references(plan_id),
    );
}

fn non_pass_code_review_guidance(
    summary: &crate::core::plan_runtime::code_reviewer::CodeReviewSummary,
    remaining_rounds: u32,
) -> Vec<String> {
    let verdict = summary.verdict.as_deref().unwrap_or("partial");
    let finding_hint = if summary.findings.is_empty() {
        "当前 findings 为空，请根据 code_review.summary 归纳一个修复点。"
    } else {
        "请直接根据 code_review.findings 落修复。"
    };
    vec![
        format!(
            "code review verdict={verdict}，plan 保持 executing。{finding_hint} 用 update_plan 重新打开一个已有 todo（set_status=in_progress），或新增一个修复 todo；修复完成后再次调用 update_plan 收口。"
        ),
        format!(
            "本次 code review 还可派发 {remaining_rounds} 轮（上限由 [plan].max_code_review_rounds 决定）；修复后再次收口会重新复审。"
        ),
    ]
}

fn resolve_target_plan_path(
    runtime: &PlanRuntime,
    explicit_plan_id: Option<String>,
    explicit_path: Option<String>,
) -> Result<PathBuf, ToolError> {
    if let Some(id) = explicit_plan_id {
        return runtime.resolved_plan_path(&id).map_err(ToolError::BadArgs);
    }
    if let Some(path) = explicit_path {
        return crate::infra::platform::normalize_path(&path)
            .map_err(|e| ToolError::BadArgs(format!("update_plan path 非法：{e}")));
    }
    if let Some(plan) = runtime.active_plan() {
        return Ok(plan.path);
    }
    Err(ToolError::BadArgs(
        "update_plan 需要 plan_id 或 path；当前模式无 active plan".into(),
    ))
}

fn enforce_cross_session_policy(
    runtime: &PlanRuntime,
    fm: &crate::core::plan_runtime::file_store::PlanFileFrontmatter,
    state: PlanFileState,
) -> Result<(), ToolError> {
    if !matches!(state, PlanFileState::Executing) {
        return Ok(());
    }
    let target_key = fm.session_key.as_deref().unwrap_or("");
    if target_key != runtime.session_key() {
        return Err(ToolError::CrossSessionDenied(format!(
            "plan {} 当前由 session {target_key} 在 EXEC，本 session {} 不能写入",
            fm.plan_id,
            runtime.session_key()
        )));
    }
    Ok(())
}

/// G2 state 矩阵闸门——参考 [update-plan.md] §6.2。
fn enforce_state_matrix(plan_state: PlanFileState, ops_list: &[UpdateOp]) -> Result<(), ToolError> {
    for op in ops_list {
        match (plan_state, op) {
            // in_progress 仅在 executing 允许
            (
                PlanFileState::Planning | PlanFileState::Pending,
                UpdateOp::SetStatus {
                    status: TodoStatus::InProgress,
                    ..
                },
            )
            | (
                PlanFileState::Planning | PlanFileState::Pending,
                UpdateOp::Upsert {
                    status: Some(TodoStatus::InProgress),
                    ..
                },
            ) => {
                return Err(ToolError::BadArgs(format!(
                    "in_progress 仅允许在 executing 状态下使用；当前 plan.state = {}",
                    plan_state.as_str()
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// E2：在 `## Todos Board` 的标记区间内重写 todos 状态视图。
///
/// 标记格式：
/// ```text
/// ## Todos Board
///
/// <!-- todos-board:auto:begin -->
/// (auto content)
/// <!-- todos-board:auto:end -->
/// ```
///
/// 若 body 中找不到标记，则**不**改 body（与"用户手工删除 marker → 关闭自动化"语义一致）。
pub fn rewrite_todos_board(
    body: &mut String,
    todos: &[crate::core::plan_runtime::file_store::TodoItem],
) {
    const BEGIN: &str = "<!-- todos-board:auto:begin -->";
    const END: &str = "<!-- todos-board:auto:end -->";
    let Some(begin_idx) = body.find(BEGIN) else {
        return;
    };
    let body_after_begin = begin_idx + BEGIN.len();
    let Some(end_rel) = body[body_after_begin..].find(END) else {
        return;
    };
    let end_idx = body_after_begin + end_rel;
    let mut rendered = String::from("\n");
    rendered.push_str("### Todos\n");
    if todos.is_empty() {
        rendered.push_str("_(empty)_\n");
    } else {
        use crate::core::plan_runtime::file_store::TodoStatus;
        for t in todos {
            let checkbox = match t.status {
                TodoStatus::Completed => "x",
                TodoStatus::InProgress => "~",
                TodoStatus::Cancelled => "-",
                TodoStatus::Pending => " ",
            };
            rendered.push_str(&format!("- [{checkbox}] {}: {}\n", t.id, t.content));
        }
    }
    body.replace_range(body_after_begin..end_idx, &rendered);
}
