import {
  assertPlanPreviewCustomEditorFlow,
  assertWebviewCompletedPlanStaysInChat,
  assertWebviewPlanModeSwitchFlow,
  assertWebviewCompactControlFlow,
  assertWebviewPersistedMessageKindFlow,
  assertWebviewAnswerCardFlow,
  assertWebviewAddModelsFlow,
  assertWebviewDiffFlow,
  assertWebviewEditDisplayReplayFlow,
  assertWebviewReviewProgressFlow,
  assertWebviewAtMentionDirectoryAndWarningFlow,
  assertWebviewAtMentionReferenceFlow,
  assertWebviewDraftForkPreservesReferenceFlow,
  assertWebviewFileDropReferenceFlow,
  assertWebviewPickContextFlow,
  assertWebviewRecoveryRejectionFlow,
  assertWebviewResumeCardFlow,
  assertWebviewRetryRecoveryFlow,
  assertWebviewGiantGroupLazyLoadFlow,
  assertWebviewInterruptFlow,
  assertWebviewMultiSessionFlow,
  assertWebviewPlanToolUxFlow,
  assertWebviewReloadReplayFlow,
  assertWebviewSelectionReferenceFlow,
  assertWebviewSessionTitleFlow,
  assertWebviewSessionSwitchRestoreFlow,
  assertWebviewStickyHistoryFlow,
  assertWebviewStreamingFlow,
  assertTranscriptUiFlow,
  assertTranscriptRichRenderingFlow,
  assertTranscriptSwitchBackOrder,
  assertWebviewMaxReasoningAndLoadingGapFlow,
  getTomcatExtensionApi,
} from "./support/hostE2eScenario";

suite("Tomcat host E2E", () => {
  test("interrupts an executing plan and resumes it in Chat", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewPlanModeSwitchFlow(api);
  });

  test("keeps the Composer in Chat after plan completion", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewCompletedPlanStaysInChat(api);
  });

  test("renders the .plan.md custom editor (hybrid default), mode switch, hot reload, and selection-to-chat", async function () {
    // Visual acceptance deliberately records four real window frames in this
    // already broad flow. Keep the regular timeout unchanged, but give that
    // explicitly requested I/O work enough room when screenshot mode is on.
    if (process.env.TOMCAT_E2E_SCREENSHOT === "1") {
      this.timeout(90_000);
    }
    const api = await getTomcatExtensionApi();
    await assertPlanPreviewCustomEditorFlow(api);
  });

  test("adds a model through settings and uses it in the webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewAddModelsFlow(api);
  });

  test("keeps max reasoning and pre-stream loading states working in the webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewMaxReasoningAndLoadingGapFlow(api);
  });

  test("streams in the Tomcat webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewStreamingFlow(api);
  });

  test("applies edits from the Tomcat webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewDiffFlow(api);
  });

  test("replays single-file and multi-file edit cards after a webview reload", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewEditDisplayReplayFlow(api);
  });

  test("shows running review progress and settles into pass/fail rows", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewReviewProgressFlow(api);
  });

  test("copy-forwards a failed same-session retry while preserving the failed chapter", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewRetryRecoveryFlow(api);
  });

  test("shows a rejected recovery reason inline on its restored error card", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewRecoveryRejectionFlow(api);
  });

  test("resumes a failed tool turn and preserves its completed error card", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewResumeCardFlow(api);
  });

  test("places the compact control beside new session with a visible layers icon", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewCompactControlFlow(api);
  });

  test("rehydrates steering, guard, and background message kinds with their distinct UI", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewPersistedMessageKindFlow(api);
  });

  test("renders ask_question answers in the Tomcat webview transcript", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewAnswerCardFlow(api);
  });

  test("renders transcript action rows and context groups in the Tomcat webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertTranscriptUiFlow(api);
  });

  test("renders transcript markdown as rich code cards and clickable file paths", async () => {
    const api = await getTomcatExtensionApi();
    await assertTranscriptRichRenderingFlow(api);
  });

  test("keeps plan tool UX clean in the Tomcat webview transcript", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewPlanToolUxFlow(api);
  });

  test("keeps sticky user prompts aligned with historical turns in the Tomcat webview", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewStickyHistoryFlow(api);
  });

  test("resets interrupted Tomcat webview sessions back to send mode", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewInterruptFlow(api);
  });

  test("keeps multiple Tomcat webview sessions isolated", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewMultiSessionFlow(api);
  });

  test("restores plan cards and Ctx after switching sessions", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewSessionSwitchRestoreFlow(api);
  });

  test("replays plan history after a webview reload", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewReloadReplayFlow(api);
  });

  test("keeps transcript thinking and tool order stable after switching away and back", async () => {
    const api = await getTomcatExtensionApi();
    await assertTranscriptSwitchBackOrder(api);
  });

  test("lazy loads a giant historical tool group without rendering half a group", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewGiantGroupLazyLoadFlow(api);
  });

  test("adds editor selections to the webview composer and rehydrates them from history", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewSelectionReferenceFlow(api);
  });

  test("deduplicates dropped file references in the webview composer", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewFileDropReferenceFlow(api);
  });

  test("keeps source draft references when creating a session", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewDraftForkPreservesReferenceFlow(api);
  });

  test("routes smart picker selections into attachments and context chips", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewPickContextFlow(api);
  });

  test("supports @ file search, inline chips, and replay after reload", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewAtMentionReferenceFlow(api);
  });

  test("supports @ directory search and no-workspace warning fallback", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewAtMentionDirectoryAndWarningFlow(api);
  });

  test("derives non-placeholder session titles from first webview prompt segments", async () => {
    const api = await getTomcatExtensionApi();
    await assertWebviewSessionTitleFlow(api);
  });
});
