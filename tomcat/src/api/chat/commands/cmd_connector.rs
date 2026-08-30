use crate::api::chat::ChatContext;
use crate::core::connector::mcp::config::{
    remove_global_server, set_global_tool_filter, upsert_global_server, McpServerConfig, ToolFilter,
};

use super::parse::{ChatCommand, ChatCommandOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorCommand {
    List,
    Add {
        name: String,
        command: String,
        args: Vec<String>,
    },
    Remove {
        name: String,
    },
    Trust {
        name: String,
    },
    Deny {
        name: String,
    },
    Test {
        name: String,
    },
    Reload,
    Tools {
        name: String,
        include: Vec<String>,
        exclude: Vec<String>,
    },
}

pub(crate) fn parse_args(tokens: Vec<String>) -> ChatCommand {
    match tokens.as_slice() {
        [command, sub] if command == "/connector" && sub == "list" => {
            ChatCommand::Connector(ConnectorCommand::List)
        }
        [command, sub, name, rest @ ..] if command == "/connector" && sub == "add" => {
            let Some((program, args)) = rest.split_first() else {
                return usage();
            };
            ChatCommand::Connector(ConnectorCommand::Add {
                name: name.clone(),
                command: program.clone(),
                args: args.to_vec(),
            })
        }
        [command, sub, name] if command == "/connector" && sub == "remove" => {
            ChatCommand::Connector(ConnectorCommand::Remove { name: name.clone() })
        }
        [command, sub, name] if command == "/connector" && sub == "trust" => {
            ChatCommand::Connector(ConnectorCommand::Trust { name: name.clone() })
        }
        [command, sub, name] if command == "/connector" && sub == "deny" => {
            ChatCommand::Connector(ConnectorCommand::Deny { name: name.clone() })
        }
        [command, sub, name] if command == "/connector" && sub == "test" => {
            ChatCommand::Connector(ConnectorCommand::Test { name: name.clone() })
        }
        [command, sub] if command == "/connector" && sub == "reload" => {
            ChatCommand::Connector(ConnectorCommand::Reload)
        }
        [command, sub, name, filter_args @ ..] if command == "/connector" && sub == "tools" => {
            let Some((include, exclude)) = parse_tool_filter(filter_args) else {
                return usage();
            };
            ChatCommand::Connector(ConnectorCommand::Tools {
                name: name.clone(),
                include,
                exclude,
            })
        }
        _ => usage(),
    }
}

pub(crate) async fn run(ctx: &ChatContext, command: ConnectorCommand) -> ChatCommandOutcome {
    match command {
        ConnectorCommand::List => list(ctx),
        ConnectorCommand::Tools {
            name,
            include,
            exclude,
        } => tools(ctx, &name, include, exclude).await,
        ConnectorCommand::Add {
            name,
            command,
            args,
        } => add(ctx, name, command, args).await,
        ConnectorCommand::Remove { name } => remove(ctx, &name).await,
        ConnectorCommand::Trust { name } => trust(ctx, &name).await,
        ConnectorCommand::Deny { name } => deny(ctx, &name),
        ConnectorCommand::Test { name } => test(ctx, &name).await,
        ConnectorCommand::Reload => reload(ctx).await,
    }
}

fn usage() -> ChatCommand {
    ChatCommand::UsageError {
        message: "用法错误：/connector list | add <name> <command> [args...] | remove <name> | trust|deny|test <name> | tools <name> [--include <glob>]... [--exclude <glob>]... | reload".to_string(),
    }
}

fn registry(
    ctx: &ChatContext,
) -> Option<&std::sync::Arc<crate::core::connector::ConnectorRegistry>> {
    ctx.global_services.connector_registry.as_ref()
}

fn list(ctx: &ChatContext) -> ChatCommandOutcome {
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用；在 tomcat.config.toml 设置 [connector] enabled = true。");
        return ChatCommandOutcome::Handled;
    };
    let statuses = registry.mcp_manager().statuses();
    if statuses.is_empty() {
        println!(
            "[connector] 未配置 MCP server（编辑 ~/.tomcat/mcp.json，或 /connector add ...）。"
        );
        return ChatCommandOutcome::Handled;
    }
    println!("MCP servers:");
    for status in statuses {
        println!(
            "  - {} [{}] {:?}; {} tool(s), {} resource(s)",
            status.name,
            status.source.as_str(),
            status.state,
            status.tool_count,
            status.resource_count,
        );
    }
    ChatCommandOutcome::Handled
}

async fn tools(
    ctx: &ChatContext,
    name: &str,
    include: Vec<String>,
    exclude: Vec<String>,
) -> ChatCommandOutcome {
    if !include.is_empty() || !exclude.is_empty() {
        match set_global_tool_filter(&ctx.config, name, ToolFilter { include, exclude }) {
            Ok(()) => {
                if let Some(registry) = registry(ctx) {
                    if let Err(error) = registry.reload().await {
                        println!("[connector] 工具过滤器已写入，但重载失败: {error}");
                    }
                }
            }
            Err(error) => {
                println!("[connector] 更新工具过滤器失败: {error}");
                return ChatCommandOutcome::Handled;
            }
        }
    }
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用。");
        return ChatCommandOutcome::Handled;
    };
    let tools = registry.mcp_manager().tool_defs(name);
    if tools.is_empty() {
        println!("[connector] {name} 暂无 ready 工具；用 /connector list 查看状态。");
        return ChatCommandOutcome::Handled;
    }
    println!("MCP tools for {name}:");
    for tool in tools {
        println!("  - {} ({})", tool.model_name, tool.raw_name);
    }
    ChatCommandOutcome::Handled
}

fn parse_tool_filter(tokens: &[String]) -> Option<(Vec<String>, Vec<String>)> {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let flag = tokens.get(index)?;
        let value = tokens.get(index + 1)?;
        match flag.as_str() {
            "--include" => include.push(value.clone()),
            "--exclude" => exclude.push(value.clone()),
            _ => return None,
        }
        index += 2;
    }
    Some((include, exclude))
}

async fn add(
    ctx: &ChatContext,
    name: String,
    command: String,
    args: Vec<String>,
) -> ChatCommandOutcome {
    let config = McpServerConfig {
        command,
        args,
        env: Default::default(),
        cwd: None,
        trusted: false,
        integrity: None,
        startup_timeout_ms: 30_000,
        call_timeout_ms: 120_000,
        tool_filter: ToolFilter::default(),
    };
    match upsert_global_server(&ctx.config, name.clone(), config) {
        Ok(()) => {
            println!("[connector] 已写入全局 mcp.json: {name}");
            if let Some(registry) = registry(ctx) {
                if let Err(error) = registry.reload().await {
                    println!("[connector] 配置已写入，但重连失败: {error}");
                }
            } else {
                println!("[connector] 启用模块后会自动连接： [connector] enabled = true");
            }
        }
        Err(error) => println!("[connector] 添加失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

async fn remove(ctx: &ChatContext, name: &str) -> ChatCommandOutcome {
    match remove_global_server(&ctx.config, name) {
        Ok(true) => {
            println!("[connector] 已从全局 mcp.json 移除: {name}");
            if let Some(registry) = registry(ctx) {
                if let Err(error) = registry.reload().await {
                    println!("[connector] 配置已移除，但重载失败: {error}");
                }
            }
        }
        Ok(false) => println!("[connector] 全局 mcp.json 中没有: {name}"),
        Err(error) => println!("[connector] 移除失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

async fn trust(ctx: &ChatContext, name: &str) -> ChatCommandOutcome {
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用。");
        return ChatCommandOutcome::Handled;
    };
    match registry.approve_and_connect(name).await {
        Ok(()) => println!("[connector] 已信任并开始连接: {name}"),
        Err(error) => println!("[connector] 信任/连接失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

fn deny(ctx: &ChatContext, name: &str) -> ChatCommandOutcome {
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用。");
        return ChatCommandOutcome::Handled;
    };
    match registry.deny(name) {
        Ok(()) => println!("[connector] 已拒绝并停止: {name}"),
        Err(error) => println!("[connector] 拒绝失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

async fn test(ctx: &ChatContext, name: &str) -> ChatCommandOutcome {
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用。");
        return ChatCommandOutcome::Handled;
    };
    match registry.mcp_manager().reconnect_server(name).await {
        Ok(()) => println!("[connector] 测试连接成功: {name}"),
        Err(error) => println!("[connector] 测试连接失败: {error}"),
    }
    ChatCommandOutcome::Handled
}

async fn reload(ctx: &ChatContext) -> ChatCommandOutcome {
    let Some(registry) = registry(ctx) else {
        println!("[connector] 模块未启用。");
        return ChatCommandOutcome::Handled;
    };
    match registry.reload().await {
        Ok(()) => println!("[connector] 已重读 mcp.json 并重连。"),
        Err(error) => println!("[connector] 重载失败: {error}"),
    }
    ChatCommandOutcome::Handled
}
