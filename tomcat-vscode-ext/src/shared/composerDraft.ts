/**
 * Composer draft storage — what the user has typed but not yet sent.
 *
 * ## Why this lives in the extension and not in the backend
 *
 * A draft is *editing state*, and editing state belongs next to the editor. Every
 * agent we surveyed (Cline, Continue, Codex, opencode, and VS Code's own SCM input
 * box) keeps it that way: the frontend owns the unsent buffer, the backend never hears
 * about a keystroke. Putting it behind an RPC boundary means every keystroke turns
 * into a round trip, and a round trip that carries the *whole* draft — attachments
 * included — is write amplification measured in megabytes per character.
 *
 * ## What is stored here, and what deliberately is not
 *
 * ```
 *   this file (extension storage)          Rust blob store
 *   ─────────────────────────────          ───────────────
 *   text the user typed                    the image bytes
 *   @-mention segments                     the thumbnail bytes
 *   attachment *references* (sha256)       the provider rendering
 *   ~200 bytes per draft                   ~4MB per image
 * ```
 *
 * The split is what makes keystrokes cheap: text changes rewrite a couple of hundred
 * bytes of JSON; image bytes were handed to the backend once, at paste time, and never
 * move again.
 *
 * ## Durability
 *
 * Writes are debounced and atomic (temp file + rename), so a crash mid-write leaves
 * either the previous draft or the new one, never a half-written file. A file that
 * fails to parse is quarantined rather than deleted — the user gets an empty composer
 * they can immediately type into, and the bad file is still on disk to look at.
 */
import * as vscode from "vscode";

import type { WebviewMessageSegment } from "../ui/webview/protocol";

/** How long to wait after the last keystroke before touching the disk. */
export const DRAFT_WRITE_DEBOUNCE_MS = 400;

/** Bumped only for changes that older builds cannot read. */
const DRAFT_SCHEMA_VERSION = 2;

/**
 * An attachment as the draft knows it: identity and metadata, never bytes.
 *
 * `blobSha` is the handle the backend recognises. The derived artefacts are optional
 * because each can legitimately be missing — thumbnail generation can fail, and most
 * formats need no conversion for the provider.
 */
export interface DraftAttachmentRef {
  blobSha: string;
  /** Original byte count, for display only. */
  bytes: number;
  filename: string;
  /**
   * Whether a downsampled version exists.
   *
   * Not a hash: the thumbnail is stored under the hash of the image it came from, so
   * `thumbs/<blobSha>` is the only address anyone ever needs.
   */
  hasThumb?: boolean;
  id: string;
  kind: "file" | "image";
  mimeType: string;
  providerSha?: string | null;
  /** SVG source handed to the model when rasterisation was not possible. */
  providerText?: string | null;
}

export interface ComposerDraft {
  attachments: DraftAttachmentRef[];
  segments: WebviewMessageSegment[];
  text: string;
}

interface PersistedDraft extends ComposerDraft {
  schemaVersion: number;
  updatedAt: number;
}

export const EMPTY_DRAFT: ComposerDraft = Object.freeze({
  attachments: Object.freeze([]) as unknown as DraftAttachmentRef[],
  segments: Object.freeze([]) as unknown as WebviewMessageSegment[],
  text: "",
});

export function isDraftEmpty(draft: ComposerDraft): boolean {
  return (
    draft.text.length === 0 &&
    draft.segments.length === 0 &&
    draft.attachments.length === 0
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Parse one persisted attachment reference, or reject it.
 *
 * Rejecting a single bad entry rather than the whole draft matters: one unreadable
 * attachment should not cost the user the paragraph they spent five minutes writing.
 */
function parseAttachment(value: unknown): DraftAttachmentRef | null {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.blobSha !== "string" ||
    !/^[0-9a-f]{64}$/.test(value.blobSha) ||
    (value.kind !== "image" && value.kind !== "file") ||
    typeof value.filename !== "string" ||
    typeof value.mimeType !== "string"
  ) {
    return null;
  }
  return {
    blobSha: value.blobSha,
    bytes: typeof value.bytes === "number" ? value.bytes : 0,
    filename: value.filename,
    hasThumb: value.hasThumb === true,
    id: value.id,
    kind: value.kind,
    mimeType: value.mimeType,
    providerSha: typeof value.providerSha === "string" ? value.providerSha : null,
    providerText: typeof value.providerText === "string" ? value.providerText : null,
  };
}

function parseSegments(value: unknown): WebviewMessageSegment[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry): WebviewMessageSegment[] => {
    if (!isRecord(entry)) return [];
    if (entry.type === "text" && typeof entry.text === "string") {
      return [{ text: entry.text, type: "text" }];
    }
    if (
      entry.type === "reference" &&
      (entry.kind === "file" || entry.kind === "selection") &&
      typeof entry.path === "string" &&
      typeof entry.label === "string"
    ) {
      return [
        {
          kind: entry.kind,
          label: entry.label,
          lineEnd: typeof entry.lineEnd === "number" ? entry.lineEnd : null,
          lineStart: typeof entry.lineStart === "number" ? entry.lineStart : null,
          path: entry.path,
          text: typeof entry.text === "string" ? entry.text : null,
          type: "reference",
        },
      ];
    }
    return [];
  });
}

/** Thrown by nothing — parse failures return null so callers cannot forget to handle them. */
function parseDraft(raw: string): ComposerDraft | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isRecord(parsed) || typeof parsed.schemaVersion !== "number") {
    return null;
  }
  if (parsed.schemaVersion > DRAFT_SCHEMA_VERSION) {
    // Written by a newer build. Reading it would silently drop fields we do not know
    // about, which is worse than starting empty.
    return null;
  }
  return {
    attachments: Array.isArray(parsed.attachments)
      ? parsed.attachments.flatMap((entry) => {
          const attachment = parseAttachment(entry);
          return attachment ? [attachment] : [];
        })
      : [],
    segments: parseSegments(parsed.segments),
    text: typeof parsed.text === "string" ? parsed.text : "",
  };
}

/**
 * Per-session draft storage backed by the extension's own storage directory.
 *
 * One file per session, so two windows editing two sessions never contend, and a
 * corrupt file costs exactly one session's draft.
 */
export class ComposerDraftStore {
  /** In-memory truth. The disk is a backup of this, not the other way round. */
  private readonly drafts = new Map<string, ComposerDraft>();

  private readonly pendingWrites = new Map<string, ReturnType<typeof setTimeout>>();

  /** In-flight disk writes, so `flush` can await them and tests can be deterministic. */
  private readonly inFlight = new Map<string, Promise<void>>();

  private rootReady: Promise<void> | null = null;

  constructor(
    private readonly root: vscode.Uri,
    private readonly debounceMs: number = DRAFT_WRITE_DEBOUNCE_MS,
  ) {}

  /**
   * Pick a storage root, preferring the workspace-scoped one.
   *
   * `storageUri` is undefined when no folder is open. Falling back to
   * `globalStorageUri` keeps drafts working in an empty window instead of silently
   * dropping every keystroke.
   */
  static forContext(
    context: Pick<vscode.ExtensionContext, "globalStorageUri" | "storageUri">,
    debounceMs?: number,
  ): ComposerDraftStore {
    const base = context.storageUri ?? context.globalStorageUri;
    return new ComposerDraftStore(vscode.Uri.joinPath(base, "composer-drafts"), debounceMs);
  }

  /**
   * Reject a session id that could name a file outside the draft directory.
   *
   * Asserted at every entry point rather than down in the write, because the write is
   * detached from the caller: a throw down there is an unhandled rejection nobody sees,
   * which is precisely how a path-traversal bug survives review.
   */
  private assertSessionId(sessionId: string): void {
    if (!/^[A-Za-z0-9_-]{1,128}$/.test(sessionId)) {
      throw new Error(`refusing to build a draft path from session id ${sessionId}`);
    }
  }

  private draftUri(sessionId: string): vscode.Uri {
    this.assertSessionId(sessionId);
    return vscode.Uri.joinPath(this.root, `${sessionId}.json`);
  }

  private async ensureRoot(): Promise<void> {
    this.rootReady ??= (async () => {
      await vscode.workspace.fs.createDirectory(this.root);
    })();
    await this.rootReady;
  }

  /**
   * Read a session's draft, falling back to empty for anything unreadable.
   *
   * `isKnownSession` lets the caller drop drafts whose session no longer exists —
   * otherwise deleting a session leaves its draft file behind forever.
   */
  async hydrate(
    sessionId: string,
    isKnownSession?: (sessionId: string) => boolean | Promise<boolean>,
  ): Promise<ComposerDraft> {
    this.assertSessionId(sessionId);
    const cached = this.drafts.get(sessionId);
    if (cached) return cached;

    if (isKnownSession && !(await isKnownSession(sessionId))) {
      await this.discard(sessionId);
      return EMPTY_DRAFT;
    }

    const uri = this.draftUri(sessionId);
    let raw: string;
    try {
      raw = new TextDecoder().decode(await vscode.workspace.fs.readFile(uri));
    } catch {
      // No draft on disk is the overwhelmingly common case, not an error.
      return EMPTY_DRAFT;
    }

    const draft = parseDraft(raw);
    if (!draft) {
      await this.quarantine(uri);
      return EMPTY_DRAFT;
    }
    this.drafts.set(sessionId, draft);
    return draft;
  }

  /** The current draft without touching the disk. */
  peek(sessionId: string): ComposerDraft {
    return this.drafts.get(sessionId) ?? EMPTY_DRAFT;
  }

  /**
   * Record a change and schedule a debounced write.
   *
   * Returns immediately: the caller is on the keystroke path and must not wait for IO.
   */
  update(sessionId: string, mutate: (current: ComposerDraft) => ComposerDraft): ComposerDraft {
    this.assertSessionId(sessionId);
    const next = mutate(this.peek(sessionId));
    this.drafts.set(sessionId, next);
    this.scheduleWrite(sessionId);
    return next;
  }

  private scheduleWrite(sessionId: string): void {
    const existing = this.pendingWrites.get(sessionId);
    if (existing) {
      clearTimeout(existing);
    }
    const timer = setTimeout(() => {
      this.pendingWrites.delete(sessionId);
      void this.writeNow(sessionId);
    }, this.debounceMs);
    // Do not hold the extension host open just to save a draft.
    (timer as unknown as { unref?(): void }).unref?.();
    this.pendingWrites.set(sessionId, timer);
  }

  private async writeNow(sessionId: string): Promise<void> {
    const previous = this.inFlight.get(sessionId) ?? Promise.resolve();
    const write = previous.catch(() => undefined).then(async () => {
      const draft = this.drafts.get(sessionId);
      if (!draft || isDraftEmpty(draft)) {
        // An empty draft is the absence of a draft. Leaving a `{"text":""}` file behind
        // would resurrect on the next hydrate and make "cleared" look like "unsaved".
        await this.deleteFile(this.draftUri(sessionId));
        return;
      }
      const payload: PersistedDraft = {
        ...draft,
        schemaVersion: DRAFT_SCHEMA_VERSION,
        updatedAt: Date.now(),
      };
      await this.ensureRoot();
      const target = this.draftUri(sessionId);
      const temp = target.with({ path: `${target.path}.tmp` });
      const bytes = new TextEncoder().encode(JSON.stringify(payload));
      try {
        // Temp + rename: a crash between the two leaves the previous draft intact.
        await vscode.workspace.fs.writeFile(temp, bytes);
        await vscode.workspace.fs.rename(temp, target, { overwrite: true });
      } catch (error) {
        await this.deleteFile(temp);
        console.warn(`Tomcat could not save the draft for ${sessionId}`, error);
      }
    });
    this.inFlight.set(sessionId, write);
    try {
      await write;
    } finally {
      if (this.inFlight.get(sessionId) === write) {
        this.inFlight.delete(sessionId);
      }
    }
  }

  /**
   * Drop a draft from memory and disk.
   *
   * Called once the backend has acknowledged the send — not before. Clearing on
   * optimism means a failed send loses the message the user was trying to send.
   */
  async discard(sessionId: string): Promise<void> {
    this.assertSessionId(sessionId);
    const pending = this.pendingWrites.get(sessionId);
    if (pending) {
      clearTimeout(pending);
      this.pendingWrites.delete(sessionId);
    }
    this.drafts.delete(sessionId);
    await this.inFlight.get(sessionId)?.catch(() => undefined);
    await this.deleteFile(this.draftUri(sessionId));
  }

  /** Await every scheduled write. Used before send and on deactivate. */
  async flush(sessionId?: string): Promise<void> {
    const ids = sessionId ? [sessionId] : [...this.pendingWrites.keys()];
    for (const id of ids) {
      const pending = this.pendingWrites.get(id);
      if (pending) {
        clearTimeout(pending);
        this.pendingWrites.delete(id);
        await this.writeNow(id);
      }
    }
    await Promise.all(
      (sessionId ? [this.inFlight.get(sessionId)] : [...this.inFlight.values()]).map((task) =>
        task?.catch(() => undefined),
      ),
    );
  }

  /** Session ids that currently have a draft in memory. */
  trackedSessions(): string[] {
    return [...this.drafts.keys()];
  }

  private async deleteFile(uri: vscode.Uri): Promise<void> {
    try {
      await vscode.workspace.fs.delete(uri, { useTrash: false });
    } catch {
      // Already gone is the desired end state.
    }
  }

  /**
   * Move an unreadable draft aside instead of deleting it.
   *
   * The user gets a working composer immediately; the bad file stays on disk so the
   * failure can actually be diagnosed rather than guessed at from a log line.
   */
  private async quarantine(uri: vscode.Uri): Promise<void> {
    const target = uri.with({ path: `${uri.path}.corrupt` });
    try {
      await vscode.workspace.fs.rename(uri, target, { overwrite: true });
      console.warn(`Tomcat quarantined an unreadable draft at ${target.fsPath}`);
    } catch {
      await this.deleteFile(uri);
    }
  }
}
