import { randomUUID } from "node:crypto";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import * as vscode from "vscode";

import { TOMCAT_CONFIG_SECTION } from "../../constants";
import type { VsCodeIde } from "../../ide/VsCodeIde";
import {
  hasAnyModelAdminCapability,
  hasServeCapability,
  SERVE_CAPABILITY_LIST_MODELS,
  type InitializeResult,
} from "../../serveClient/initialize";
import {
  normalizeAskQuestionResponse,
  type AskQuestionWireRequest,
  type AskQuestionWireResponse,
} from "../../serveClient/protocol";
import type { SessionRouter } from "../../serveClient/sessionRouter";
import type { TomcatMessenger } from "../../serveClient/TomcatMessenger";
import type { ServeContentSegment, ServeEvent } from "../../serveClient/wire";
import {
  cacheAttachmentThumbnail,
  ingestAttachment,
  type AttachmentUpload,
} from "../../shared/attachmentIngest";
import {
  attachmentResourceRoots,
  resolveAttachmentUris,
} from "../../shared/attachmentUris";
import {
  ComposerDraftStore,
  isDraftEmpty,
  type ComposerDraft,
  type DraftAttachmentRef,
} from "../../shared/composerDraft";
import {
  validateAttachmentCandidate,
} from "../../shared/attachmentProtocol";
import type { PreviewSection } from "../../shared/imagePreviewProtocol";
import {
  createHostFrameMessageId,
  isWebviewIntent,
  PendingMessageTracker,
  type FileDiffLine,
  type HostEventFrameContent,
  type HostToWebviewFrame,
  type WebviewApprovalCard,
  type WebviewDomAction,
  type WebviewMessageBlock,
  type WebviewAttachmentView,
  type WebviewMessageSegment,
  type WebviewMediaRoot,
  type WebviewPendingAttachment,
  type WebviewIntent,
  type WebviewPlanFileCard,
  type WebviewReference,
  type WebviewStateSnapshot,
  type WebviewToolCard,
} from "./protocol";
import { resolveWebviewEntryAssets } from "../guiAssets";
import { parsePlanDocument } from "../planPreview/planDocument";
import { ContextSearchService } from "./contextSearch";
import { buildFileReference } from "./contextReferences";
import { TomcatSessionPool } from "./sessionPool";
import {
  type StateBroadcasterFlushPlan,
  StateBroadcaster,
} from "./stateBroadcaster";
import {
  type AttachmentUriResolver,
  type SessionRenderMutation,
  WebviewStateStore,
} from "./state";

const HISTORY_PAGE_ENTRIES = 80;

/**
 * How history pages ask for image attachments.
 *
 * `reference` gets hashes instead of inline base64. The default is `inline` to keep the
 * CLI working unchanged, but for a UI it is the wrong answer by two orders of magnitude:
 * a page of 80 entries containing eleven photos is ~66MB of base64 that would land on
 * the extension host's heap, be forwarded into a state snapshot, and be pinned there for
 * as long as the user stays scrolled back.
 */
const HISTORY_ATTACHMENT_MODE = "reference" as const;

/** Where the last known attachment directory is remembered between launches. */
const ATTACHMENT_ROOT_MEMENTO_KEY = "tomcat.attachmentRoot";

type PendingQuestion = {
  request: AskQuestionWireRequest;
  resolve(response: AskQuestionWireResponse): void;
  sessionId?: string | null;
};

type DomSnapshot = Extract<
  WebviewIntent,
  { type: "__test.dom_snapshot" }
>["data"];

type UserSubmitKind = "prompt" | "steer";

function isRecordValue(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isMutationTool(toolName: string): boolean {
  return toolName === "write" || toolName === "edit" || toolName === "hashline_edit";
}

function reconstructDiffPair(diff: FileDiffLine[]): { after: string; before: string } {
  const before: string[] = [];
  const after: string[] = [];
  for (const line of diff) {
    if (line.tag !== "add") {
      before.push(line.text);
    }
    if (line.tag !== "del") {
      after.push(line.text);
    }
  }
  return {
    after: after.join("\n"),
    before: before.join("\n"),
  };
}

export interface TomcatWebviewProviderDeps {
  /**
   * Where to keep composer drafts.
   *
   * Only the two storage URIs are needed, not the whole extension context, so tests can
   * hand over a temp directory without faking half of VS Code.
   */
  draftStorage?: Pick<vscode.ExtensionContext, "globalStorageUri" | "storageUri">;
  extensionUri: vscode.Uri;
  /**
   * Remembers the backend's attachment directory between launches.
   *
   * Resource roots can only be granted by reassigning `webview.options`, and VS Code
   * reloads the webview when they change. Only `get_state` knows the path, which is not
   * available until after the webview has loaded — so without a remembered value every
   * single launch would load the panel twice. The path is stable for a given tomcat data
   * directory, and a stale value costs nothing: it is re-granted from `get_state` anyway.
   */
  attachmentRootMemento?: Pick<vscode.Memento, "get" | "update">;
  getDefaultCwd(): string | undefined;
  ide: VsCodeIde;
  initialize(): Promise<InitializeResult>;
  messenger: TomcatMessenger;
  openModelSettings?(route?: "models"): void;
  refreshPlanPreview?(planId: string | null, path: string | null): Promise<void> | void;
  sessionRouter: SessionRouter;
  showOpenDialog?(
    options: vscode.OpenDialogOptions,
  ): Thenable<readonly vscode.Uri[] | undefined> | readonly vscode.Uri[] | undefined;
}

function getNonce(): string {
  return Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
}

function parseCapabilityNames(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string");
  }
  if (typeof value !== "object" || value === null) {
    return [];
  }
  return Object.entries(value)
    .filter(([, enabled]) => enabled === true)
    .map(([name]) => name);
}

function extractPlanPreviewRefreshArgs(
  event: ServeEvent,
): { path: string | null; planId: string | null } | null {
  switch (event.type) {
    case "plan.create":
    case "plan.update":
      return {
        path: typeof event.path === "string" ? event.path : null,
        planId: typeof event.planId === "string" ? event.planId : null,
      };
    case "plan.todos":
      return {
        path: null,
        planId: typeof event.planId === "string" ? event.planId : null,
      };
    default:
      return null;
  }
}

export function parseModelCatalog(payload: unknown): {
  capabilities: Record<string, string[]>;
  ids: string[];
  reasoningLevels: Record<string, string[]>;
} {
  if (typeof payload !== "object" || payload === null) {
    return { capabilities: {}, ids: [], reasoningLevels: {} };
  }
  const models = (payload as { models?: unknown }).models;
  if (!Array.isArray(models)) {
    return { capabilities: {}, ids: [], reasoningLevels: {} };
  }
  const ids: string[] = [];
  const capabilities: Record<string, string[]> = {};
  const reasoningLevels: Record<string, string[]> = {};
  for (const entry of models) {
    if (typeof entry !== "object" || entry === null || typeof (entry as { id?: unknown }).id !== "string") {
      continue;
    }
    if ((entry as { keyPresent?: unknown }).keyPresent === false) {
      continue;
    }
    const id = (entry as { id: string }).id;
    ids.push(id);
    capabilities[id] = parseCapabilityNames((entry as { capabilities?: unknown }).capabilities);
    reasoningLevels[id] = Array.isArray((entry as { supportedReasoningLevels?: unknown }).supportedReasoningLevels)
      ? ((entry as { supportedReasoningLevels?: unknown }).supportedReasoningLevels as unknown[]).filter(
          (level): level is string => typeof level === "string",
        )
      : [];
  }
  return { capabilities, ids, reasoningLevels };
}

function guessMimeType(filePath: string): string {
  switch (path.extname(filePath).toLowerCase()) {
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    case ".gif":
      return "image/gif";
    case ".webp":
      return "image/webp";
    case ".svg":
      return "image/svg+xml";
    case ".md":
      return "text/markdown";
    case ".txt":
      return "text/plain";
    case ".json":
      return "application/json";
    case ".pdf":
      return "application/pdf";
    default:
      return "application/octet-stream";
  }
}

type PickedUriKind = "attachment" | "reference";

type PickedUriMetadata = {
  isDirectory: boolean;
  mimeType: string;
};

type ResolvedPickedUri =
  | {
      kind: "attachment";
      /** Bytes read off disk, on their way to `ingest_attachment`. */
      upload: AttachmentUpload;
    }
  | {
      kind: "reference";
      reference: WebviewReference;
    };

function isAttachmentMimeType(mimeType: string): boolean {
  return mimeType === "application/pdf" || mimeType.startsWith("image/");
}

function classifyPickedUriMetadata(metadata: PickedUriMetadata): PickedUriKind {
  if (metadata.isDirectory) {
    return "reference";
  }
  return isAttachmentMimeType(metadata.mimeType) ? "attachment" : "reference";
}

async function readPickedUriMetadata(uri: vscode.Uri): Promise<PickedUriMetadata> {
  const stat = await vscode.workspace.fs.stat(uri).then(
    (value) => value,
    () => null,
  );
  return {
    isDirectory: stat?.type === vscode.FileType.Directory,
    mimeType: guessMimeType(uri.fsPath || uri.path),
  };
}

export async function classifyPickedUri(uri: vscode.Uri): Promise<PickedUriKind> {
  return classifyPickedUriMetadata(await readPickedUriMetadata(uri));
}

export function buildAttachmentOpenDialogOptions(): vscode.OpenDialogOptions {
  return {
    canSelectFiles: true,
    canSelectFolders: true,
    canSelectMany: true,
    openLabel: "Add to Tomcat",
  };
}

function shouldReconcileSessionState(event: ServeEvent): boolean {
  return (
    event.type === "agent_end" ||
    event.type === "agent_interrupted" ||
    event.type === "turn_end" ||
    event.type === "plan.complete" ||
    event.type === "plan.exit" ||
    event.type === "plan.pending"
  );
}

function shouldForceServeEventFlush(event: ServeEvent): boolean {
  return (
    event.type === "message_end" ||
    event.type === "tool_execution_end" ||
    event.type === "turn_end" ||
    event.type === "agent_end" ||
    event.type === "agent_idle" ||
    event.type === "agent_interrupted" ||
    event.type === "plan.complete" ||
    event.type === "plan.exit" ||
    event.type === "plan.pending"
  );
}

type PlanMetadataCacheEntry = {
  mtimeMs: number;
  overview?: string;
  title?: string;
};

function expandHomePath(filePath: string): string {
  if (filePath === "~") {
    return os.homedir();
  }
  if (filePath.startsWith("~/")) {
    return path.join(os.homedir(), filePath.slice(2));
  }
  if (filePath.startsWith("$HOME/")) {
    return path.join(os.homedir(), filePath.slice("$HOME/".length));
  }
  return filePath;
}

export function parsePlanFrontmatter(
  text: string,
): Pick<WebviewPlanFileCard, "overview" | "title"> {
  const parsed = parsePlanDocument(text);
  const metadata: Pick<WebviewPlanFileCard, "overview" | "title"> = {};
  if (parsed.title) {
    metadata.title = parsed.title;
  }
  if (parsed.overview) {
    metadata.overview = parsed.overview;
  }
  return metadata;
}

export async function readPlanMetadata(
  filePath: string,
  cache: Map<string, PlanMetadataCacheEntry>,
): Promise<Pick<WebviewPlanFileCard, "overview" | "title">> {
  const resolvedPath = expandHomePath(filePath);
  try {
    const stat = await fs.promises.stat(resolvedPath);
    const cached = cache.get(filePath);
    if (cached && cached.mtimeMs === stat.mtimeMs) {
      return cached;
    }

    const text = await fs.promises.readFile(resolvedPath, "utf8");
    const metadata = parsePlanFrontmatter(text);
    cache.set(filePath, {
      ...metadata,
      mtimeMs: stat.mtimeMs,
    });
    return metadata;
  } catch {
    cache.delete(filePath);
    return {};
  }
}

function formatBridgeError(action: string, error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("Timed out waiting for response")) {
    return `Unable to ${action}: Tomcat bridge is not responding. Restart Tomcat and try again.`;
  }
  if (message.includes("tomcat serve exited")) {
    return `Unable to ${action}: Tomcat serve exited. Restart Tomcat and try again.`;
  }
  if (message.includes("TomcatMessenger has been disposed")) {
    return `Unable to ${action}: Tomcat bridge is unavailable. Restart Tomcat and try again.`;
  }
  return `Unable to ${action}: ${message}`;
}

export class TomcatWebviewViewProvider implements vscode.WebviewViewProvider, vscode.Disposable {
  private readonly contextSearch = new ContextSearchService();
  private readonly domSnapshots = new PendingMessageTracker<DomSnapshot>();
  private readonly historyFetchGen = new Map<string, number>();
  /**
   * Unsent composer state, owned by this extension.
   *
   * The backend is never told about a keystroke. It only ever hears about attachment
   * bytes, once, at paste time. See `shared/composerDraft.ts` for why.
   */
  private readonly draftStore: ComposerDraftStore;
  /**
   * Filesystem root of the backend's attachment store, reported by the handshake.
   *
   * Needed to grant the webview read access and to turn hashes into URLs. It is derived
   * from agent config on the backend side, so the extension cannot compute it.
   */
  private attachmentRoot: string | null = null;
  /** Whether the backend has confirmed the path, as opposed to it being a remembered guess. */
  private attachmentRootResolved = false;
  /** Resolves once the handshake has settled the root, so no further reload is pending. */
  private attachmentRootPrimed?: Promise<void>;
  private imagePreviewSessionId: string | null = null;
  private readonly pendingQuestions = new Map<string, PendingQuestion>();
  private readonly planMetadataCache = new Map<string, PlanMetadataCacheEntry>();
  private readonly sessionsAwaitingErrorHistoryRefresh = new Set<string>();
  private readonly readyWaiters = new Set<{
    reject(error: Error): void;
    resolve(): void;
    timeout: NodeJS.Timeout;
  }>();
  private serveEventQueue: Promise<void> = Promise.resolve();
  private readonly sessionPool: TomcatSessionPool;
  private readonly stateStore: WebviewStateStore;
  private readonly stateBroadcaster: StateBroadcaster;
  private readonly eventSubscription: { dispose(): void };
  private readonly workspaceFolderSubscription: vscode.Disposable;
  private readonly sessionPatchFramesEnabled =
    process.env.TOMCAT_DISABLE_SESSION_PATCHES !== "1";
  /** Plan paths already auto-opened after review, so repeats don't reopen. */
  private readonly autoOpenedPlanPaths = new Set<string>();
  /** `plan.create` gives us the path; `plan.review` is the actual open trigger. */
  private readonly pendingPlanOpenByPlanId = new Map<string, string>();
  private initialized?: InitializeResult;
  private isReady = false;
  private lastContextSearchIntent: Extract<WebviewIntent, { type: "searchContext" }> | null = null;
  private openFileObserved = false;
  private contextSearchTokenSource?: vscode.CancellationTokenSource;
  private messageSubscription?: vscode.Disposable;
  private visibilitySubscription?: vscode.Disposable;
  private view?: vscode.WebviewView;

  constructor(private readonly deps: TomcatWebviewProviderDeps) {
    // Restored before the view is ever resolved, so the first `webview.options` already
    // grants the attachment directory and nothing has to change later.
    const remembered = deps.attachmentRootMemento?.get<string>(ATTACHMENT_ROOT_MEMENTO_KEY);
    this.attachmentRoot = typeof remembered === "string" && remembered ? remembered : null;
    this.draftStore = deps.draftStorage
      ? ComposerDraftStore.forContext(deps.draftStorage)
      : // No storage granted (unit tests, and the settings-only host) means drafts live
        // for the lifetime of the window and no further. Better than refusing to run.
        new ComposerDraftStore(vscode.Uri.joinPath(deps.extensionUri, ".drafts"));
    this.sessionPool = new TomcatSessionPool(deps.sessionRouter);
    this.stateStore = new WebviewStateStore();
    this.stateBroadcaster = new StateBroadcaster({
      delayMs: 16,
      flush: (plan) => this.flushStateBroadcastPlan(plan),
    });
    this.eventSubscription = deps.messenger.onEvent((event) => {
      this.serveEventQueue = this.serveEventQueue
        .then(() => this.handleServeEvent(event))
        .catch((error) => {
          console.error("Tomcat webview failed to process serve event", error);
        });
    });
    const onDidChangeWorkspaceFolders =
      (vscode.workspace as typeof vscode.workspace & {
        onDidChangeWorkspaceFolders?: (
          listener: (event: vscode.WorkspaceFoldersChangeEvent) => void,
        ) => vscode.Disposable;
      }).onDidChangeWorkspaceFolders;
    this.workspaceFolderSubscription =
      typeof onDidChangeWorkspaceFolders === "function"
        ? onDidChangeWorkspaceFolders(() => {
            this.handleWorkspaceFolderChange();
          })
        : new vscode.Disposable(() => undefined);
  }

  dispose(): void {
    this.contextSearchTokenSource?.cancel();
    this.contextSearchTokenSource?.dispose();
    this.contextSearch.dispose();
    this.messageSubscription?.dispose();
    this.visibilitySubscription?.dispose();
    this.eventSubscription.dispose();
    this.workspaceFolderSubscription.dispose();
    this.stateBroadcaster.dispose();
    this.domSnapshots.rejectAll(new Error("Tomcat webview disposed"));
    for (const waiter of [...this.readyWaiters]) {
      clearTimeout(waiter.timeout);
      waiter.reject(new Error("Tomcat webview disposed"));
      this.readyWaiters.delete(waiter);
    }
  }

  resolveWebviewView(view: vscode.WebviewView): void | Thenable<void> {
    this.view = view;
    this.isReady = false;
    this.stateStore.setReady(false);
    // Attachment URLs are webview-scoped, so the mapping is only valid once there is a
    // webview to scope them to, and has to be replaced whenever this one is.
    this.stateStore.setAttachmentUriResolver(this.historyAttachmentResolver());
    view.webview.options = this.webviewOptions();
    view.webview.html = this.renderHtml(view.webview);
    // Deliberately not awaited: the first paint should not wait on the backend. But it is
    // started here, as early as possible, because settling the attachment root can reload
    // the document (see `adoptAttachmentRoot`) and that must not land mid-interaction.
    this.attachmentRootPrimed = this.ensureInitialized().then(
      () => undefined,
      (error: unknown) => {
        // Initialization failures surface through the normal bootstrap path; here they
        // only mean "no attachment root yet", which degrades to images not rendering.
        console.warn("Tomcat could not resolve the attachment root", error);
      },
    );
    this.messageSubscription?.dispose();
    this.visibilitySubscription?.dispose();
    this.messageSubscription = view.webview.onDidReceiveMessage((message: unknown) => {
      void this.handleWebviewMessage(message);
    });
    this.visibilitySubscription = view.onDidChangeVisibility(() => {
      if (view.visible) {
        void this.broadcastFullState({ force: true });
      }
    });
  }

  async waitUntilReady(timeoutMs = 15_000): Promise<void> {
    // Granting the attachment root reloads the document and invalidates an earlier
    // "ready", so a ready flag observed before that point cannot be trusted.
    await this.attachmentRootPrimed;
    if (this.isReady) {
      return;
    }
    return new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.readyWaiters.delete(waiter);
        reject(new Error("Timed out waiting for the Tomcat webview to become ready"));
      }, timeoutMs).unref();
      const waiter = { reject, resolve, timeout };
      this.readyWaiters.add(waiter);
    });
  }

  async captureDomSnapshot(): Promise<DomSnapshot> {
    await this.waitUntilReady();
    const messageId = createHostFrameMessageId("webview-dom");
    const pending = this.domSnapshots.create(messageId, 20_000);
    await this.postMessage({
      channel: "event",
      content: { type: "__test.capture_dom" },
      messageId,
    });
    return pending;
  }

  getOpenFileObserved(): boolean {
    return this.openFileObserved;
  }

  resetOpenFileObserved(): void {
    this.openFileObserved = false;
  }

  getLastContextSearchIntent(): Extract<WebviewIntent, { type: "searchContext" }> | null {
    return this.lastContextSearchIntent;
  }

  resetForTestReload(): void {
    this.isReady = false;
    this.lastContextSearchIntent = null;
    this.openFileObserved = false;
    this.planMetadataCache.clear();
    this.stateStore.resetForReload();
  }

  async dispatchTestDomAction(action: WebviewDomAction): Promise<void> {
    await this.waitUntilReady();
    await this.postMessage({
      channel: "event",
      content: { action, type: "__test.dom_action" },
      messageId: createHostFrameMessageId("webview-dom-action"),
    });
  }

  async dispatchTestHostEvent(content: HostEventFrameContent): Promise<void> {
    await this.waitUntilReady();
    await this.postEvent(content);
  }

  reveal(preserveFocus = false): void {
    this.view?.show(preserveFocus);
  }

  async dispatchTestIntent(intent: Exclude<WebviewIntent, { type: "__test.dom_snapshot" }>): Promise<void> {
    await this.handleIntent(intent);
  }

  async askUser(
    request: AskQuestionWireRequest,
    sessionId?: string | null,
  ): Promise<AskQuestionWireResponse> {
    const responsePromise = new Promise<AskQuestionWireResponse>((resolve) => {
      this.pendingQuestions.set(request.requestId, { request, resolve, sessionId });
    }).finally(() => {
      this.pendingQuestions.delete(request.requestId);
      this.stateStore.resolveApproval(request.requestId);
      void this.postState();
    });

    this.stateStore.applyEvent({
      payload: request,
      requestId: request.requestId,
      sessionId,
      subtype: "ask_question",
      type: "control_request",
    });
    await this.postEvent({
      payload: request,
      requestId: request.requestId,
      sessionId,
      subtype: "ask_question",
      type: "control_request",
    });
    await this.postState();
    return responsePromise;
  }

  currentState() {
    return this.decorateStateSnapshot(this.stateStore.snapshot());
  }

  private peekState(): Readonly<WebviewStateSnapshot> {
    return this.stateStore.view();
  }

  private decorateStateSnapshot(snapshot: WebviewStateSnapshot): WebviewStateSnapshot {
    return {
      ...snapshot,
      mediaRoots: this.view ? this.mediaRootsForWebview(this.view.webview) : [],
    };
  }

  private findToolCard(
    toolCallId: string,
  ): { sessionId: string; tool: WebviewToolCard } | undefined {
    for (const [sessionId, session] of Object.entries(this.peekState().sessionViews)) {
      const tool = session.timeline.find(
        (item): item is WebviewToolCard => item.type === "tool" && item.toolCallId === toolCallId,
      );
      if (tool) {
        return { sessionId, tool };
      }
    }
    return undefined;
  }

  async refreshModelCatalog(): Promise<void> {
    await this.refreshModels();
    if (this.isReady) {
      await this.postState();
    }
  }

  private pendingAttachmentsForSession(sessionId: string): WebviewPendingAttachment[] {
    return this.peekState().sessionViews[sessionId]?.pendingAttachments ?? [];
  }

  private async showOpenDialog(
    options: vscode.OpenDialogOptions,
  ): Promise<readonly vscode.Uri[] | undefined> {
    return this.deps.showOpenDialog?.(options) ?? vscode.window.showOpenDialog(options);
  }

  private async resolvePickedUri(uri: vscode.Uri): Promise<ResolvedPickedUri> {
    const metadata = await readPickedUriMetadata(uri);
    if (classifyPickedUriMetadata(metadata) === "attachment") {
      return {
        kind: "attachment",
        upload: await this.readAttachmentUpload(uri, metadata.mimeType),
      };
    }
    return {
      kind: "reference",
      reference: buildFileReference(uri, {
        isDirectory: metadata.isDirectory,
      }),
    };
  }

  private async ingestPickedUris(
    sessionId: string,
    uris: readonly vscode.Uri[],
  ): Promise<void> {
    const resolved: ResolvedPickedUri[] = [];
    const errors: string[] = [];
    for (const uri of uris) {
      try {
        resolved.push(await this.resolvePickedUri(uri));
      } catch (error) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    }

    const uploads: AttachmentUpload[] = [];
    for (const entry of resolved) {
      if (entry.kind === "reference") {
        await this.postInsertReference(sessionId, entry.reference);
      } else {
        uploads.push(entry.upload);
      }
    }

    const accepted = await this.ingestUploads(sessionId, uploads, errors);
    await this.reportAttachmentOutcome(accepted, errors);
  }

  /**
   * Hand a batch of uploads to the backend and add whatever it accepts to the draft.
   *
   * Per-item outcomes, not all-or-nothing: pasting eleven images where one is oversized
   * should attach ten and explain the one.
   */
  private async ingestUploads(
    sessionId: string,
    uploads: AttachmentUpload[],
    errors: string[],
  ): Promise<DraftAttachmentRef[]> {
    if (uploads.length === 0) return [];
    const references: DraftAttachmentRef[] = [];
    for (const upload of uploads) {
      const outcome = await ingestAttachment(
        this.deps.messenger,
        sessionId,
        randomUUID(),
        upload,
      );
      if (outcome.ok) {
        references.push(outcome.reference);
      } else {
        errors.push(`${upload.filename ?? "attachment"}: ${outcome.error}`);
      }
    }
    if (references.length > 0) {
      await this.saveAttachmentsToDraft(sessionId, references);
      await this.postState();
    }
    return references;
  }

  /**
   * Tell the user only what they need to hear.
   *
   * Success is announced to screen readers and left off the screen: the attachment strip
   * already shows the images, so a "1 attachment added" banner is redundant. Failures do
   * get a visible message, because nothing else on screen explains an image that is not
   * there.
   */
  private async reportAttachmentOutcome(
    accepted: DraftAttachmentRef[],
    errors: string[],
  ): Promise<void> {
    if (errors.length === 0 && accepted.length === 0) {
      return;
    }
    await this.postEvent({
      data: {
        hasErrors: errors.length > 0,
        message:
          errors.length > 0
            ? errors.join("; ")
            : `${accepted.length} attachment${accepted.length === 1 ? "" : "s"} added`,
      },
      type: "attachmentFeedback",
    });
  }

  /**
   * Grant the webview read access to wherever the backend keeps attachment bytes.
   *
   * The path arrives with the `initialize` handshake, which is the earliest it can
   * possibly be known, and that timing is the whole point: resource roots can only be
   * granted by reassigning `webview.options`, and VS Code reloads the webview document
   * when they change. Remembering the path across launches means the reload happens at
   * most once ever — on the very first run after install, during bootstrap, before the
   * user can type. Learning it later (on the first paste, as this once did) reloaded the
   * transcript at the exact moment an image was being attached.
   */
  private adoptAttachmentRoot(root: string | null): void {
    this.attachmentRootResolved = true;
    if (!root || root === this.attachmentRoot) {
      return;
    }
    this.attachmentRoot = root;
    void this.deps.attachmentRootMemento?.update(ATTACHMENT_ROOT_MEMENTO_KEY, root);
    // Every attachment URL is built from this root, so history images resolved before it
    // arrived have to be resolved again.
    this.stateStore.setAttachmentUriResolver(this.historyAttachmentResolver());
    this.reloadWebviewResourceRoots();
  }

  private lookupRetryableUserMessage(
    sessionId: string,
    messageId: string,
  ): { segments?: WebviewMessageSegment[]; submitKind: UserSubmitKind; text: string } | null {
    const session = this.peekState().sessionViews[sessionId];
    const message = session?.timeline.find(
      (item): item is WebviewMessageBlock =>
        item.type === "message" && item.kind === "user" && item.id === messageId,
    );
    if (
      !message ||
      message.deliveryState !== "failed" ||
      message.retryable !== true ||
      (message.submitKind !== "prompt" && message.submitKind !== "steer")
    ) {
      return null;
    }
    return {
      segments: message.segments,
      submitKind: message.submitKind,
      text: message.text,
    };
  }

  private async sendUserMessage(
    sessionId: string,
    submitKind: UserSubmitKind,
    text: string,
    segments?: WebviewMessageSegment[],
    options?: {
      messageId?: string;
      retrying?: boolean;
    },
  ): Promise<void> {
    const userMessageId = options?.messageId ?? randomUUID();
    // Steering messages join a turn already in flight and carry no attachments.
    const attachments =
      submitKind === "prompt"
        ? this.draftStore.peek(sessionId).attachments
        : [];
    this.stateStore.setActiveSession(sessionId);
    if (options?.retrying) {
      this.stateStore.markLocalUserMessagePending(sessionId, userMessageId);
    } else {
      this.stateStore.appendLocalUserMessage(sessionId, text, {
        // The optimistic bubble shows the same references the strip was showing, so the
        // attachment appears instantly without a byte moving anywhere.
        attachments: attachments.map((attachment) => this.toAttachmentView(attachment)),
        messageId: userMessageId,
        segments,
        submitKind,
      });
    }
    await this.postState();
    try {
      const response = await this.deps.messenger.request({
        params: {
          // Hashes only. The bytes went across once, at paste time.
          attachments: attachments.map((attachment) => ({
            blobSha: attachment.blobSha,
            filename: attachment.filename,
            kind: attachment.kind,
            mimeType: attachment.mimeType,
            providerSha: attachment.providerSha ?? null,
          })),
          segments: segments as ServeContentSegment[] | undefined,
          userMessageId,
        },
        sessionId,
        text,
        type: submitKind,
      });
      if (!response.success) {
        this.stateStore.markLocalUserMessageFailed(
          sessionId,
          userMessageId,
          response.error ?? `Tomcat ${submitKind} failed`,
          true,
        );
      } else {
        this.stateStore.markLocalUserMessageConfirmed(sessionId, userMessageId);
        if (submitKind === "prompt") {
          // The acknowledgement is the single signal that clears the draft. Clearing on
          // optimism would lose the user's message whenever a send failed. The webview
          // drops any attachment feedback on its own once the strip empties, so there is
          // nothing to tell it here.
          await this.draftStore.discard(sessionId);
          this.stateStore.clearPendingAttachments(sessionId);
          await this.syncImagePreviewPanel(sessionId);
        }
      }
    } catch (error) {
      this.stateStore.markLocalUserMessageFailed(
        sessionId,
        userMessageId,
        formatBridgeError(
          submitKind === "prompt" ? "send the message" : "send the steering message",
          error,
        ),
        false,
      );
    }
    await this.refreshSessionState(sessionId, { trustBusy: true });
    await this.refreshSessions();
    await this.postState();
  }

  /** Reference view for a sent attachment, used by history bubbles. */
  private toAttachmentView(attachment: DraftAttachmentRef): WebviewAttachmentView {
    const uris = this.view
      ? resolveAttachmentUris(this.view.webview, this.attachmentRoot, {
          blobSha: attachment.blobSha,
          hasThumb: attachment.hasThumb,
        })
      : null;
    return {
      blobSha: attachment.blobSha,
      bytes: attachment.bytes,
      filename: attachment.filename,
      fullUri: uris?.fullUri ?? null,
      hasThumb: Boolean(attachment.hasThumb),
      id: attachment.id,
      kind: attachment.kind,
      mimeType: attachment.mimeType,
      path: attachment.sourcePath ?? null,
      thumbUri: uris?.thumbUri ?? null,
    };
  }

  private async postContextSearchResult(
    intent: Extract<WebviewIntent, { type: "searchContext" }>,
    payload?: {
      matches: Extract<HostEventFrameContent, { type: "contextSearchResult" }>["matches"];
      truncated: boolean;
      workspaceAvailable: boolean;
    },
  ): Promise<void> {
    await this.postEvent({
      matches: payload?.matches ?? [],
      query: intent.data.query,
      requestId: intent.data.requestId,
      sessionId: intent.data.sessionId ?? null,
      truncated: payload?.truncated ?? false,
      type: "contextSearchResult",
      workspaceAvailable: payload?.workspaceAvailable,
    });
  }

  private async handleContextSearch(
    intent: Extract<WebviewIntent, { type: "searchContext" }>,
  ): Promise<void> {
    this.lastContextSearchIntent = intent;
    this.contextSearchTokenSource?.cancel();
    this.contextSearchTokenSource?.dispose();
    const tokenSource = new vscode.CancellationTokenSource();
    this.contextSearchTokenSource = tokenSource;
    try {
      const result = await this.contextSearch.search({
        kind: intent.data.kind,
        query: intent.data.query,
        token: tokenSource.token,
      });
      await this.postContextSearchResult(intent, {
        matches: result.matches,
        truncated: result.truncated,
        workspaceAvailable: result.workspaceAvailable,
      });
    } catch (error) {
      if (!tokenSource.token.isCancellationRequested) {
        console.error("Tomcat context search failed", error);
      }
      await this.postContextSearchResult(intent);
    } finally {
      if (this.contextSearchTokenSource === tokenSource) {
        this.contextSearchTokenSource = undefined;
      }
      tokenSource.dispose();
    }
  }

  private lookupApprovalSessionId(requestId: string): string | null {
    for (const session of Object.values(this.peekState().sessionViews)) {
      const approval = session.timeline.find(
        (item): item is WebviewApprovalCard =>
          item.type === "approval" && item.request.requestId === requestId,
      );
      if (approval) {
        return approval.sessionId ?? session.sessionId;
      }
    }
    return null;
  }

  private async bootstrap(): Promise<void> {
    await this.ensureInitialized();
    await this.refreshModels();
    const sessions = await this.sessionPool.refresh();
    this.stateStore.syncSessionList(sessions);
    const preferredSessionId =
      this.sessionPool.pickDefaultSession(sessions) ??
      this.initialized?.sessionId ??
      null;
    if (!preferredSessionId) {
      const sessionId = await this.sessionPool.createSession(this.deps.getDefaultCwd());
      await this.selectSession(sessionId);
      return;
    }
    await this.selectSession(preferredSessionId);
  }

  private async ensureInitialized(): Promise<InitializeResult> {
    if (this.initialized) {
      return this.initialized;
    }
    this.initialized = await this.deps.initialize();
    this.adoptAttachmentRoot(this.initialized.attachmentRoot);
    return this.initialized;
  }

  private async handleIntent(intent: Exclude<WebviewIntent, { type: "__test.dom_snapshot" }>): Promise<void> {
    switch (intent.type) {
      case "ready":
        this.isReady = true;
        this.stateStore.setReady(true);
        for (const waiter of [...this.readyWaiters]) {
          clearTimeout(waiter.timeout);
          waiter.resolve();
          this.readyWaiters.delete(waiter);
        }
        await this.bootstrap();
        return;
      case "listSessions":
        await this.refreshSessions();
        return;
      case "listCheckpoints":
        await this.refreshCheckpoints(intent.data.sessionId);
        await this.postState();
        return;
      case "resyncSessionView":
        await this.postSessionView(intent.data.sessionId);
        return;
      case "loadOlderHistory":
        await this.loadOlderHistory(intent.data.sessionId);
        return;
      case "newSession": {
        await this.ensureInitialized();
        const sessionId = await this.sessionPool.createSession(
          intent.data?.cwd ?? this.deps.getDefaultCwd(),
        );
        await this.selectSession(sessionId);
        return;
      }
      case "switchSession":
        await this.switchSessionView(intent.data.sessionId);
        return;
      case "closeSession": {
        const closed = await this.sessionPool.release(intent.data.sessionId);
        if (closed) {
          await this.refreshSessions();
          const fallback = this.sessionPool.pickDefaultSession(this.currentStateToSessionList());
          if (fallback) {
            await this.refreshSessionState(fallback, { trustBusy: true });
            await this.refreshSessionHistory(fallback);
            await this.refreshCheckpoints(fallback);
            this.stateStore.setActiveSession(fallback);
          } else {
            this.stateStore.setActiveSession(null);
          }
          await this.postState();
        }
        return;
      }
      case "prompt":
      case "steer": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSession(intent.data.sessionId ?? null);
        if (!sessionId) {
          await this.postState();
          return;
        }
        await this.sendUserMessage(
          sessionId,
          intent.type,
          intent.data.text,
          intent.data.segments,
          {
            messageId: intent.data.userMessageId,
          },
        );
        return;
      }
      case "retryUserMessage": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSession(intent.data.sessionId);
        if (!sessionId) {
          await this.postState();
          return;
        }
        const retry = this.lookupRetryableUserMessage(sessionId, intent.data.messageId);
        if (!retry) {
          return;
        }
        await this.sendUserMessage(
          sessionId,
          retry.submitKind,
          retry.text,
          retry.segments,
          {
            messageId: intent.data.messageId,
            retrying: true,
          },
        );
        return;
      }
      case "resolveDrop": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSessionWithoutHistory(
          intent.data.sessionId ?? null,
        );
        if (!sessionId) {
          await this.postState();
          return;
        }
        const uris: vscode.Uri[] = [];
        for (const rawUri of intent.data.uris) {
          try {
            uris.push(vscode.Uri.parse(rawUri));
          } catch {
            // Ignore malformed drop payload entries; the editor keeps the rest.
          }
        }
        await this.ingestPickedUris(sessionId, uris);
        return;
      }
      case "searchContext":
        await this.handleContextSearch(intent);
        return;
      case "showWarningMessage":
        await vscode.window.showWarningMessage(intent.data.message);
        return;
      case "pickContext": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSession(intent.data?.sessionId ?? null);
        if (!sessionId) {
          await this.postState();
          return;
        }
        const picks = await this.showOpenDialog(buildAttachmentOpenDialogOptions());
        if (!picks?.length) {
          return;
        }
        await this.ingestPickedUris(sessionId, picks);
        return;
      }
      case "cacheAttachmentThumbnail": {
        await this.storeGeneratedThumbnail(intent.data);
        return;
      }
      case "syncComposerDraft": {
        const { sessionId, text, segments } = intent.data;
        if (!sessionId) {
          return;
        }
        try {
          // Memory is updated synchronously; the disk write is debounced. No protocol
          // traffic at all — this is the path that used to fire a full draft round trip
          // on every keystroke.
          //
          // Deliberately no `postState()`: the webview is where this draft came from, so
          // echoing the whole snapshot back at it every 250ms of typing is pure cost. The
          // snapshot only needs pushing when the host changes something the webview does
          // not already know about.
          await this.saveDraftContent(sessionId, text, segments);
        } catch (error) {
          console.warn("Tomcat failed to save the Composer draft", error);
        }
        return;
      }
      case "attachFiles": {
        const { sessionId, files } = intent.data;
        if (!sessionId || files.length === 0) return;

        const uploads: AttachmentUpload[] = [];
        const errors: string[] = [];
        for (const file of files) {
          // Validate before the bytes go any further. The backend validates again — this
          // pass exists so the user hears about an oversized paste immediately.
          const validation = validateAttachmentCandidate(file);
          if (!validation.ok) {
            errors.push(`${file.filename ?? "attachment"}: ${validation.error}`);
            continue;
          }
          uploads.push({
            dataBase64: file.dataBase64,
            filename: validation.filename,
            kind: validation.kind,
            mimeType: validation.mimeType,
            providerBase64: file.providerBase64,
            providerMimeType: file.providerMimeType,
            providerText: file.providerText,
            sourcePath: typeof file.sourcePath === "string" ? file.sourcePath : null,
            thumbBase64: file.thumbBase64,
          });
          // The webview reports what it could not derive (a thumbnail, an SVG raster).
          // These are degradations, not failures, but the user should still know.
          errors.push(
            ...(file.warnings ?? []).map(
              (warning) => `${validation.filename}: ${warning}`,
            ),
          );
        }

        const accepted = await this.ingestUploads(sessionId, uploads, errors);
        await this.postEvent({
          items: accepted.map((reference) => ({
            filename: reference.filename,
            id: reference.id,
            mimeType: reference.mimeType,
          })),
          type: "attachFilesResult",
        });
        await this.reportAttachmentOutcome(accepted, errors);
        return;
      }
      case "removeDraftAttachment": {
        const { sessionId, attachmentId } = intent.data;
        if (!sessionId || !attachmentId) {
          return;
        }
        await this.removeDraftAttachment(sessionId, attachmentId);
        await this.postState();
        return;
      }
      case "openImagePreview": {
        const { sessionId, attachmentId } = intent.data;
        if (!sessionId || !attachmentId) {
          return;
        }
        // Delegate to the ImagePreviewPanel handler
        await this.openImagePreviewPanel(sessionId, attachmentId);
        return;
      }
      case "removeAttachment": {
        const sessionId = intent.data.sessionId ?? this.peekState().activeSessionId;
        if (!sessionId) {
          return;
        }
        await this.removeDraftAttachment(sessionId, intent.data.attachmentId);
        await this.postState();
        return;
      }
      case "interrupt": {
        const sessionId = await this.ensureWebviewSessionWithoutHistory(
          intent.data?.sessionId ?? this.peekState().activeSessionId,
        );
        if (!sessionId) {
          await this.postState();
          return;
        }
        await this.deps.messenger.request({
          sessionId,
          type: "interrupt",
        });
        return;
      }
      case "restoreCheckpoint": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSessionWithoutHistory(intent.data.sessionId);
        if (!sessionId) {
          await this.postState();
          return;
        }
        try {
          await this.deps.sessionRouter.restoreCheckpoint(
            sessionId,
            intent.data.checkpointId,
            intent.data.revertFiles,
          );
        } catch (error) {
          this.stateStore.appendMessage(
            sessionId,
            "error",
            formatBridgeError("restore checkpoint", error),
          );
          await this.postState();
          return;
        }
        await this.refreshSessionState(sessionId, { trustBusy: true });
        await this.refreshSessionHistory(sessionId);
        await this.refreshCheckpoints(sessionId);
        await this.refreshSessions();
        await this.postState();
        return;
      }
      case "setModel": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSession(intent.data.sessionId ?? null);
        if (!sessionId) {
          await this.postState();
          return;
        }
        try {
          const response = await this.deps.messenger.sendSetModel(sessionId, intent.data.modelId);
          if (!response.success) {
            this.stateStore.appendMessage(
              sessionId,
              "error",
              response.error ?? "Unable to switch model",
            );
          }
        } catch (error) {
          this.stateStore.appendMessage(
            sessionId,
            "error",
            formatBridgeError("switch models", error),
          );
        }
        await this.refreshModels();
        await this.refreshSessionState(sessionId, { trustBusy: true });
        await this.postState();
        return;
      }
      case "setThinkingLevel": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSession(intent.data.sessionId ?? null);
        if (!sessionId) {
          await this.postState();
          return;
        }
        try {
          const response = await this.deps.messenger.sendSetThinkingLevel(
            sessionId,
            intent.data.modelId,
            intent.data.level,
          );
          if (!response.success) {
            this.stateStore.appendMessage(
              sessionId,
              "error",
              response.error ?? "Unable to change reasoning effort",
            );
          }
        } catch (error) {
          this.stateStore.appendMessage(
            sessionId,
            "error",
            formatBridgeError("change reasoning effort", error),
          );
        }
        await this.refreshSessionState(sessionId, { trustBusy: true });
        await this.postState();
        return;
      }
      case "openModelSettings":
        if (!hasAnyModelAdminCapability(await this.ensureInitialized())) {
          return;
        }
        this.deps.openModelSettings?.(intent.data?.route ?? "models");
        return;
      case "setBuildModel": {
        await vscode.workspace
          .getConfiguration(TOMCAT_CONFIG_SECTION)
          .update("plan.buildModel", intent.data.modelId, vscode.ConfigurationTarget.Global);
        this.stateStore.setBuildModel(intent.data.modelId);
        await this.postState();
        return;
      }
      case "setPlanMode": {
        await this.ensureInitialized();
        const sessionId = await this.ensureWebviewSessionWithoutHistory(
          intent.data.sessionId ?? null,
        );
        if (!sessionId) {
          await this.postState();
          return;
        }
        if (intent.data.action === "build") {
          await this.runPlanBuild(sessionId, intent.data.planId);
          return;
        }
        try {
          const response = await this.deps.messenger.sendSetPlanMode({
            action: intent.data.action,
            planId: intent.data.planId,
            sessionId,
          });
          if (!response.success) {
            this.stateStore.appendMessage(
              sessionId,
              "error",
              response.error ?? "Unable to change plan mode",
            );
          }
        } catch (error) {
          this.stateStore.appendMessage(
            sessionId,
            "error",
            formatBridgeError("change plan mode", error),
          );
        }
        await this.refreshSessionState(sessionId, { trustBusy: true });
        await this.postState();
        return;
      }
      case "openFile":
        this.openFileObserved = true;
        try {
          await this.deps.ide.showFile(intent.data.path, intent.data.line);
        } catch (error) {
          await vscode.window.showErrorMessage(
            formatBridgeError(`open file ${intent.data.path}`, error),
          );
        }
        return;
      case "openDiff": {
        const toolInfo = this.findToolCard(intent.data.toolCallId);
        const tool = toolInfo?.tool;
        const displayPath =
          tool?.display?.kind === "file"
            ? tool.display.file
            : typeof tool?.args?.path === "string"
              ? tool.args.path
              : null;
        if (!tool || !displayPath) {
          return;
        }
        try {
          if (tool.diff?.length) {
            const { after, before } = reconstructDiffPair(tool.diff);
            await this.deps.ide.openReconstructedDiff(
              intent.data.toolCallId,
              displayPath,
              before,
              after,
            );
          } else {
            await this.deps.ide.showFile(displayPath);
            const sessionId = toolInfo?.sessionId ?? this.peekState().activeSessionId;
            if (sessionId) {
              this.stateStore.appendMessage(
                sessionId,
                "notice",
                "File too large for inline diff. Opened the current file instead.",
              );
              await this.postState();
            }
          }
        } catch (error) {
          const sessionId = this.peekState().activeSessionId;
          if (sessionId) {
            this.stateStore.appendMessage(
              sessionId,
              "error",
              formatBridgeError(`open diff ${displayPath}`, error),
            );
            await this.postState();
          }
        }
        return;
      }
      case "openPlanFile":
        try {
          await this.deps.ide.openWith(intent.data.path, "tomcat.planPreview");
        } catch (error) {
          try {
            await this.deps.ide.showFile(intent.data.path);
          } catch {
            const sessionId = this.peekState().activeSessionId;
            if (sessionId) {
              this.stateStore.appendMessage(
                sessionId,
                "error",
                formatBridgeError(`open plan file ${intent.data.path}`, error),
              );
              await this.postState();
            }
          }
        }
        return;
      case "answerQuestion": {
        const pending = this.pendingQuestions.get(intent.data.requestId);
        if (!pending) {
          const sessionId =
            this.lookupApprovalSessionId(intent.data.requestId)
            ?? this.peekState().activeSessionId;
          if (sessionId) {
            this.stateStore.appendMessage(
              sessionId,
              "notice",
              "This question is no longer active. Please ask again if you still need it.",
            );
            await this.postState();
          }
          return;
        }
        pending.resolve(
          normalizeAskQuestionResponse(intent.data.requestId, intent.data.result),
        );
        return;
      }
    }
  }

  private async handleServeEvent(event: ServeEvent): Promise<void> {
    if (
      event.type === "tool_execution_start" &&
      isMutationTool(event.toolName) &&
      typeof this.deps.ide.rememberToolStart === "function"
    ) {
      try {
        await this.deps.ide.rememberToolStart(event.toolCallId, event.args);
      } catch (error) {
        console.warn("Tomcat webview failed to capture tool start snapshot", error);
      }
    }
    if (
      event.type === "tool_execution_end" &&
      event.display?.kind === "file" &&
      isMutationTool(event.toolName) &&
      typeof this.deps.ide.rememberToolResult === "function"
    ) {
      try {
        await this.deps.ide.rememberToolResult(
          event.toolCallId,
          event.display.file,
          event.display.diff?.length ? reconstructDiffPair(event.display.diff) : undefined,
        );
      } catch (error) {
        console.warn("Tomcat webview failed to capture tool result snapshot", error);
      }
    }
    if (
      event.type === "agent_end"
      && event.sessionId
      && typeof event.error === "string"
      && event.error !== "interrupted"
    ) {
      this.sessionsAwaitingErrorHistoryRefresh.add(event.sessionId);
    }
    const mutation = this.stateStore.applyEvent(event);
    await this.maybeAutoOpenPlanPreview(event);
    const previewRefresh = extractPlanPreviewRefreshArgs(event);
    if (previewRefresh && this.deps.refreshPlanPreview) {
      void Promise.resolve(
        this.deps.refreshPlanPreview(previewRefresh.planId, previewRefresh.path),
      ).catch((error) => {
        console.warn("Tomcat webview failed to refresh the plan preview", error);
      });
    }
    let requiresSessionView = false;
    if (event.sessionId) {
      if (shouldReconcileSessionState(event)) {
        await this.refreshSessionState(event.sessionId, { trustBusy: false });
        requiresSessionView = true;
      }
      if (event.type === "turn_end") {
        await this.refreshCheckpoints(event.sessionId);
        requiresSessionView = true;
      }
      if (event.type === "agent_idle" && this.sessionsAwaitingErrorHistoryRefresh.has(event.sessionId)) {
        this.sessionsAwaitingErrorHistoryRefresh.delete(event.sessionId);
        await this.refreshSessionHistory(event.sessionId);
        requiresSessionView = true;
      }
    }
    await this.postEvent(event);
    const force = shouldForceServeEventFlush(event);
    if (!event.sessionId) {
      await this.broadcastFullState({ force });
      return;
    }
    if (requiresSessionView) {
      await this.broadcastSession(event.sessionId, { force });
      return;
    }
    await this.broadcastMutation(mutation, { force });
  }

  /**
   * Record the persisted `planId -> path` on `plan.create`, but only auto-open the
   * preview once the reviewer finishes (`plan.review`). This avoids stealing focus
   * while the draft is still under review, yet still opens deterministically once
   * the reviewed plan is ready to read. Deduped per path so repeated review events
   * don't reopen the same preview.
   */
  private async maybeAutoOpenPlanPreview(event: ServeEvent): Promise<void> {
    if (event.type === "plan.create") {
      const planId = "planId" in event && typeof event.planId === "string" ? event.planId : "";
      const path = "path" in event && typeof event.path === "string" ? event.path : "";
      if (planId && path && !this.autoOpenedPlanPaths.has(path)) {
        this.pendingPlanOpenByPlanId.set(planId, path);
      }
      return;
    }
    if (event.type !== "plan.review") {
      return;
    }
    const planId = "planId" in event && typeof event.planId === "string" ? event.planId : "";
    const path = planId ? (this.pendingPlanOpenByPlanId.get(planId) ?? "") : "";
    if (!path || this.autoOpenedPlanPaths.has(path)) {
      return;
    }
    if (typeof this.deps.ide.openWith !== "function") {
      return;
    }
    this.autoOpenedPlanPaths.add(path);
    if (planId) {
      this.pendingPlanOpenByPlanId.delete(planId);
    }
    try {
      await this.deps.ide.openWith(path, "tomcat.planPreview");
    } catch {
      try {
        await this.deps.ide.showFile(path);
      } catch {
        // Best-effort: opening is a convenience, never fatal to event handling.
      }
    }
  }

  private async handleWebviewMessage(message: unknown): Promise<void> {
    if (!isWebviewIntent(message)) {
      return;
    }
    if (message.type === "__test.dom_snapshot") {
      this.domSnapshots.resolve(message.messageId, message.data);
      return;
    }
    await this.handleIntent(message);
  }

  private currentStateToSessionList() {
    const state = this.peekState();
    return {
      activeSessionId: state.activeSessionId,
      scope: "disk" as const,
      sessions: state.sessions.map((session) => ({
        busy: session.busy,
        isCurrent: session.isCurrent,
        sessionId: session.sessionId,
        title: session.title,
        updatedAt: session.updatedAt,
      })),
    };
  }

  private async ensureWebviewSession(sessionId: string | null): Promise<string | null> {
    const target = sessionId ?? this.peekState().activeSessionId;
    if (!target) {
      const created = await this.sessionPool.createSession(this.deps.getDefaultCwd());
      await this.selectSession(created);
      return created;
    }
    await this.selectSession(target);
    return target;
  }

  private async ensureWebviewSessionWithoutHistory(
    sessionId: string | null,
  ): Promise<string | null> {
    const target = sessionId ?? this.peekState().activeSessionId;
    if (!target) {
      const created = await this.sessionPool.createSession(this.deps.getDefaultCwd());
      this.stateStore.setActiveSession(created);
      await this.sessionPool.switchTo(created);
      await this.refreshSessions();
      return created;
    }

    this.stateStore.setActiveSession(target);
    await this.sessionPool.switchTo(target);
    return target;
  }

  private async postEvent(content: HostEventFrameContent): Promise<void> {
    await this.postMessage({
      channel: "event",
      content,
      messageId: createHostFrameMessageId("event"),
    });
  }

  private async flushStateBroadcastPlan(
    plan: StateBroadcasterFlushPlan,
  ): Promise<void> {
    if (!this.view || !this.isReady) {
      return;
    }
    if (plan.fullState) {
      await this.postStateFrame();
      return;
    }
    for (const sessionId of plan.sessionIds) {
      await this.postSessionView(sessionId);
    }
    for (const patch of plan.sessionPatches) {
      if (!this.sessionPatchFramesEnabled) {
        await this.postSessionView(patch.sessionId);
        continue;
      }
      await this.postSessionPatch(
        patch.sessionId,
        patch.seq,
        patch.ops,
      );
    }
  }

  private async broadcastFullState(
    options: { force?: boolean } = {},
  ): Promise<void> {
    this.stateBroadcaster.markFullState();
    if (options.force) {
      await this.stateBroadcaster.forceFlush();
    }
  }

  private async broadcastSession(
    sessionId: string | null | undefined,
    options: { force?: boolean } = {},
  ): Promise<void> {
    if (!sessionId) {
      if (options.force) {
        await this.stateBroadcaster.forceFlush();
      }
      return;
    }
    this.stateBroadcaster.markSession(sessionId);
    if (options.force) {
      await this.stateBroadcaster.forceFlush();
    }
  }

  private async broadcastMutation(
    mutation: SessionRenderMutation,
    options: { force?: boolean } = {},
  ): Promise<void> {
    if (mutation.kind === "patch") {
      if (this.sessionPatchFramesEnabled) {
        this.stateBroadcaster.appendPatch(mutation.sessionId, mutation.ops);
      } else {
        this.stateBroadcaster.markSession(mutation.sessionId);
      }
    } else if (mutation.kind === "session") {
      this.stateBroadcaster.markSession(mutation.sessionId);
    }
    if (options.force) {
      await this.stateBroadcaster.forceFlush();
    }
  }

  private async postMessage(frame: HostToWebviewFrame): Promise<void> {
    if (!this.view) {
      return;
    }
    await this.view.webview.postMessage(frame);
  }

  private async postState(): Promise<void> {
    await this.postStateFrame();
  }

  private async postStateFrame(): Promise<void> {
    if (!this.view || !this.isReady) {
      return;
    }
    const snapshot = this.decorateStateSnapshot(
      await this.enrichPlanCards(this.stateStore.snapshot()),
    );
    await this.postMessage({
      channel: "state",
      content: snapshot,
      messageId: createHostFrameMessageId("state"),
    });
  }

  private async postSessionView(sessionId: string): Promise<void> {
    if (!this.view || !this.isReady) {
      return;
    }
    const view = this.stateStore.snapshotSession(sessionId);
    if (!view) {
      return;
    }
    const tab = this.stateStore.snapshotSessionTab(sessionId);
    await this.postMessage({
      channel: "sessionView",
      content: {
        sessionId,
        tab,
        view: await this.enrichPlanSession(view),
      },
      messageId: createHostFrameMessageId("session-view"),
    });
  }

  private async postSessionPatch(
    sessionId: string,
    seq: number,
    ops: StateBroadcasterFlushPlan["sessionPatches"][number]["ops"],
  ): Promise<void> {
    if (!this.view || !this.isReady || ops.length === 0) {
      return;
    }
    await this.postMessage({
      channel: "sessionPatch",
      content: {
        ops,
        seq,
        sessionId,
      },
      messageId: createHostFrameMessageId("session-patch"),
    });
  }

  async postInsertReference(sessionId: string, reference: WebviewReference): Promise<void> {
    await this.postEvent({
      reference,
      sessionId,
      type: "insertReference",
    });
  }

  private async enrichPlanSession(
    session: WebviewStateSnapshot["sessionViews"][string],
  ): Promise<WebviewStateSnapshot["sessionViews"][string]> {
    const planCards = session.timeline.filter(
      (item): item is WebviewPlanFileCard => item.type === "plan",
    );
    const createPlanTools = session.timeline.filter(
      (item): item is WebviewToolCard =>
        item.type === "tool" &&
        item.toolName === "create_plan" &&
        item.planActivity?.kind === "create" &&
        typeof item.planPath === "string" &&
        item.planPath.length > 0,
    );
    await Promise.all(
      [...planCards, ...createPlanTools].map(async (item) => {
        const planPath = item.type === "plan" ? item.path : item.planPath;
        if (!planPath) {
          return;
        }
        const metadata = await readPlanMetadata(planPath, this.planMetadataCache);
        if (item.type === "plan") {
          if (metadata.title) {
            item.title = metadata.title;
          } else {
            delete item.title;
          }
          if (metadata.overview) {
            item.overview = metadata.overview;
          } else {
            delete item.overview;
          }
          return;
        }
        const planActivity = item.planActivity;
        if (!planActivity) {
          return;
        }
        item.planActivity = {
          ...planActivity,
          overview: metadata.overview ?? null,
          title: metadata.title ?? planActivity.title ?? null,
        };
      }),
    );
    return session;
  }

  private async enrichPlanCards(snapshot: WebviewStateSnapshot): Promise<WebviewStateSnapshot> {
    const sessions = Object.values(snapshot.sessionViews);
    await Promise.all(sessions.map((session) => this.enrichPlanSession(session)));
    return snapshot;
  }

  private refreshHtml(): void {
    if (!this.view) {
      return;
    }
    this.view.webview.html = this.renderHtml(this.view.webview);
  }

  private readBuildModelConfig(): string {
    return (
      vscode.workspace
        .getConfiguration(TOMCAT_CONFIG_SECTION)
        .get<string>("plan.buildModel", "") ?? ""
    );
  }

  /** Re-read `tomcat.plan.buildModel` and push it to the webview (config sync). */
  async syncBuildModel(): Promise<void> {
    this.stateStore.setBuildModel(this.readBuildModelConfig());
    if (this.isReady) {
      await this.postState();
    }
  }

  /**
   * Single build path shared by the chat PlanFileCard and the plan preview
   * editor: apply the global build model (when set) before entering build mode.
   */
  private async runPlanBuild(sessionId: string, planId?: string | null): Promise<void> {
    const buildModel = this.readBuildModelConfig();
    try {
      if (buildModel) {
        const modelResponse = await this.deps.messenger.sendSetModel(sessionId, buildModel);
        if (!modelResponse.success) {
          this.stateStore.appendMessage(
            sessionId,
            "error",
            modelResponse.error ?? "Unable to switch model",
          );
        }
      }
      const response = await this.deps.messenger.sendSetPlanMode({
        action: "build",
        planId,
        sessionId,
      });
      if (!response.success) {
        this.stateStore.appendMessage(
          sessionId,
          "error",
          response.error ?? "Unable to change plan mode",
        );
      }
    } catch (error) {
      this.stateStore.appendMessage(
        sessionId,
        "error",
        formatBridgeError("change plan mode", error),
      );
    }
    if (buildModel) {
      await this.refreshModels();
    }
    await this.refreshSessionState(sessionId, { trustBusy: true });
    await this.postState();
  }

  /** Public build entry for the plan preview editor (ensures a session first). */
  async buildPlan(planId: string | null): Promise<void> {
    await this.ensureInitialized();
    const sessionId = await this.ensureWebviewSessionWithoutHistory(null);
    if (!sessionId) {
      await this.postState();
      return;
    }
    await this.runPlanBuild(sessionId, planId);
  }

  private async refreshModels(): Promise<void> {
    this.stateStore.setBuildModel(this.readBuildModelConfig());
    const initializeResult = await this.ensureInitialized();
    this.stateStore.setModelAdminSupported(
      hasAnyModelAdminCapability(initializeResult),
    );
    if (!hasServeCapability(initializeResult, SERVE_CAPABILITY_LIST_MODELS)) {
      this.stateStore.setAvailableModels([], {}, {});
      return;
    }
    const response = await this.deps.messenger.sendListModels().catch(() => null);
    if (!response) {
      this.stateStore.setAvailableModels([], {}, {});
      return;
    }
    if (!response.success) {
      this.stateStore.setAvailableModels([], {}, {});
      return;
    }
    const catalog = parseModelCatalog(response.payload);
    this.stateStore.setAvailableModels(
      catalog.ids,
      catalog.capabilities,
      catalog.reasoningLevels,
    );
  }

  private async refreshSessions(): Promise<void> {
    await this.ensureInitialized();
    const sessions = await this.sessionPool.refresh();
    this.stateStore.syncSessionList(sessions);
    await this.postState();
  }

  private async refreshSessionState(
    sessionId: string,
    options: {
      trustBusy?: boolean;
    } = {},
  ): Promise<void> {
    const state = await this.deps.sessionRouter.getState(sessionId).catch(() => null);
    if (!state) {
      return;
    }
    this.stateStore.applySessionState(state, { trustBusy: options.trustBusy ?? true });
  }

  private bumpHistoryFetchGen(sessionId: string): number {
    const next = (this.historyFetchGen.get(sessionId) ?? 0) + 1;
    this.historyFetchGen.set(sessionId, next);
    return next;
  }

  private currentHistoryFetchGen(sessionId: string): number {
    return this.historyFetchGen.get(sessionId) ?? 0;
  }

  private async refreshSessionHistory(sessionId: string): Promise<void> {
    if (typeof this.deps.sessionRouter.getMessages !== "function") {
      return;
    }
    const fetchGen = this.bumpHistoryFetchGen(sessionId);
    const history = await this.deps.sessionRouter.getMessages(sessionId, {
      attachmentMode: HISTORY_ATTACHMENT_MODE,
      limit: HISTORY_PAGE_ENTRIES,
    }).catch(() => null);
    if (this.currentHistoryFetchGen(sessionId) !== fetchGen) {
      return;
    }
    if (!history || history.sessionId !== sessionId) {
      return;
    }
    this.stateStore.hydrateHistory(sessionId, history);
    await this.syncImagePreviewPanel(sessionId);
  }

  private async refreshCheckpoints(sessionId: string): Promise<void> {
    if (typeof this.deps.sessionRouter.listCheckpoints !== "function") {
      return;
    }
    const checkpoints = await this.deps.sessionRouter.listCheckpoints(sessionId).catch(() => null);
    if (!checkpoints || checkpoints.sessionId !== sessionId) {
      return;
    }
    this.stateStore.setCheckpoints(sessionId, checkpoints.checkpoints);
  }

  private async loadOlderHistory(sessionId: string): Promise<void> {
    if (typeof this.deps.sessionRouter.getMessages !== "function") {
      return;
    }
    const session = this.peekState().sessionViews[sessionId];
    if (!session?.hasMoreHistory || session.historyLoading !== false) {
      return;
    }
    const cursor = this.stateStore.getOldestHistoryCursor(sessionId);
    if (!cursor) {
      return;
    }
    const fetchGen = this.currentHistoryFetchGen(sessionId);
    this.stateStore.setHistoryLoading(sessionId, true);
    await this.postState();
    const history = await this.deps.sessionRouter.getMessages(sessionId, {
      attachmentMode: HISTORY_ATTACHMENT_MODE,
      cursor,
      limit: HISTORY_PAGE_ENTRIES,
    }).catch(() => null);
    if (this.currentHistoryFetchGen(sessionId) !== fetchGen) {
      this.stateStore.setHistoryLoading(sessionId, false);
      await this.postState();
      return;
    }
    if (!history || history.sessionId !== sessionId) {
      this.stateStore.setHistoryLoading(sessionId, false);
      await this.postState();
      return;
    }
    this.stateStore.prependHistory(sessionId, history);
    await this.postState();
  }

  /**
   * Load a session's draft into the snapshot the webview renders from.
   *
   * Runs on session open and on switch. Attachments whose bytes the backend no longer
   * has are marked unavailable rather than dropped: silently deleting an image the user
   * attached is worse than showing it struck through with a remove button.
   */
  private async hydrateDraft(sessionId: string): Promise<void> {
    try {
      const draft = await this.draftStore.hydrate(sessionId, (candidate) =>
        this.sessionExists(candidate),
      );
      await this.applyDraftToState(sessionId, draft);
      await this.syncImagePreviewPanel(sessionId);
      await this.postState();
    } catch (error) {
      // A draft that will not load must never block the composer. Worst case the user
      // types again; refusing to render the input box would be unusable.
      console.warn("Tomcat failed to hydrate the Composer draft", error);
    }
  }

  /**
   * Does this session still exist on the backend?
   *
   * Used to drop drafts for sessions that were deleted elsewhere, so their files do not
   * accumulate forever. An unreachable backend answers "yes" — keeping a draft we cannot
   * verify is the safe direction.
   */
  private async sessionExists(sessionId: string): Promise<boolean> {
    try {
      const payload = await this.deps.sessionRouter.listSessions();
      return payload.sessions.some((session) => session.sessionId === sessionId);
    } catch {
      return true;
    }
  }

  /**
   * The hash-to-URL mapping handed to the state store for history images.
   *
   * History comes from the transcript, which records hashes only, so images rebuilt on a
   * session switch or a reopened window need this to have any address at all. Bytes that
   * are gone are reported as unavailable rather than left as an image that will never
   * load: a placeholder that never resolves reads as a hang.
   */
  private historyAttachmentResolver(): AttachmentUriResolver {
    return (attachment) => {
      const uris = this.view
        ? resolveAttachmentUris(this.view.webview, this.attachmentRoot, attachment)
        : null;
      return {
        fullUri: uris?.fullUri ?? null,
        thumbUri: uris?.thumbUri ?? null,
        ...(this.hasBlobBytes(attachment.blobSha) ? {} : { unavailable: true }),
      };
    };
  }

  /** Does the backend still hold the bytes for this hash? */
  private hasBlobBytes(blobSha: string): boolean {
    // Root unknown yet: assume present. Re-checked once the handshake reports it.
    if (!this.attachmentRoot) return true;
    return fs.existsSync(path.join(this.attachmentRoot, "blobs", blobSha));
  }

  /**
   * Ask the backend which of a draft's attachments still have bytes behind them.
   *
   * One `get_state` call reports the attachment root; presence is then a plain
   * filesystem check, which is far cheaper than a round trip per attachment.
   */
  private markMissingAttachments(
    attachments: DraftAttachmentRef[],
  ): { available: DraftAttachmentRef[]; missing: DraftAttachmentRef[] } {
    const available: DraftAttachmentRef[] = [];
    const missing: DraftAttachmentRef[] = [];
    for (const attachment of attachments) {
      (this.hasBlobBytes(attachment.blobSha) ? available : missing).push(attachment);
    }
    return { available, missing };
  }

  /** Project a draft onto the webview snapshot, resolving hashes to URLs. */
  private async applyDraftToState(
    sessionId: string,
    draft: ComposerDraft,
  ): Promise<void> {
    this.stateStore.setComposerDraft(sessionId, {
      segments: draft.segments,
      text: draft.text,
    });
    const { available, missing } = this.markMissingAttachments(draft.attachments);
    if (missing.length > 0) {
      console.warn(
        `Tomcat draft for ${sessionId} references ${missing.length} attachment(s) whose bytes are gone`,
      );
    }
    this.stateStore.setPendingAttachments(
      sessionId,
      [
        ...available.map((attachment) => this.toPendingView(attachment, false)),
        ...missing.map((attachment) => this.toPendingView(attachment, true)),
      ],
    );
  }

  /**
   * Turn a stored reference into the view the webview renders.
   *
   * This is the whole of the host's involvement with image data: it maps hashes to URLs.
   * It never reads, decodes, or buffers a single image byte.
   */
  private toPendingView(
    attachment: DraftAttachmentRef,
    unavailable: boolean,
  ): WebviewPendingAttachment {
    const uris = this.view
      ? resolveAttachmentUris(this.view.webview, this.attachmentRoot, {
          blobSha: attachment.blobSha,
          hasThumb: attachment.hasThumb,
        })
      : null;
    return {
      blobSha: attachment.blobSha,
      bytes: attachment.bytes,
      filename: attachment.filename,
      fullUri: uris?.fullUri ?? null,
      hasThumb: Boolean(attachment.hasThumb),
      id: attachment.id,
      kind: attachment.kind,
      label: attachment.filename,
      mimeType: attachment.mimeType,
      path: attachment.sourcePath ?? null,
      thumbUri: uris?.thumbUri ?? null,
      ...(unavailable ? { unavailable: true } : {}),
    };
  }

  /** Record a text/reference edit. Returns as soon as memory is updated. */
  private async saveDraftContent(
    sessionId: string,
    text: string,
    segments: WebviewMessageSegment[],
  ): Promise<void> {
    const draft = this.draftStore.update(sessionId, (current) => ({
      ...current,
      segments,
      text,
    }));
    await this.applyDraftToState(sessionId, draft);
  }

  /**
   * Store a thumbnail the webview generated, and start using it.
   *
   * Runs for attachments that reached the backend without passing through a webview — the
   * file picker, and images replayed from history — because those have no thumbnail and
   * the strip will not render an original in place of one. Until this lands the user sees
   * a placeholder, so it is worth being quick, but a failure is only cosmetic.
   */
  private async storeGeneratedThumbnail(data: {
    blobSha: string;
    sessionId: string;
    thumbBase64: string;
  }): Promise<void> {
    const stored = await cacheAttachmentThumbnail(
      this.deps.messenger,
      data.sessionId,
      data.blobSha,
      data.thumbBase64,
    );
    if (!stored) {
      console.warn(`Tomcat could not store a thumbnail for ${data.blobSha}`);
      return;
    }

    // The draft holds its own copy of the reference, so flip the flag there too or the
    // next hydrate would ask the webview to build the same thumbnail again.
    const draft = this.draftStore.update(data.sessionId, (current) => ({
      ...current,
      attachments: current.attachments.map((attachment) =>
        attachment.blobSha === data.blobSha
          ? { ...attachment, hasThumb: true }
          : attachment,
      ),
    }));
    await this.applyDraftToState(data.sessionId, draft);
    this.adoptHistoryThumbnail(data.sessionId, data.blobSha);
    await this.postState();
  }

  /**
   * Point history images at a thumbnail that now exists.
   *
   * History views are built from the transcript rather than from the draft, so they need
   * their own pass — otherwise a sent message would keep showing a placeholder for the
   * rest of the session even though the thumbnail is on disk.
   */
  private adoptHistoryThumbnail(sessionId: string, blobSha: string): void {
    const webview = this.view?.webview;
    if (!webview) return;
    const uris = resolveAttachmentUris(webview, this.attachmentRoot, {
      blobSha,
      hasThumb: true,
    });
    if (!uris) return;
    this.stateStore.updateHistoryAttachments(sessionId, blobSha, {
      hasThumb: true,
      thumbUri: uris.thumbUri,
    });
  }

  /** Add freshly ingested attachments to the draft. */
  private async saveAttachmentsToDraft(
    sessionId: string,
    references: DraftAttachmentRef[],
  ): Promise<void> {
    const draft = this.draftStore.update(sessionId, (current) => ({
      ...current,
      attachments: [...current.attachments, ...references],
    }));
    await this.applyDraftToState(sessionId, draft);
    await this.syncImagePreviewPanel(sessionId);
  }

  /** Drop one attachment from the draft, leaving text and references alone. */
  private async removeDraftAttachment(
    sessionId: string,
    attachmentId: string,
  ): Promise<void> {
    const draft = this.draftStore.update(sessionId, (current) => ({
      ...current,
      attachments: current.attachments.filter(
        (attachment) => attachment.id !== attachmentId,
      ),
    }));
    await this.applyDraftToState(sessionId, draft);
    await this.syncImagePreviewPanel(sessionId);
  }

  /**
   * Build the preview panel's picture list from references.
   *
   * Every entry is a URL. The panel loads the full-resolution one for whatever is on
   * screen and the thumbnail for the filmstrip, which is the difference between decoding
   * one large bitmap and decoding all of them.
   */
  private imagePreviewSections(sessionId: string): PreviewSection[] {
    const session = this.peekState().sessionViews[sessionId];
    if (!session) return [];

    const toPicture = (attachment: WebviewAttachmentView) => ({
      filename: attachment.filename,
      fullUri: attachment.fullUri ?? "",
      id: attachment.id,
      mimeType: attachment.mimeType,
      thumbUri: attachment.thumbUri ?? attachment.fullUri ?? "",
    });

    const pendingPictures = (session.pendingAttachments ?? [])
      .filter(
        (attachment) =>
          attachment.kind === "image" && !attachment.unavailable && attachment.fullUri,
      )
      .map(toPicture);
    const pendingIds = new Set(pendingPictures.map((picture) => picture.id));

    const historySections = session.timeline.flatMap((item, messageIndex) => {
      if (item.type !== "message" || item.kind !== "user" || !item.attachments?.length) {
        return [];
      }
      const pictures = item.attachments
        .filter(
          (attachment) =>
            attachment.kind === "image" &&
            !pendingIds.has(attachment.id) &&
            !attachment.unavailable &&
            attachment.fullUri,
        )
        .map(toPicture);
      return pictures.length > 0
        ? [{ label: `Sent images ${messageIndex + 1}`, pictures }]
        : [];
    });

    return [
      ...(pendingPictures.length > 0 ? [{ label: "Pending", pictures: pendingPictures }] : []),
      ...historySections,
    ];
  }

  private async syncImagePreviewPanel(sessionId: string): Promise<void> {
    if (this.imagePreviewSessionId !== sessionId) return;
    const { ImagePreviewPanel } = await import(
      "../imagePreview/ImagePreviewPanel.js"
    );
    ImagePreviewPanel.getCurrent()?.updateSections(
      this.imagePreviewSections(sessionId),
    );
  }

  /** Open or reuse the image preview panel for draft and transcript images. */
  private async openImagePreviewPanel(
    sessionId: string,
    attachmentId: string,
  ): Promise<void> {
    try {
      const sections = this.imagePreviewSections(sessionId);
      if (
        !sections.some((section) =>
          section.pictures.some((picture) => picture.id === attachmentId),
        )
      ) {
        return;
      }
      const { ImagePreviewPanel } = await import(
        "../imagePreview/ImagePreviewPanel.js"
      );
      this.imagePreviewSessionId = sessionId;
      ImagePreviewPanel.getInstance(this.deps.extensionUri, this.attachmentRoot).reveal(
        sections,
        attachmentId,
      );
    } catch (error) {
      console.warn("Tomcat failed to open the image preview", error);
    }
  }

  /**
   * Read a picked file into an upload, validating before anything crosses the wire.
   *
   * Files chosen from disk skip the webview, so there is no thumbnail and no SVG
   * rasterisation for them. The image still displays (Chromium fetches the full one) and
   * the thumbnail arrives on a later pass once the webview has seen it.
   */
  private async readAttachmentUpload(
    uri: vscode.Uri,
    mimeType = guessMimeType(uri.fsPath || uri.path),
  ): Promise<AttachmentUpload> {
    const bytes = Buffer.from(await vscode.workspace.fs.readFile(uri));
    const basename = path.basename(uri.fsPath || uri.path);
    const dataBase64 = bytes.toString("base64");
    const validation = validateAttachmentCandidate({
      dataBase64,
      filename: basename,
      mimeType,
    });
    if (!validation.ok) {
      throw new Error(`${basename}: ${validation.error}`);
    }
    return {
      dataBase64,
      filename: validation.filename,
      kind: validation.kind,
      mimeType: validation.mimeType,
      sourcePath: uri.fsPath || uri.path,
    };
  }

  /**
   * Every directory the chat webview is allowed to read from.
   *
   * The attachment directories sit in the user's tomcat data directory, nowhere near
   * `extensionUri`, so they have to be granted explicitly. Omit them and every image
   * silently fails to load — a failure left deliberately visible, because the
   * alternative (falling back to `data:` URIs) would quietly restore the half-gigabyte
   * memory profile this whole design exists to remove.
   *
   * Built here rather than inline at the two call sites, so the initial grant and the
   * regrant after the attachment root is discovered cannot drift apart.
   */
  private resourceRoots(): vscode.Uri[] {
    return [
      vscode.Uri.joinPath(this.deps.extensionUri, "gui", "dist"),
      vscode.Uri.joinPath(this.deps.extensionUri, "media"),
      ...this.workspaceMediaRootUris(),
      vscode.Uri.file(os.tmpdir()),
      ...attachmentResourceRoots(this.attachmentRoot),
    ];
  }

  private webviewOptions(): vscode.WebviewOptions {
    return {
      enableScripts: true,
      localResourceRoots: this.resourceRoots(),
    };
  }

  private workspaceMediaRootUris(): vscode.Uri[] {
    return (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri);
  }

  private mediaRootsForWebview(webview: vscode.Webview): WebviewMediaRoot[] {
    const deduped = new Map<string, WebviewMediaRoot>();
    for (const root of [...this.workspaceMediaRootUris(), vscode.Uri.file(os.tmpdir())]) {
      const fsPath = root.fsPath;
      if (!fsPath || deduped.has(fsPath)) {
        continue;
      }
      deduped.set(fsPath, {
        fsPath,
        webviewBase: webview.asWebviewUri(root).toString(),
      });
    }
    return [...deduped.values()];
  }

  private reloadWebviewResourceRoots(): void {
    if (!this.view) {
      return;
    }
    // Changing local resource roots rebuilds the document. Mirror first-mount state so
    // nothing keeps talking to a document VS Code is about to throw away.
    this.isReady = false;
    this.stateStore.setReady(false);
    this.view.webview.options = this.webviewOptions();
  }

  private handleWorkspaceFolderChange(): void {
    this.reloadWebviewResourceRoots();
  }

  /**
   * What the host granted and what it believes exists on disk.
   *
   * Images reaching a webview depend on three things lining up — the backend's reported
   * root, the granted resource roots, and the files themselves — and when they do not,
   * the only symptom is a broken image with no explanation anywhere. This turns that into
   * something the acceptance report can state outright.
   */
  getAttachmentDiagnostics(): {
    attachmentRoot: string | null;
    attachmentRootResolved: boolean;
    blobsDirExists: boolean;
    resourceRoots: string[];
    thumbsDirExists: boolean;
  } {
    const exists = (...parts: string[]): boolean =>
      this.attachmentRoot !== null &&
      fs.existsSync(path.join(this.attachmentRoot, ...parts));
    return {
      attachmentRoot: this.attachmentRoot,
      attachmentRootResolved: this.attachmentRootResolved,
      blobsDirExists: exists("blobs"),
      resourceRoots: this.resourceRoots().map((uri) => uri.fsPath),
      thumbsDirExists: exists("thumbs"),
    };
  }

  private renderHtml(webview: vscode.Webview): string {
    const distRoot = path.join(this.deps.extensionUri.fsPath, "gui", "dist");
    const assets = resolveWebviewEntryAssets(distRoot, "index.html", "index.js");
    if (assets.scripts.length === 0) {
      return this.renderFallbackHtml(
        "Tomcat webview assets are missing. Run `npm run build` in `tomcat-vscode-ext` to generate `gui/dist`.",
      );
    }

    const nonce = getNonce();
    const styleTags = assets.stylesheets
      .map(
        (file) =>
          `<link rel="stylesheet" href="${webview.asWebviewUri(vscode.Uri.file(file)).toString()}" />`,
      )
      .join("\n    ");
    const scriptTags = assets.scripts
      .map(
        (file) =>
          `<script nonce="${nonce}" type="module" src="${webview.asWebviewUri(vscode.Uri.file(file)).toString()}"></script>`,
      )
      .join("\n    ");

    return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; img-src ${webview.cspSource} blob:; connect-src ${webview.cspSource}; font-src ${webview.cspSource}; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}' 'strict-dynamic';"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    ${styleTags}
    <title>Tomcat</title>
  </head>
  <body>
    <div id="root"></div>
    ${scriptTags}
  </body>
</html>`;
  }

  private renderFallbackHtml(message: string): string {
    return `<!DOCTYPE html>
<html lang="en">
  <body>
    <pre>${message}</pre>
  </body>
</html>`;
  }

  private async selectSession(sessionId: string): Promise<void> {
    await this.ensureInitialized();
    this.stateStore.setActiveSession(sessionId);
    await this.sessionPool.switchTo(sessionId);
    await this.refreshSessionState(sessionId, { trustBusy: true });
    await this.refreshSessionHistory(sessionId);
    await this.refreshCheckpoints(sessionId);
    await this.refreshSessions();
    this.stateStore.setActiveSession(sessionId);

    // Hydrate composer draft from Rust backend (pending images/text/segments)
    await this.hydrateDraft(sessionId);

    await this.postState();
  }

  private async switchSessionView(sessionId: string): Promise<void> {
    await this.ensureInitialized();
    await this.sessionPool.switchTo(sessionId);
    await this.refreshSessionState(sessionId, { trustBusy: true });
    await this.refreshSessionHistory(sessionId);
    await this.refreshCheckpoints(sessionId);
    await this.refreshSessions();
    this.stateStore.setActiveSession(sessionId);
    // Hydrate draft for this session
    await this.hydrateDraft(sessionId);
    await this.postState();
  }
}
