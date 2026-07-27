use super::common::*;

#[tokio::test]
async fn code_review_pass_completes_without_verifier() {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let captured: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    {
        let sink = std::sync::Arc::clone(&captured);
        rt.attach_transcript_appender(std::sync::Arc::new(move |extra| {
            sink.lock().push(extra);
            Ok(())
        }));
    }
    rt.attach_code_reviewer(std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![
        CodeReviewSummary {
            aborted: false,
            verdict: Some("pass".into()),
            summary: "code review passed".into(),
            changes_summary: "none".into(),
            applied_changes: false,
            findings: vec![crate::core::plan_runtime::review::Finding::new(
                "suggestion".into(),
                "tests".into(),
                "nice to have".into(),
            )],
            ..Default::default()
        },
    ])));
    let verifier = std::sync::Arc::new(MockVerifierDispatcher::new(vec![ok_verify_pass()]));
    rt.attach_verifier(verifier.clone());
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(1);

    let out = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![
                update_plan::UpdateOp::SetStatus {
                    id: "t1".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
                update_plan::UpdateOp::SetStatus {
                    id: "t2".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
            ],
        },
    )
    .await
    .unwrap();

    assert_eq!(out["code_review"]["verdict"], "pass");
    assert_eq!(out["code_review"]["findings"][0]["area"], "tests");
    assert_eq!(out["plan_state_after"], "completed");
    assert!(out.get("verify").is_none());
    assert_eq!(rt.code_review_rounds(&plan_id), 1);
    assert_eq!(
        verifier
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let persisted = read_plan(&plan_path_for_id(&plan_id).unwrap()).unwrap();
    assert_eq!(persisted.frontmatter.state, PlanFileState::Completed);
    assert!(matches!(rt.mode(), PlanState::Chat));
    let events = captured.lock();
    let code_review_event = events
        .iter()
        .find(|v| v["event"] == "plan.code_review")
        .expect("缺少 plan.code_review");
    assert_eq!(code_review_event["verdict"], "pass");
    assert_eq!(code_review_event["findings"][0]["area"], "tests");
    assert_eq!(code_review_event["rounds"], 1);
    cleanup_home(&home);
}

/// D1-b 后门 1：reviewer 中止拿不到通过结论，绝不能当作通过收工。
#[tokio::test]
async fn aborted_code_review_keeps_plan_executing() {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let reviewer = std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![aborted_code_review(
        "reviewer spawn failed",
    )]));
    let verifier = std::sync::Arc::new(MockVerifierDispatcher::new(vec![ok_verify_pass()]));
    rt.attach_code_reviewer(reviewer.clone());
    rt.attach_verifier(verifier.clone());
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(1);

    let out = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![
                update_plan::UpdateOp::SetStatus {
                    id: "t1".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
                update_plan::UpdateOp::SetStatus {
                    id: "t2".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
            ],
        },
    )
    .await
    .unwrap();

    assert_eq!(out["code_review"]["verdict"], "aborted");
    assert_eq!(out["plan_state_after"], "executing");
    assert!(out.get("verify").is_none());
    let persisted = read_plan(&plan_path_for_id(&plan_id).unwrap()).unwrap();
    assert_eq!(persisted.frontmatter.state, PlanFileState::Executing);
    assert_eq!(
        reviewer
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        verifier
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let warnings = out["warnings"].as_array().expect("warnings array");
    assert!(
        warnings.iter().any(|warning| {
            warning
                .as_str()
                .is_some_and(|text| text.contains("aborted"))
        }),
        "aborted code review 应明确说明未拿到通过结论"
    );
    cleanup_home(&home);
}

#[tokio::test]
async fn code_review_non_pass_returns_to_main_and_rounds_exhaustion_hands_back(
) {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let captured: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    {
        let sink = std::sync::Arc::clone(&captured);
        rt.attach_transcript_appender(std::sync::Arc::new(move |extra| {
            sink.lock().push(extra);
            Ok(())
        }));
    }
    let reviewer = std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![CodeReviewSummary {
        aborted: false,
        verdict: Some("fail".into()),
        summary: "code review found a concrete issue".into(),
        changes_summary: "none".into(),
        applied_changes: false,
        findings: vec![crate::core::plan_runtime::review::Finding::new(
            "concern".into(),
            "logic".into(),
            "missing guard".into(),
        )],
        ..Default::default()
    }]));
    let verifier = std::sync::Arc::new(MockVerifierDispatcher::new(vec![ok_verify_pass()]));
    rt.attach_code_reviewer(reviewer.clone());
    rt.attach_verifier(verifier.clone());
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(1);

    let first = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![
                update_plan::UpdateOp::SetStatus {
                    id: "t1".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
                update_plan::UpdateOp::SetStatus {
                    id: "t2".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(first["code_review"]["verdict"], "fail");
    assert_eq!(first["plan_state_after"], "executing");
    assert!(first.get("verify").is_none());
    assert_eq!(first["code_review"]["findings"][0]["note"], "missing guard");
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    assert!(first["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| !item["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("cr_fix_")));
    assert_eq!(
        reviewer
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        verifier
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let warnings = first["warnings"].as_array().expect("warnings array");
    assert!(warnings.iter().any(|warning| warning
        .as_str()
        .is_some_and(|text| text.contains("重新打开一个已有 todo"))));
    assert!(warnings.iter().any(|warning| warning
        .as_str()
        .is_some_and(|text| text.contains("新增一个修复 todo"))));

    let reopen = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![update_plan::UpdateOp::SetStatus {
                id: "t1".into(),
                content: None,
                status: TodoStatus::InProgress,
            }],
        },
    )
    .await
    .unwrap();
    assert!(reopen.get("verify").is_none());

    let second = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![update_plan::UpdateOp::SetStatus {
                id: "t1".into(),
                content: None,
                status: TodoStatus::Completed,
            }],
        },
    )
    .await
    .unwrap();
    assert!(second["code_review"].is_null());
    assert!(second.get("verify").is_none());
    // D1-b 后门 2：轮数用尽不是收口的理由。
    assert_eq!(second["plan_state_after"], "executing");
    let persisted = read_plan(&plan_path_for_id(&plan_id).unwrap()).unwrap();
    assert_eq!(persisted.frontmatter.state, PlanFileState::Executing);
    assert_eq!(rt.code_review_rounds(&plan_id), 1);
    assert_eq!(
        reviewer
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        verifier
            .call_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let second_warnings = second["warnings"].as_array().expect("warnings array");
    assert!(second_warnings.iter().any(|warning| warning
        .as_str()
        .is_some_and(|text| text.contains("轮次预算已用尽"))));

    let events = captured.lock();
    let exhausted = events
        .iter()
        .find(|v| v["event"] == "plan.code_review.exhausted")
        .expect("缺少 plan.code_review.exhausted");
    assert_eq!(exhausted["rounds"], 1);
    assert_eq!(
        exhausted["unresolved_findings"]
            .as_array()
            .expect("unresolved_findings array")
            .len(),
        1
    );
    assert!(
        !events.iter().any(|v| v["event"] == "plan.complete"),
        "轮数用尽不得写 plan.complete"
    );
    cleanup_home(&home);
}

#[tokio::test]
async fn code_review_transcript_matches_tool_result_after_normalization() {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let captured: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    {
        let sink = std::sync::Arc::clone(&captured);
        rt.attach_transcript_appender(std::sync::Arc::new(move |extra| {
            sink.lock().push(extra);
            Ok(())
        }));
    }
    rt.attach_code_reviewer(std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![
        CodeReviewSummary {
            aborted: false,
            verdict: None,
            summary: "review finished without verdict".into(),
            changes_summary: "none".into(),
            applied_changes: false,
            ..Default::default()
        },
    ])));
    rt.attach_verifier(std::sync::Arc::new(MockVerifierDispatcher::new(vec![
        ok_verify_pass(),
    ])));
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(1);

    let out = update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![
                update_plan::UpdateOp::SetStatus {
                    id: "t1".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
                update_plan::UpdateOp::SetStatus {
                    id: "t2".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
            ],
        },
    )
    .await
    .unwrap();

    assert_eq!(out["code_review"]["verdict"], "partial");
    let events = captured.lock();
    let code_review_event = events
        .iter()
        .find(|v| v["event"] == "plan.code_review")
        .expect("缺少 plan.code_review");
    assert_eq!(code_review_event["verdict"], out["code_review"]["verdict"]);
    assert_eq!(code_review_event["summary"], out["code_review"]["summary"]);
    assert_eq!(code_review_event["rounds"], 1);
    cleanup_home(&home);
}

/// D1-d：第 2 轮 review 必须拿到第 1 轮的未清 finding，并且已修项要被核销。
#[tokio::test]
async fn second_review_round_receives_previous_open_findings_and_clears_fixed_ones() {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let round1 = Finding::new("concern".into(), "logic".into(), "missing guard".into());
    let round2 = Finding::new("concern".into(), "tests".into(), "no regression test".into());
    let reviewer = std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![
        CodeReviewSummary {
            aborted: false,
            verdict: Some("fail".into()),
            summary: "round 1".into(),
            findings: vec![round1.clone()],
            ..Default::default()
        },
        CodeReviewSummary {
            aborted: false,
            verdict: Some("fail".into()),
            summary: "round 2".into(),
            findings: vec![round2.clone()],
            ..Default::default()
        },
    ]));
    rt.attach_code_reviewer(reviewer.clone());
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(8);

    let complete_all = |plan_id: String| update_plan::UpdatePlanArgs {
        plan_id: Some(plan_id),
        path: None,
        replace: false,
        ops: vec![
            update_plan::UpdateOp::SetStatus {
                id: "t1".into(),
                content: None,
                status: TodoStatus::Completed,
            },
            update_plan::UpdateOp::SetStatus {
                id: "t2".into(),
                content: None,
                status: TodoStatus::Completed,
            },
        ],
    };

    update_plan::execute(&rt, complete_all(plan_id.clone()))
        .await
        .unwrap();
    assert_eq!(rt.unresolved_finding_ids(&plan_id), vec![round1.id.clone()]);

    // 重开一个 todo 再收口，触发第 2 轮。
    update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![update_plan::UpdateOp::SetStatus {
                id: "t1".into(),
                content: None,
                status: TodoStatus::InProgress,
            }],
        },
    )
    .await
    .unwrap();
    update_plan::execute(&rt, complete_all(plan_id.clone()))
        .await
        .unwrap();

    let seen = reviewer.open_findings_per_round();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].is_empty(), "第 1 轮没有历史 finding");
    assert_eq!(seen[1], vec![round1.clone()], "第 2 轮应收到第 1 轮的未清项");
    // 第 2 轮没再报 round1 → 视为已修，只剩 round2。
    assert_eq!(rt.unresolved_finding_ids(&plan_id), vec![round2.id.clone()]);
    assert_ne!(round1.id, round2.id);
    cleanup_home(&home);
}

/// D1-d：finding id 由内容派生，同一问题跨轮次必须是同一个 id。
#[test]
fn finding_id_is_content_derived_and_severity_independent() {
    let a = Finding::new("concern".into(), "logic".into(), "missing guard".into());
    let b = Finding::new("blocker".into(), "logic".into(), " missing guard ".into());
    let c = Finding::new("concern".into(), "logic".into(), "missing null check".into());
    assert_eq!(a.id, b.id, "severity 与空白不参与派生");
    assert_ne!(a.id, c.id);
    assert!(a.id.starts_with("f-"));
}

/// D1-e：同一进程里二次 build 同一个计划，轮数预算重新发放。
#[tokio::test]
async fn rebuild_resets_code_review_rounds() {
    let _g = home_lock().lock().unwrap();
    let home = setup_isolated_home();
    let rt = PlanRuntime::new("session-a");
    let reviewer = std::sync::Arc::new(MockCodeReviewerDispatcher::new(vec![
        CodeReviewSummary {
            aborted: false,
            verdict: Some("fail".into()),
            summary: "round 1".into(),
            findings: vec![Finding::new(
                "concern".into(),
                "logic".into(),
                "missing guard".into(),
            )],
            ..Default::default()
        },
    ]));
    rt.attach_code_reviewer(reviewer.clone());
    let plan_id = fresh_planning_plan(&rt);
    mark_plan_executing(&rt, &plan_id, "session-a");
    rt.set_max_code_review_rounds(1);

    update_plan::execute(
        &rt,
        update_plan::UpdatePlanArgs {
            plan_id: Some(plan_id.clone()),
            path: None,
            replace: false,
            ops: vec![
                update_plan::UpdateOp::SetStatus {
                    id: "t1".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
                update_plan::UpdateOp::SetStatus {
                    id: "t2".into(),
                    content: None,
                    status: TodoStatus::Completed,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(rt.code_review_rounds(&plan_id), 1);

    // 计划回到 pending 后重新 build —— 预算按次发放，计数与未清项一起清零。
    let path = plan_path_for_id(&plan_id).unwrap();
    let mut plan = read_plan(&path).unwrap();
    plan.frontmatter.state = PlanFileState::Pending;
    write_plan(&path, &plan, 2000).unwrap();
    rt.set_mode_pending_with_path(plan_id.clone(), Some(path.clone()));
    rt.build_plan(&plan_id, Some("sid-session-a".into()))
        .expect("二次 build 失败");

    assert_eq!(rt.code_review_rounds(&plan_id), 0);
    assert!(rt.unresolved_finding_ids(&plan_id).is_empty());
    cleanup_home(&home);
}
