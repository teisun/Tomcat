import type { AttachmentCandidate } from "../../src/shared/attachmentProtocol";

export type WebviewReferenceKind = "selection" | "file";

export type WebviewMessageSegment =
  | {
      text: string;
      type: "text";
    }
  | {
      kind: WebviewReferenceKind;
      label: string;
      lineEnd?: number | null;
      lineStart?: number | null;
      path: string;
      text?: string | null;
      type: "reference";
    };

export type WebviewReference = Extract<
  WebviewMessageSegment,
  { type: "reference" }
>;

export interface ContextSearchMatch {
  description?: string | null;
  reference: WebviewReference;
}

export interface WebviewDomAction {
  kind:
    | "clickTestId"
    | "dragOverTestId"
    | "dragLeaveTestId"
    | "focusTestId"
    | "pasteClipboardFiles"
    | "pressKeyOnTestId"
    | "probeAttachmentFetch"
    | "probeFullResolutionMemory"
    | "probeImagePipeline"
    | "scrollIntoView"
    | "scrollToEdge"
    | "setInputValue"
    | "setRootWidth";
  edge?: "bottom" | "top";
  files?: AttachmentCandidate[];
  index?: number;
  scrollBlock?: "center" | "end" | "nearest" | "start";
  testId?: string;
  value?: string;
  widthPx?: number | null;
}

export interface WebviewMessageBlock {
  assistantMessageId?: string;
  detailText?: string | null;
  deliveryError?: string | null;
  deliveryState?: "failed" | "pending";
  id: string;
  label?: string | null;
  /**
   * Images attached to this message, as references rather than bytes.
   *
   * The bytes stay on disk in the backend and reach Chromium over the webview resource
   * protocol, so scrolling back through a long transcript costs DOM nodes, not
   * megabytes of base64 pinned on the JavaScript heap.
   */
  attachments?: WebviewAttachmentView[];
  kind: "assistant" | "error" | "notice" | "user" | "warn";
  retryable?: boolean;
  segments?: WebviewMessageSegment[];
  submitKind?: "prompt" | "steer";
  text: string;
  type: "message";
}

export interface WebviewThinkingBlock {
  assistantMessageId?: string;
  id: string;
  summaryTitle?: string | null;
  text: string;
  type: "thinking";
}

export interface WebviewBoundaryBlock {
  coveredCount?: number | null;
  id: string;
  summary?: string | null;
  type: "boundary";
}

export interface WebviewCheckpoint {
  changedFiles: string[];
  createdAt: string;
  id: string;
  kind: string;
  label?: string | null;
  messageAnchor?: string | null;
}

export interface WebviewCheckpointMarker {
  changedFiles: string[];
  checkpointId: string;
  createdAt: string;
  id: string;
  kind: string;
  label?: string | null;
  messageAnchor: string;
  type: "checkpoint";
}

export interface WebviewTodo {
  content: string;
  id: string;
  status: "cancelled" | "completed" | "in_progress" | "pending";
}

export type FileDiffTag = "add" | "ctx" | "del";

export interface FileDiffLine {
  newLine?: number | null;
  oldLine?: number | null;
  tag: FileDiffTag;
  text: string;
}

export interface WebviewToolDisplayFile {
  added?: number | null;
  diff?: FileDiffLine[] | null;
  file: string;
  kind: "file";
  removed?: number | null;
}

export interface WebviewToolDisplayPlan {
  kind: "plan";
  plan: string;
}

export interface WebviewToolDisplayText {
  kind: "text";
  text: string;
}

export type WebviewToolDisplay =
  WebviewToolDisplayFile | WebviewToolDisplayPlan | WebviewToolDisplayText;

export type WebviewToolStatus =
  "complete" | "interrupted" | "running" | "streaming";

export interface WebviewToolDiffStat {
  added: number;
  removed: number;
}

export interface WebviewPlanActivity {
  applied?: number;
  checked?: number;
  completed?: number;
  kind: "create" | "update";
  overview?: string | null;
  stateAfter?: WebviewPlanState | null;
  stateBefore?: WebviewPlanState | null;
  title?: string | null;
  total?: number;
}

export interface WebviewToolCard {
  args?: Record<string, unknown>;
  assistantMessageId?: string;
  backgroundExitCode?: number;
  backgroundRunning?: boolean;
  backgroundTaskId?: string;
  liveOutput?: string;
  liveOutputOffset?: number;
  liveOutputSequence?: number;
  liveOutputTruncated?: boolean;
  logPath?: string;
  display?: WebviewToolDisplay;
  diff?: FileDiffLine[];
  diffStat?: WebviewToolDiffStat;
  id: string;
  isError: boolean;
  planActivity?: WebviewPlanActivity;
  planId?: string | null;
  planPath?: string | null;
  startedAt?: number;
  status: WebviewToolStatus;
  summary?: string;
  /** utility-flash 异步生成的命令"目的"短句（bash 卡片标题）；live-only。 */
  summaryTitle?: string | null;
  toolCallId: string;
  toolName: string;
  type: "tool";
}

export type WebviewPlanState =
  "chat" | "planning" | "executing" | "pending" | "completed";

export interface WebviewPlanFileRef {
  path: string;
  planId?: string | null;
  state: WebviewPlanState | null;
}

export interface WebviewPlanFileCard extends WebviewPlanFileRef {
  id: string;
  overview?: string;
  title?: string;
  todos?: WebviewTodo[];
  type: "plan";
}

export type WebviewReviewVerdict = "aborted" | "fail" | "partial" | "pass";

export interface WebviewReviewFinding {
  area: string;
  note: string;
  severity: string;
}

export interface WebviewReviewRow {
  anchorToolCallId?: string | null;
  findings?: WebviewReviewFinding[];
  id: string;
  planId: string;
  reviewAttemptId: string;
  round?: number | null;
  rounds?: number | null;
  status: "done" | "running";
  summary?: string | null;
  type: "review";
  verdict?: WebviewReviewVerdict;
}

export interface WebviewApprovalOption {
  id: string;
  label: string;
  recommended?: boolean;
}

export interface WebviewApprovalQuestion {
  id: string;
  options: WebviewApprovalOption[];
  prompt: string;
}

export const CUSTOM_OPTION_ID = "__custom__";

export interface AskQuestionAnswer {
  customText?: string | null;
  optionIds: string[];
  pickedRecommended: boolean;
  questionId: string;
  skipped?: boolean;
}

export interface AskQuestionResult {
  answers: AskQuestionAnswer[];
  cancelled: boolean;
}

export interface WebviewApprovalCard {
  id: string;
  request: {
    questions: WebviewApprovalQuestion[];
    requestId: string;
    responseEvent: string;
  };
  resolved: boolean;
  sessionId?: string | null;
  type: "approval";
}

/** An image the webview may render, identified by hash instead of carrying bytes. */
export interface WebviewAttachmentView {
  /** sha256 of the original bytes; the backend's name for this attachment. */
  blobSha: string;
  /** Original byte count, for display only. */
  bytes?: number;
  filename: string;
  /** Full-resolution URL. Only the preview panel should load this. */
  fullUri?: string | null;
  /** True once a downsampled version exists in the backend. */
  hasThumb?: boolean;
  id: string;
  kind: "file" | "image";
  mimeType: string;
  /** Downsampled URL, or null until a thumbnail has been generated. */
  thumbUri?: string | null;
  /**
   * The backend no longer has these bytes.
   *
   * Applies to history as much as to a draft: a transcript keeps naming an image after
   * its blob has been garbage-collected, and a message that quietly renders an empty
   * square is worse than one that says the image is gone.
   */
  unavailable?: boolean;
}

export interface WebviewPendingAttachment extends WebviewAttachmentView {
  label: string;
  path?: string | null;
}

export interface WebviewComposerDraft {
  segments: WebviewMessageSegment[];
  text: string;
}

export interface WebviewSessionSnapshot {
  busy: boolean;
  checkpoints?: WebviewCheckpoint[];
  composerDraft?: WebviewComposerDraft;
  contextRatio?: number | null;
  hasMoreHistory?: boolean;
  historyLoading?: boolean;
  model?: string | null;
  planTodos: WebviewTodo[];
  sessionTodos: WebviewTodo[];
  thinkingLevel?: string | null;
  ownedByThisFrontend: boolean;
  pendingAttachments: WebviewPendingAttachment[];
  planFile?: WebviewPlanFileRef | null;
  planId?: string | null;
  planState?: WebviewPlanState | null;
  sessionId: string;
  timeline: WebviewTimelineItem[];
}

export interface WebviewSessionTab {
  busy: boolean;
  isCurrent: boolean;
  ownedByThisFrontend: boolean;
  sessionId: string;
  title: string | null;
  updatedAt: number | null;
}

export interface WebviewMediaRoot {
  fsPath: string;
  webviewBase: string;
}

export interface WebviewStateSnapshot {
  activeSessionId: string | null;
  availableModelCapabilities?: Record<string, string[]>;
  availableModelReasoningLevels?: Record<string, string[]>;
  availableModels: string[];
  buildModel?: string;
  mediaRoots?: WebviewMediaRoot[];
  modelAdminSupported: boolean;
  ready: boolean;
  sessionViews: Record<string, WebviewSessionSnapshot>;
  sessions: WebviewSessionTab[];
}

export type WebviewTimelineItem =
  | WebviewApprovalCard
  | WebviewBoundaryBlock
  | WebviewCheckpointMarker
  | WebviewMessageBlock
  | WebviewPlanFileCard
  | WebviewReviewRow
  | WebviewThinkingBlock
  | WebviewToolCard;

export type WebviewSessionPatchOp =
  | {
      id: string;
      text: string;
      type: "appendText";
    }
  | {
      afterId?: string | null;
      beforeId?: string | null;
      item: WebviewTimelineItem;
      type: "upsert";
    }
  | {
      id: string;
      type: "remove";
    };

export type HostToWebviewFrame =
  | {
      channel: "event";
      content:
        | {
            matches: ContextSearchMatch[];
            query: string;
            requestId: string;
            sessionId?: string | null;
            truncated: boolean;
            type: "contextSearchResult";
            workspaceAvailable?: boolean;
          }
        | {
            reference: WebviewReference;
            sessionId?: string | null;
            type: "insertReference";
          }
        | {
            type: "__test.capture_dom";
          }
        | {
            action: WebviewDomAction;
            type: "__test.dom_action";
          }
        | Record<string, unknown>;
      messageId: string;
    }
  | {
      channel: "sessionPatch";
      content: {
        ops: WebviewSessionPatchOp[];
        seq: number;
        sessionId: string;
      };
      messageId: string;
    }
  | {
      channel: "sessionView";
      content: {
        sessionId: string;
        tab?: WebviewSessionTab | null;
        view: WebviewSessionSnapshot;
      };
      messageId: string;
    }
  | {
      channel: "state";
      content: WebviewStateSnapshot;
      messageId: string;
    };

export type WebviewIntent =
  | {
      messageId: string;
      type: "answerQuestion";
      data: {
        requestId: string;
        result: AskQuestionResult;
      };
    }
  | {
      messageId: string;
      type: "closeSession" | "switchSession";
      data: {
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "interrupt";
      data?: {
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "listSessions" | "ready";
    }
  | {
      messageId: string;
      type: "loadOlderHistory";
      data: {
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "newSession";
      data?: {
        cwd?: string | null;
      };
    }
  | {
      messageId: string;
      type: "prompt" | "steer";
      data: {
        segments?: WebviewMessageSegment[];
        sessionId?: string | null;
        text: string;
        userMessageId?: string;
      };
    }
  | {
      messageId: string;
      type: "pickContext";
      data?: {
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "retryUserMessage";
      data: {
        messageId: string;
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "resyncSessionView";
      data: {
        sessionId: string;
      };
    }
  /**
   * A thumbnail the webview generated for an attachment the backend had none for.
   *
   * The only message that carries image bytes besides the paste path, and it carries the
   * small end: a 192px PNG, never the original.
   */
  | {
      messageId: string;
      type: "cacheAttachmentThumbnail";
      data: {
        blobSha: string;
        sessionId: string;
        thumbBase64: string;
      };
    }
  | {
      messageId: string;
      type: "syncComposerDraft";
      data: {
        segments: WebviewMessageSegment[];
        sessionId: string;
        text: string;
      };
    }
  | {
      messageId: string;
      /**
       * The one message in the whole system that carries image bytes.
       *
       * Everything downstream of ingest speaks in hashes. Downsampling and SVG
       * rasterisation happen here, before the send, because a webview is the only place
       * in this system with a decoder that can resize *during* decode rather than
       * after — which is the difference between a 4000x3000 paste costing 48 KB and
       * costing 48 MB.
       */
      type: "attachFiles";
      data: {
        files: AttachmentCandidate[];
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "openImagePreview" | "removeDraftAttachment";
      data: {
        attachmentId: string;
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "removeAttachment";
      data: {
        attachmentId: string;
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "setModel";
      data: {
        modelId: string;
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "setBuildModel";
      data: {
        modelId: string;
      };
    }
  | {
      messageId: string;
      type: "setThinkingLevel";
      data: {
        level: string;
        modelId: string;
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "openModelSettings";
      data?: {
        route?: "models" | null;
      };
    }
  | {
      messageId: string;
      type: "setPlanMode";
      data: {
        action: "build" | "enter" | "exit";
        planId?: string | null;
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "openFile" | "openPlanFile";
      data: {
        line?: number;
        path: string;
      };
    }
  | {
      messageId: string;
      type: "openDiff";
      data: {
        toolCallId: string;
      };
    }
  | {
      messageId: string;
      type: "resolveDrop";
      data: {
        sessionId?: string | null;
        uris: string[];
      };
    }
  | {
      messageId: string;
      type: "searchContext";
      data: {
        kind?: "file";
        query: string;
        requestId: string;
        sessionId?: string | null;
      };
    }
  | {
      messageId: string;
      type: "showWarningMessage";
      data: {
        message: string;
      };
    }
  | {
      messageId: string;
      type: "listCheckpoints";
      data: {
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "restoreCheckpoint";
      data: {
        checkpointId: string;
        revertFiles: boolean;
        sessionId: string;
      };
    }
  | {
      messageId: string;
      type: "__test.dom_snapshot";
      data: {
        activeSessionId: string | null;
        approvalCount: number;
        approvalInputTestIds: string[];
        approvalOptionStates: Array<{
          selected: boolean;
          testId: string;
        }>;
        composerControlMetrics: Record<
          string,
          {
            top: number;
            width: number;
          }
        >;
        composerFooterPlanStatus: string | null;
        composerPlanStatusInBarCount: number;
        composerRowCount: number;
        ctxLabel: string | null;
        disabledTestIds: string[];
        expandedThinkingCount: number;
        expandedToolTitles: string[];
        fileChipTopWithinStream: number | null;
        fileChipVisible: boolean;
        historyLoaderVisible: boolean;
        historyAttachmentThumbCount: number;
        html: string;
        jumpToLatestVisible: boolean;
        planCardTopWithinStream: number | null;
        latestUserTopWithinStream: number | null;
        messageTexts: string[];
        modelDropdownBottom: number | null;
        modelDropdownFullyVisible: boolean;
        modelDropdownHeight: number;
        modelDropdownLeft: number | null;
        modelDropdownRight: number | null;
        modelDropdownTop: number | null;
        overflowAnchor: string | null;
        pendingAttachmentStripClientWidth: number;
        pendingAttachmentStripOverflowing: boolean;
        pendingAttachmentStripScrollWidth: number;
        pendingAttachmentThumbCount: number;
        sessionTabs: string[];
        sessionGroupHeaders: string[];
        sessionMoreButtons: string[];
        stickyPromptText: string | null;
        streamMetrics: {
          clientHeight: number;
          distanceFromBottom: number;
          scrollHeight: number;
          scrollTop: number;
        };
        timelineKinds: string[];
        toolBodyMetrics: Array<{
          clientHeight: number;
          expanded: boolean;
          scrollHeight: number;
          title: string;
        }>;
        toolTitles: string[];
        assistantResponseGroups: number;
        assistantClickablePathCount: number;
        assistantCodeCardCount: number;
        groupFoldTitles: string[];
        userPromptPill: boolean;
        assistantNoCard: boolean;
        planCardCount: number;
        planFooterSameRow: boolean;
        planCardTodoCountText: string | null;
        planCardTitleText: string | null;
        planNoticeReplayed: boolean;
        planStateText: string | null;
        progressRow: boolean;
        loadingShimmerCount: number;
        planTodos: number;
        standaloneThinkingTitles: string[];
        todoWidgetExpanded: boolean;
        todoWidgetItemCount: number;
        todoWidgetTitle: string | null;
        todoWidgetVisible: boolean;
        toolRowFlat: boolean;
        toolRowExpandable: boolean;
        ellipsisAboveGroupHeader: boolean;
        leftGuideLine: boolean;
        toolRowCount: number;
        toolCardCount: number;
        actionToolRowCount: number;
        editDiffBadgeCount: number;
        commandBlockCount: number;
      };
    };

export interface VsCodeApiLike {
  postMessage(message: WebviewIntent): void;
  setState?(state: unknown): void;
}
