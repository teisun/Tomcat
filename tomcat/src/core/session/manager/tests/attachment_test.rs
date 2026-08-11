//! # `SessionManager` 与附件 blob store 的联动测试
//!
//! 覆盖三条只有在 `SessionManager` 这一层才能验证的行为：
//!
//! - `discard_legacy_draft_dir`：升级后一次性丢弃旧版本的后端草稿目录
//! - `any_transcript_references_blob`：GC 判据必须问遍所有会话，
//!   否则内容寻址下会把别的会话还在用的字节删掉
//! - `delete_session`：删会话时联动回收它的未发送字节

use super::super::*;
use super::mocks::temp_sessions_dir;
use crate::core::session::user_message_sidecar::user_message_sidecar_path;

fn fresh_dir() -> std::path::PathBuf {
    let dir = temp_sessions_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn discard_legacy_draft_dir_removes_the_old_backend_drafts() {
    let dir = fresh_dir();
    let legacy = dir.join("drafts");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("sid_old.json"), b"{\"text\":\"unsent\"}").unwrap();

    let mgr = SessionManager::new(dir.clone());
    mgr.discard_legacy_draft_dir();

    assert!(!legacy.exists(), "旧草稿目录必须被整体删除，不做迁移");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_legacy_draft_dir_is_a_no_op_when_there_is_nothing_to_discard() {
    // 绝大多数启动都会走这条路，它必须便宜且安静。
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    mgr.discard_legacy_draft_dir();
    mgr.discard_legacy_draft_dir();
    assert!(dir.exists(), "不得误伤 sessions 目录本身");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn any_transcript_references_blob_looks_across_every_session() {
    // 内容寻址下同一张图被多个会话共享同一份字节。
    // 只看当前会话就会把别人还在用的字节删掉 —— 这条守住那个坑。
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    let shared_sha = "a".repeat(64);
    let orphan_sha = "b".repeat(64);

    std::fs::write(
        dir.join("sid_other.jsonl"),
        format!("{{\"role\":\"user\",\"content\":[{{\"blobSha\":\"{shared_sha}\"}}]}}\n"),
    )
    .unwrap();

    assert!(
        mgr.any_transcript_references_blob(&shared_sha),
        "别的会话仍在引用，必须判为「还有人用」"
    );
    assert!(!mgr.any_transcript_references_blob(&orphan_sha));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn any_transcript_references_blob_ignores_non_transcript_files() {
    // blobs/ 目录下的文件名就是 sha 本身，不能把它当成「有人引用」。
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    let sha = mgr.attachment_store().put(b"some image bytes").unwrap();

    assert!(
        !mgr.any_transcript_references_blob(&sha),
        "blob 文件本身不构成引用，否则任何字节都永远删不掉"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn any_transcript_references_blob_ignores_user_message_sidecar() {
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    let sha = mgr.attachment_store().put(b"sidecar-only bytes").unwrap();
    let session = mgr.create_session(mgr.current_session_key(), None).unwrap();
    std::fs::write(
        user_message_sidecar_path(&mgr.transcript_path(&session.session_id)),
        format!("{{\"message\":{{\"content\":[{{\"blobSha\":\"{sha}\"}}]}}}}\n"),
    )
    .unwrap();

    assert!(
        !mgr.any_transcript_references_blob(&sha),
        "派生 sidecar 不能把已删除 transcript 的附件误判为仍被引用"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_session_reclaims_its_unsent_attachment_bytes() {
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    let entry = mgr.create_session(mgr.current_session_key(), None).unwrap();

    let store = mgr.attachment_store();
    let sha = store.put(b"unsent image bytes").unwrap();
    store.mark_pending(&entry.session_id, &sha).unwrap();
    std::fs::write(
        user_message_sidecar_path(&mgr.transcript_path(&entry.session_id)),
        format!("{{\"message\":{{\"content\":[{{\"blobSha\":\"{sha}\"}}]}}}}\n"),
    )
    .unwrap();

    mgr.delete_session(&entry.session_id).unwrap();

    assert!(
        !store.exists(&sha),
        "会话删除后它未发送的字节应一并回收，否则草稿字节会永久泄漏"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_session_keeps_bytes_another_session_still_references() {
    let dir = fresh_dir();
    let mgr = SessionManager::new(dir.clone());
    let doomed = mgr.create_session(mgr.current_session_key(), None).unwrap();

    let store = mgr.attachment_store();
    let sha = store.put(b"shared image bytes").unwrap();
    store.mark_pending(&doomed.session_id, &sha).unwrap();
    // 另一个会话已经把这张图发出去了，字节写进了它的 transcript。
    std::fs::write(
        dir.join("sid_keeper.jsonl"),
        format!("{{\"role\":\"user\",\"content\":[{{\"blobSha\":\"{sha}\"}}]}}\n"),
    )
    .unwrap();

    mgr.delete_session(&doomed.session_id).unwrap();

    assert!(
        store.exists(&sha),
        "别的会话还在引用这份字节，删会话不得把它带走"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
