use super::super::args::{has_real_edits, parse_edit_args, parse_edit_ops};
use super::super::edit_sim::simulate_apply_edits;
use super::super::guard::check_mutation_stamp;
use super::super::{ToolDisplay, ToolExecCtx, AGENT_PLUGIN_ID};
use crate::core::tools::primitive::EditOperation;
use crate::infra::events::{ToolDisplayFileEntry, ToolDisplayFileStatus};

/// 一次批量 edit 最多几个文件。上限存在的理由是可读性：一屏能看完的失败列表才有人会看。
const MAX_BATCH_EDIT_FILES: usize = 10;

pub(in super::super) async fn handle_edit(
    ctx: &ToolExecCtx<'_>,
    args: &serde_json::Value,
    display_out: &mut Option<ToolDisplay>,
) -> Result<String, String> {
    if let Some(files) = parse_batch_files(args)? {
        return edit_batch(ctx, files, display_out).await;
    }

    let (path, edits) = parse_edit_args(args)?;
    precheck_file(ctx, path)?;
    reviewer_body_guard(ctx, path, &edits)?;

    ctx.primitive
        .edit_file_with_cancel(path, edits, ctx.cancel, AGENT_PLUGIN_ID)
        .await
        .map(|r| {
            if r.applied {
                *display_out = Some(ToolDisplay::File {
                    file: r.path.clone(),
                    added: r.added,
                    removed: r.removed,
                    diff: r.diff.clone(),
                });
                format!("已编辑: {}", r.path)
            } else {
                let msg = format!("编辑被拒绝: {}", r.path);
                *display_out = Some(ToolDisplay::Text { text: msg.clone() });
                msg
            }
        })
        .map_err(|e| e.to_string())
}

struct BatchFile {
    path: String,
    edits: Vec<EditOperation>,
}

/// 顶层 `path`/`edits` 里是否还有一段 `files` 中不存在的真实编辑。
///
/// 有才是真歧义。实测模型在批量编辑时会把整批编辑**原样抄一份**放进顶层槽位（`path`
/// 填第一个文件），那是同一个意图写了两遍，不是两套意图 —— 按歧义拒掉只是白白罚一次
/// 往返。反过来，顶层真有一段别处没有的编辑，就必须问清楚：edit 会落盘，猜错要写坏文件。
fn top_level_edit_outside_files(
    args: &serde_json::Value,
    files: &[&serde_json::Value],
) -> Option<String> {
    let known: std::collections::BTreeSet<(String, String)> = files
        .iter()
        .flat_map(|file| super::super::args::edit_segments(file))
        .map(super::super::args::segment_identity)
        .collect();

    super::super::args::edit_segments(args)
        .into_iter()
        .map(super::super::args::segment_identity)
        .find(|identity| identity.0 != identity.1 && !known.contains(identity))
        .map(|(old, _)| old)
}

/// `files` 里的空壳条目：既没有路径也没有编辑内容。
fn is_placeholder_file(item: &serde_json::Value) -> bool {
    let no_path = item
        .get("path")
        .and_then(|v| v.as_str())
        .is_none_or(|p| p.trim().is_empty());
    no_path && !has_real_edits(item)
}

/// 解析 Shape C `{ files: [{ path, edits }] }`。返回 `None` 表示这是单文件调用。
///
/// 形态由「内容在哪」决定，不由「哪个键出现过」决定：strict schema 下模型会把没用到
/// 的那一套字段也填上空值，两边都按「出现即选中」来判，任何一种形态都过不了互斥校验。
fn parse_batch_files(args: &serde_json::Value) -> Result<Option<Vec<BatchFile>>, String> {
    let Some(raw) = args.get("files") else {
        return Ok(None);
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| "edit: `files` 必须是数组".to_string())?;
    let raw: Vec<&serde_json::Value> = raw
        .iter()
        .filter(|item| !is_placeholder_file(item))
        .collect();
    if raw.is_empty() {
        // 空壳 `files`：这其实是一次单文件调用，交给下面的 Shape A/B 分支。
        return Ok(None);
    }
    if raw.len() > MAX_BATCH_EDIT_FILES {
        return Err(format!(
            "edit: 一次最多批量修改 {MAX_BATCH_EDIT_FILES} 个文件，收到 {}",
            raw.len()
        ));
    }
    if let Some(conflict) = top_level_edit_outside_files(args, &raw) {
        return Err(format!(
            "edit: 顶层还带了一段 `files` 里没有的编辑（改 `{}`）。\
             批量改多个文件时请只用 `files`，把这一段也放进去",
            conflict.chars().take(40).collect::<String>()
        ));
    }

    let mut files = Vec::with_capacity(raw.len());
    let mut seen = std::collections::BTreeSet::new();
    for (idx, item) in raw.iter().enumerate() {
        let path = item
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| format!("edit: files[{idx}] 缺少非空的 `path`"))?;
        // 同一文件出现两次时，第二条看到的是第一条改完的内容，old_content 多半对不上；
        // 与其让它以 NotFound 失败，不如直接要求合并。
        if !seen.insert(path.to_string()) {
            return Err(format!(
                "edit: files 里 `{path}` 出现多次，请把它的编辑段合并成一条"
            ));
        }
        let edits = parse_edit_ops(item, &format!("files[{idx}]"))?;
        files.push(BatchFile {
            path: path.to_string(),
            edits,
        });
    }
    Ok(Some(files))
}

/// 三段式：先全量预检（不提前退出，把所有问题一次报全），再逐文件落盘（文件内原子），
/// 最后逐文件回报并显式说明磁盘现在是什么状态。
async fn edit_batch(
    ctx: &ToolExecCtx<'_>,
    files: Vec<BatchFile>,
    display_out: &mut Option<ToolDisplay>,
) -> Result<String, String> {
    // 阶段 1：全量预检。一个文件不通过不影响其余文件继续被检查。
    let mut precheck_errors: std::collections::BTreeMap<usize, String> =
        std::collections::BTreeMap::new();
    for (idx, file) in files.iter().enumerate() {
        if let Err(err) = precheck_file(ctx, &file.path)
            .and_then(|()| reviewer_body_guard(ctx, &file.path, &file.edits))
        {
            precheck_errors.insert(idx, err);
        }
    }

    // 阶段 2：只落盘预检通过的文件。primitive 内部对单个文件是先算后写的，
    // 所以某个文件即便在这里失败，它自己也不会留下半截内容。
    let mut entries: Vec<ToolDisplayFileEntry> = Vec::with_capacity(files.len());
    for (idx, file) in files.into_iter().enumerate() {
        if let Some(err) = precheck_errors.remove(&idx) {
            entries.push(failed_entry(file.path, err));
            continue;
        }
        match ctx
            .primitive
            .edit_file_with_cancel(&file.path, file.edits, ctx.cancel, AGENT_PLUGIN_ID)
            .await
        {
            Ok(result) if result.applied => {
                refresh_read_stamp(ctx, &file.path);
                entries.push(ToolDisplayFileEntry {
                    file: result.path,
                    added: result.added,
                    removed: result.removed,
                    diff: result.diff,
                    range: None,
                    status: Some(ToolDisplayFileStatus::Applied),
                    note: None,
                });
            }
            Ok(result) => entries.push(failed_entry(result.path, "编辑被拒绝".to_string())),
            Err(err) => entries.push(failed_entry(file.path, err.to_string())),
        }
    }

    // 阶段 3：逐文件回报。磁盘状态必须写死在文案里 —— 批量最容易误解的就是
    // 「报了错，那是不是一个都没改？」
    let applied = entries
        .iter()
        .filter(|e| e.status == Some(ToolDisplayFileStatus::Applied))
        .count();
    let failed = entries.len() - applied;
    let summary = if failed == 0 {
        format!("已编辑 {applied} 个文件，全部落盘")
    } else {
        format!(
            "{} 个文件已落盘，{failed} 个失败且未写入；失败的文件磁盘内容保持原样",
            applied
        )
    };

    let mut text = summary.clone();
    for entry in &entries {
        if entry.status == Some(ToolDisplayFileStatus::Applied) {
            text.push_str(&format!(
                "\n- APPLIED {} (+{} -{})",
                entry.file,
                entry.added.unwrap_or(0),
                entry.removed.unwrap_or(0)
            ));
        } else {
            text.push_str(&format!(
                "\n- FAILED  {}: {}",
                entry.file,
                entry.note.as_deref().unwrap_or("未知错误")
            ));
        }
    }

    *display_out = Some(display_for_entries(summary, entries));
    Ok(text)
}

fn display_for_entries(summary: String, mut entries: Vec<ToolDisplayFileEntry>) -> ToolDisplay {
    if entries.len() == 1 {
        let entry = entries.pop().expect("len checked");
        return ToolDisplay::File {
            file: entry.file,
            added: entry.added,
            removed: entry.removed,
            diff: entry.diff,
        };
    }
    ToolDisplay::Files {
        summary,
        files: entries,
    }
}

fn failed_entry(file: String, error: String) -> ToolDisplayFileEntry {
    ToolDisplayFileEntry {
        file,
        added: None,
        removed: None,
        diff: None,
        range: None,
        status: Some(ToolDisplayFileStatus::Failed),
        note: Some(error),
    }
}

/// 落盘之后立刻用新内容刷新 ReadStamp，否则同一回合里对刚改过的文件再 edit 会被
/// 「Stale：自上次 read 后已被修改」挡下 —— 而那次修改正是我们自己做的。
fn refresh_read_stamp(ctx: &ToolExecCtx<'_>, path: &str) {
    let Some(state) = ctx.read_file_state else {
        return;
    };
    let Ok(resolved) = crate::infra::platform::normalize_path(path) else {
        return;
    };
    let Ok(meta) = std::fs::metadata(&resolved) else {
        return;
    };
    if meta.is_dir() {
        return;
    }
    let content = std::fs::read(&resolved).unwrap_or_default();
    state.put(
        resolved,
        crate::core::tools::pipeline::read_state::ReadStamp {
            mtime_ms: crate::core::tools::pipeline::read_state::metadata_mtime_ms(&meta),
            size: meta.len(),
            content_hash: crate::core::tools::pipeline::read_state::hash_content(&content),
            offset: None,
            limit: None,
            is_partial_view: false,
            // 落盘后的正文模型并没有见过（它看到的是 diff），所以这条 stamp 只用来放行
            // 后续的 edit，不能拿来短路 read —— 那会是「你已经读过了」的假话。
            covered_lines: None,
            reached_eof: false,
            tool_call_id: Some(ctx.tool_call_id.to_string()),
        },
    );
}

fn precheck_file(ctx: &ToolExecCtx<'_>, path: &str) -> Result<(), String> {
    if crate::core::tools::pipeline::edit_normalize::is_unsupported_structured_file(path) {
        return Err(format!(
            "Notebook: `{}` 是 Jupyter 笔记本（.ipynb），edit 不支持；请使用专用 nbformat 工具或先把目标 cell 导出为 .py / .md 再 edit",
            path
        ));
    }
    if let Some(state) = ctx.read_file_state {
        check_mutation_stamp(state, path, "edit")?;
    }
    Ok(())
}

fn reviewer_body_guard(
    ctx: &ToolExecCtx<'_>,
    path: &str,
    edits: &[EditOperation],
) -> Result<(), String> {
    if ctx.subagent_type != crate::core::agent_loop::types::SubagentType::PlanReviewer {
        return Ok(());
    }
    let normalized_path = crate::infra::platform::normalize_path(path)
        .map_err(|e| format!("reviewer edit 预检路径解析失败：{e}"))?;
    let old = std::fs::read_to_string(&normalized_path)
        .map_err(|e| format!("reviewer edit 预检读原文失败：{e}"))?;
    let new = simulate_apply_edits(&old, edits);
    crate::core::plan_runtime::safety::reviewer_body_diff_guard(&old, &new)
        .map_err(|denied| format!("reviewer edit 被拒：{denied}"))
}
