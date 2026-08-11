use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::prompts::{load as load_prompt, render as render_prompt, PromptKey};

use super::{
    review::{Finding, ParsedReview},
    DisputedFinding,
};

pub const CODE_REVIEWER_ALLOWED_TOOLS: &[&str] = &["read", "search_files", "list_dir", "bash"];

pub fn code_reviewer_allowed_tools_with_policy(expose_skills: bool) -> Vec<&'static str> {
    let mut tools = CODE_REVIEWER_ALLOWED_TOOLS.to_vec();
    if expose_skills {
        tools.push("load_skill");
    }
    tools
}

pub fn code_review_system_prompt_text() -> &'static str {
    load_prompt(PromptKey::ReviewerCode)
}

/// 上一轮未清 finding 渲染成 prompt 片段，要求 reviewer 逐条核销。
fn render_open_findings_section(open_findings: &[Finding]) -> String {
    if open_findings.is_empty() {
        return String::new();
    }
    let lines = open_findings
        .iter()
        .map(|f| format!("         - [{}] {}: {}", f.severity, f.area, f.note))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "         Open findings from the previous review round (verify each one first):\n\
         {lines}\n\
         - Verify each item against the current code. Report it again ONLY if it is still unfixed.\n\
         - Do not re-report an issue you confirmed fixed.\n"
    )
}

fn render_adjudicated_section(disputed_findings: &[DisputedFinding]) -> String {
    if disputed_findings.is_empty() {
        return String::new();
    }
    let lines = disputed_findings
        .iter()
        .map(|finding| {
            format!(
                "         - [{}] {}: {} — accepted trade-off: {}",
                finding.severity, finding.area, finding.note, finding.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "         Known accepted trade-offs (do NOT re-flag these, including with different wording):\n\
         {lines}\n"
    )
}

/// 构建 code-review 首轮请求的输入集合，避免 prompt 组装继续堆叠位置参数。
pub(crate) struct CodeReviewPromptInput<'a> {
    pub plan_id: &'a str,
    pub plan_text: &'a str,
    pub plan_path: &'a Path,
    pub workspace_root: Option<&'a Path>,
    pub diff_stat: &'a str,
    pub changed_files: &'a [String],
    pub open_findings: &'a [Finding],
    pub disputed_findings: &'a [DisputedFinding],
}

pub(crate) fn build_code_review_prompt(input: CodeReviewPromptInput<'_>) -> String {
    let CodeReviewPromptInput {
        plan_id,
        plan_text,
        plan_path,
        workspace_root,
        diff_stat,
        changed_files,
        open_findings,
        disputed_findings,
    } = input;
    let plan_path = crate::infra::platform::format_home_path(plan_path);
    let workspace_hint = workspace_root
        .map(|path| {
            format!(
                "         - Project/workspace root (start repo inspection here first): `{}`\n\
                 - Access note: reads and bash still follow runtime authorization / permission rules.\n",
                crate::infra::platform::format_home_path(path)
            )
        })
        .unwrap_or_default();
    let diff_section = if diff_stat.trim().is_empty() {
        "         Runtime git diff summary: unavailable (git diff injection failed or found no tracked changes).\n".to_string()
    } else {
        format!(
            "         Runtime git diff summary (`git diff --stat HEAD`):\n\
             ```text\n{diff_stat}\n```\n"
        )
    };
    let changed_files_section = if changed_files.is_empty() {
        "         Runtime changed files list: unavailable.\n".to_string()
    } else {
        let joined = changed_files
            .iter()
            .take(80)
            .map(|path| format!("         - `{path}`"))
            .collect::<Vec<_>>()
            .join("\n");
        let suffix = if changed_files.len() > 80 {
            format!(
                "\n         - ... {} more file(s) omitted",
                changed_files.len() - 80
            )
        } else {
            String::new()
        };
        format!("         Runtime changed files list:\n{joined}{suffix}\n")
    };
    render_prompt(
        PromptKey::ReviewerCodeBrief,
        &[
            ("plan_id", plan_id),
            ("plan_path", &plan_path),
            ("workspace_hint", &workspace_hint),
            ("diff_section", &diff_section),
            ("changed_files_section", &changed_files_section),
            (
                "open_findings_section",
                &render_open_findings_section(open_findings),
            ),
            (
                "adjudicated_findings_section",
                &render_adjudicated_section(disputed_findings),
            ),
            ("plan_text", plan_text),
        ],
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeReviewSummary {
    pub aborted: bool,
    #[serde(default)]
    pub verdict: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub changes_summary: String,
    pub applied_changes: bool,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub reviewer_turns_used: u32,
    #[serde(default)]
    pub reviewer_turns_limit: u32,
    #[serde(default)]
    pub reviewer_stop_reason: String,
    #[serde(default)]
    pub child_session_id: String,
}

impl CodeReviewSummary {
    pub fn placeholder_pending() -> Self {
        Self {
            aborted: true,
            verdict: Some("aborted".into()),
            summary: "reviewer 子 Agent 将在 P4 接入；当前阶段返回 aborted 占位".into(),
            changes_summary: "none".into(),
            applied_changes: false,
            findings: Vec::new(),
            reviewer_turns_used: 0,
            reviewer_turns_limit: 0,
            reviewer_stop_reason: "not_dispatched".into(),
            child_session_id: String::new(),
        }
    }

    pub fn aborted_with(reason: impl Into<String>) -> Self {
        Self {
            aborted: true,
            verdict: Some("aborted".into()),
            summary: reason.into(),
            changes_summary: "none".into(),
            applied_changes: false,
            findings: Vec::new(),
            reviewer_turns_used: 0,
            reviewer_turns_limit: 0,
            reviewer_stop_reason: "aborted".into(),
            child_session_id: String::new(),
        }
    }

    pub fn from_parsed(parsed: ParsedReview) -> Self {
        Self {
            aborted: false,
            verdict: parsed.verdict,
            summary: parsed.summary,
            changes_summary: parsed.changes_summary,
            applied_changes: parsed.applied_changes,
            findings: parsed.findings,
            reviewer_turns_used: 0,
            reviewer_turns_limit: 0,
            reviewer_stop_reason: "completed".into(),
            child_session_id: String::new(),
        }
    }

    pub fn normalize_for_result(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.aborted {
            if self.verdict.as_deref() != Some("aborted") {
                self.verdict = Some("aborted".into());
                warnings.push("code review 中止，verdict 已规范化为 aborted".into());
            }
            self.applied_changes = false;
            return warnings;
        }

        match self.verdict.clone() {
            Some(verdict)
                if matches!(verdict.as_str(), "pass" | "fail" | "partial" | "aborted") => {}
            Some(other) => {
                self.verdict = Some("aborted".into());
                warnings.push(format!(
                    "code review verdict `{other}` 非法，已规范化为 aborted"
                ));
            }
            None => {
                self.verdict = Some("partial".into());
                warnings.push("code review 未返回 verdict，已规范化为 partial".into());
            }
        }

        if self.applied_changes {
            self.applied_changes = false;
            warnings.push("code reviewer 不允许直接改动，applied_changes 已重置为 false".into());
        }
        if self.changes_summary.trim().is_empty() {
            self.changes_summary = "none".into();
            warnings.push("code review 未返回 changes_summary，已规范化为 none".into());
        }

        warnings
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "aborted": self.aborted,
            "verdict": self.verdict,
            "summary": self.summary,
            "changes_summary": self.changes_summary,
            "applied_changes": self.applied_changes,
            "findings": self.findings,
            "reviewer_turns_used": self.reviewer_turns_used,
            "reviewer_turns_limit": self.reviewer_turns_limit,
            "reviewer_stop_reason": self.reviewer_stop_reason,
            "child_session_id": self.child_session_id,
        })
    }
}

pub async fn collect_git_diff_context(workspace_root: &std::path::Path) -> (String, Vec<String>) {
    use std::collections::BTreeSet;

    let diff_stat = run_git_capture(workspace_root, &["diff", "--stat", "--no-ext-diff", "HEAD"])
        .await
        .unwrap_or_default();

    let mut changed_files = BTreeSet::new();
    for line in run_git_lines(
        workspace_root,
        &["diff", "--name-only", "--no-ext-diff", "HEAD"],
    )
    .await
    {
        if !line.is_empty() {
            changed_files.insert(line);
        }
    }
    for line in run_git_lines(
        workspace_root,
        &["ls-files", "--others", "--exclude-standard"],
    )
    .await
    {
        if !line.is_empty() {
            changed_files.insert(line);
        }
    }

    (diff_stat, changed_files.into_iter().collect())
}

/// 当前 Git diff 中的代码文件和其中最新的文件修改时间。
///
/// 不读取 diff 正文、更不计算内容哈希：`git --name-only` + 文件 metadata 足以把
/// 「验收前又改过代码」和「仍是同一份代码」区分开。删除的代码文件没有 mtime，
/// 用当前时间作为保守下界，迫使后续验收在删除之后启动。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeDiffContext {
    pub changed_code_files: Vec<String>,
    pub newest_edit_mtime_ms: Option<u128>,
}

pub async fn collect_code_diff_context(workspace_root: &std::path::Path) -> CodeDiffContext {
    let (_, changed_files) = collect_git_diff_context(workspace_root).await;
    let changed_code_files: Vec<String> = changed_files
        .into_iter()
        .filter(|path| is_code_path(path))
        .collect();
    if changed_code_files.is_empty() {
        return CodeDiffContext::default();
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let newest_edit_mtime_ms = changed_code_files
        .iter()
        .filter_map(|relative| std::fs::metadata(workspace_root.join(relative)).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .filter_map(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .max()
        .or(Some(now_ms));

    CodeDiffContext {
        changed_code_files,
        newest_edit_mtime_ms,
    }
}

fn is_code_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "mjs"
                    | "cjs"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "kts"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "cxx"
                    | "h"
                    | "hpp"
                    | "cs"
                    | "rb"
                    | "php"
                    | "swift"
                    | "scala"
                    | "sh"
                    | "sql"
                    | "vue"
                    | "svelte"
            )
        })
}

async fn run_git_capture(workspace_root: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(args)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_git_lines(workspace_root: &std::path::Path, args: &[&str]) -> Vec<String> {
    run_git_capture(workspace_root, args)
        .await
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
