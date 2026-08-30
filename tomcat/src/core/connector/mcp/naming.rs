use sha2::{Digest, Sha256};

const MAX_TOOL_NAME_LEN: usize = 64;

pub fn to_model_name(server: &str, raw_tool: &str) -> String {
    let readable = format!(
        "mcp__{}__{}",
        sanitize_component(server),
        sanitize_component(raw_tool)
    );
    if readable.len() <= MAX_TOOL_NAME_LEN {
        return readable;
    }

    let hash = short_hash(&format!("{server}\0{raw_tool}"));
    let suffix = format!("_{hash}");
    let keep = MAX_TOOL_NAME_LEN.saturating_sub(suffix.len());
    format!("{}{}", truncate_ascii(&readable, keep), suffix)
}

fn sanitize_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "unnamed".to_string()
    } else {
        output
    }
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn truncate_ascii(value: &str, len: usize) -> &str {
    &value[..value
        .char_indices()
        .nth(len)
        .map_or(value.len(), |(index, _)| index)]
}

#[cfg(test)]
mod tests {
    use super::to_model_name;

    #[test]
    fn short_names_remain_readable() {
        assert_eq!(
            to_model_name("playwright", "browser_take_screenshot"),
            "mcp__playwright__browser_take_screenshot"
        );
    }

    #[test]
    fn long_names_keep_a_readable_head_and_unique_hash() {
        let name = to_model_name(
            "playwright",
            "navigate_and_wait_for_network_idle_then_capture_full_screenshot",
        );
        assert!(name.starts_with("mcp__playwright__navigate_and_wait"));
        assert_eq!(name.len(), 64);
        assert_ne!(
            name,
            to_model_name(
                "playwright",
                "navigate_and_wait_for_network_idle_then_capture_viewport_screenshot",
            )
        );
    }

    #[test]
    fn underscore_in_server_and_tool_names_is_preserved() {
        assert_eq!(
            to_model_name("browser_tools", "take_screenshot"),
            "mcp__browser_tools__take_screenshot"
        );
    }
}
