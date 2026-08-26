//! 内置 skill 的物化。
//!
//! skill discovery 只扫描磁盘目录；把内嵌内容写到 managed root 后，内置 skill 与
//! 用户安装的 skill 共用同一套 frontmatter、权限和 `load_skill` 路径。

use std::path::PathBuf;

use crate::infra::config::get_work_dir;
use crate::infra::error::AppError;
use crate::AppConfig;

const VERIFY_SKILL: &str = include_str!("builtin_verify.md");

pub fn materialize_builtin_skills(cfg: &AppConfig) -> Result<PathBuf, AppError> {
    let path = get_work_dir(cfg)?
        .join("skills")
        .join("verify")
        .join("SKILL.md");
    if std::fs::read_to_string(&path).ok().as_deref() == Some(VERIFY_SKILL) {
        return Ok(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Config("内置 verify skill 路径缺少父目录".into()))?;
    std::fs::create_dir_all(parent)?;
    crate::infra::platform::write_file_atomic(&path, VERIFY_SKILL.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{materialize_builtin_skills, VERIFY_SKILL};
    use crate::core::skill::{discover, load_skill_payload, SkillSource};
    use crate::core::tools::primitive::PrimitiveExecutor;
    use crate::infra::error::AppError;
    use crate::AppConfig;

    struct SkillFilePrimitive;

    #[async_trait::async_trait]
    impl PrimitiveExecutor for SkillFilePrimitive {
        async fn read_file(&self, path: &str, _plugin_id: &str) -> Result<String, AppError> {
            std::fs::read_to_string(path).map_err(AppError::Io)
        }

        async fn list_dir(
            &self,
            _path: &str,
            _plugin_id: &str,
        ) -> Result<Vec<crate::core::tools::primitive::DirEntry>, AppError> {
            unreachable!("the built-in skill load test only reads a file")
        }

        async fn write_file(
            &self,
            _path: &str,
            _content: &str,
            _overwrite: bool,
            _plugin_id: &str,
        ) -> Result<crate::core::tools::primitive::WriteFileResult, AppError> {
            unreachable!("the built-in skill load test only reads a file")
        }

        async fn edit_file(
            &self,
            _path: &str,
            _edits: Vec<crate::core::tools::primitive::EditOperation>,
            _plugin_id: &str,
        ) -> Result<crate::core::tools::primitive::EditFileResult, AppError> {
            unreachable!("the built-in skill load test only reads a file")
        }

        async fn execute_bash(
            &self,
            _command: &str,
            _cwd: Option<&str>,
            _plugin_id: &str,
            _argv: Option<&[String]>,
            _foreground_wait_ms: Option<u64>,
        ) -> Result<crate::core::tools::primitive::BashResult, AppError> {
            unreachable!("the built-in skill load test only reads a file")
        }

        async fn require_user_confirmation(
            &self,
            _operation: crate::core::tools::primitive::PrimitiveOperation,
            _preview: &str,
            _plugin_id: &str,
        ) -> Result<bool, AppError> {
            unreachable!("the built-in skill load test only reads a file")
        }
    }

    #[test]
    fn verify_skill_materialized_content_declares_evidence_contract() {
        assert!(VERIFY_SKILL.contains("name: verify"));
        assert!(VERIFY_SKILL.contains("run_in_background=true"));
        assert!(VERIFY_SKILL.contains("\"green_build_pass\": true"));
        assert!(VERIFY_SKILL.contains("\"command\": \"<the exact background bash command>\""));
    }

    #[tokio::test]
    async fn materialized_verify_skill_is_discovered_and_loadable() {
        let temp = tempfile::tempdir().expect("test temp directory");
        let workspace = temp.path().join("workspace");
        let work_dir = temp.path().join("work");
        std::fs::create_dir_all(&workspace).expect("test workspace");

        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(work_dir.to_string_lossy().into_owned());

        let materialized_path =
            materialize_builtin_skills(&cfg).expect("materialize built-in verify skill");
        let skills = discover(&cfg, &workspace);
        let verify = skills
            .by_name
            .get("verify")
            .expect("discovery should find materialized verify skill");
        assert_eq!(verify.source, SkillSource::Managed);
        assert_eq!(
            std::fs::read_to_string(&verify.file_path).expect("read discovered skill"),
            std::fs::read_to_string(&materialized_path).expect("read materialized skill"),
            "discovery must reference the content materialized into the managed skill root"
        );

        let payload = load_skill_payload(&SkillFilePrimitive, "__test__", verify, None)
            .await
            .expect("load materialized verify skill through the normal loader");
        assert!(payload.contains("<skill name=\"verify\" location=\"SKILL.md\">"));
        assert!(payload.contains("## Green-build verification"));
        assert!(payload.contains("Scale the checks to the change"));
        assert!(!payload.contains("name: verify"));
    }
}
