//! Provider-agnostic prompt-cache affinity keys.
//!
//! A key is scoped to one session and one request family. Families must remain
//! distinct because their stable prompt prefixes are intentionally different.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptCacheKeyFamily<'a> {
    Main,
    Subagent(&'a str),
    Compaction,
    Title,
    Extension,
}

impl PromptCacheKeyFamily<'_> {
    pub fn key_for(self, session_id: &str) -> Option<String> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return None;
        }
        let family = match self {
            Self::Main => "main".to_string(),
            Self::Subagent(kind) => format!("subagent:{kind}"),
            Self::Compaction => "compaction".to_string(),
            Self::Title => "title".to_string(),
            Self::Extension => "ext".to_string(),
        };
        Some(format!("{session_id}:{family}"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::PromptCacheKeyFamily;

    #[test]
    fn cache_key_families_are_mutually_distinct() {
        let families = [
            PromptCacheKeyFamily::Main,
            PromptCacheKeyFamily::Subagent("plan_reviewer"),
            PromptCacheKeyFamily::Compaction,
            PromptCacheKeyFamily::Title,
            PromptCacheKeyFamily::Extension,
        ];
        let keys = families
            .into_iter()
            .map(|family| family.key_for("session-1").expect("non-empty session id"))
            .collect::<HashSet<_>>();
        assert_eq!(keys.len(), families.len());
    }

    #[test]
    fn cache_key_is_none_without_session_id() {
        assert_eq!(PromptCacheKeyFamily::Main.key_for("  "), None);
    }
}
