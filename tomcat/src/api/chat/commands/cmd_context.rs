use crate::api::chat::ChatContext;
use crate::{AppError, ModelPrefsStore};

use super::parse::{ChatCommand, ChatCommandOutcome};

pub(crate) fn parse_args(tokens: Vec<String>) -> ChatCommand {
    match tokens.as_slice() {
        [_cmd, value] => match parse_context_window(value) {
            Some(context_window) => ChatCommand::Context { context_window },
            None => usage_error(value),
        },
        [_cmd] => ChatCommand::UsageError {
            message: "用法错误：/context 需要一个正整数档位，例如 /context 400000。".to_string(),
        },
        _ => ChatCommand::UsageError {
            message: "用法错误：/context 仅支持一个正整数档位，例如 /context 400000。".to_string(),
        },
    }
}

pub(crate) fn parse_context_window(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().filter(|window| *window > 0)
}

pub(crate) fn apply_context_window(
    store: &ModelPrefsStore,
    model: &str,
    context_window: u32,
) -> Result<(), AppError> {
    store.set_context_window(model, Some(context_window))
}

pub(crate) fn run(ctx: &ChatContext, context_window: u32) -> ChatCommandOutcome {
    let session = match ctx
        .session_runtime
        .session
        .get_session(ctx.session_runtime.session.current_session_key())
    {
        Ok(session) => session,
        Err(error) => {
            println!("[context] 读取当前会话失败: {error}");
            return ChatCommandOutcome::Handled;
        }
    };
    let model = ctx.effective_model(session.as_ref());
    let entry = match ctx.global_services.model_catalog.lookup_explicit(&model) {
        Ok(entry) => entry,
        Err(error) => {
            println!("[context] {error}");
            return ChatCommandOutcome::Handled;
        }
    };
    if entry.context_window_options.is_empty()
        || !entry.context_window_options.contains(&context_window)
    {
        println!(
            "[context] 设置失败：模型 {model} 可选档位为 {:?}。",
            entry.context_window_options
        );
        return ChatCommandOutcome::Handled;
    }

    match apply_context_window(&ctx.global_services.model_prefs, &model, context_window) {
        Ok(()) => println!("[context] 模型 {model} 的 Context 档位已设为 {context_window}"),
        Err(error) => println!("[context] 设置失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

fn usage_error(value: &str) -> ChatCommand {
    ChatCommand::UsageError {
        message: format!(
            "用法错误：/context 仅支持一个正整数档位，例如 /context 400000，收到 `{value}`。"
        ),
    }
}
