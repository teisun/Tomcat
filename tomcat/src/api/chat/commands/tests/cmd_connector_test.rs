use crate::api::chat::commands::{parse_chat_command, ChatCommand, ConnectorCommand};

#[test]
fn parses_connector_add_with_command_and_args() {
    assert_eq!(
        parse_chat_command(r#"/connector add playwright npx -y @playwright/mcp@0.0.79"#),
        ChatCommand::Connector(ConnectorCommand::Add {
            name: "playwright".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@playwright/mcp@0.0.79".to_string()],
        })
    );
}

#[test]
fn parses_connector_tool_filter_flags() {
    assert_eq!(
        parse_chat_command(
            r#"/connector tools playwright --include "browser_*" --exclude "browser_save_as_pdf""#
        ),
        ChatCommand::Connector(ConnectorCommand::Tools {
            name: "playwright".to_string(),
            include: vec!["browser_*".to_string()],
            exclude: vec!["browser_save_as_pdf".to_string()],
        })
    );
}

#[test]
fn connector_rejects_invalid_subcommands() {
    assert!(matches!(
        parse_chat_command("/connector add"),
        ChatCommand::UsageError { .. }
    ));
}
