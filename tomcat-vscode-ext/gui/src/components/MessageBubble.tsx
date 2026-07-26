import { memo, useEffect, useState } from "react";

import { AttachmentStrip } from "./AttachmentStrip";
import { ReferenceChip } from "./ReferenceChip";
import { ChatMarkdown } from "./markdown/ChatMarkdown";
import type {
  WebviewMessageBlock,
  WebviewMediaRoot,
  WebviewMessageSegment,
  WebviewPendingAttachment,
} from "../types";

const MESSAGE_LABELS: Record<WebviewMessageBlock["kind"], string> = {
  assistant: "Tomcat",
  error: "Error",
  notice: "Notice",
  user: "You",
  warn: "Warn",
};
const NOOP_OPEN_FILE = () => undefined;

type MessageBubbleProps = {
  item: WebviewMessageBlock;
  mediaRoots?: WebviewMediaRoot[];
  onOpenFile?: (path: string, line?: number) => void;
  onOpenImagePreview?: (imageId: string) => void;
  onRetry?: (messageId: string) => void;
  onZoomImage?: (image: { alt: string; src: string }) => void;
};

function MessageBubbleComponent({
  item,
  mediaRoots,
  onOpenFile,
  onOpenImagePreview,
  onRetry,
  onZoomImage,
}: MessageBubbleProps) {
  const [detailsExpanded, setDetailsExpanded] = useState(false);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const showHeader = item.kind !== "user" && item.kind !== "assistant";
  const isFailedUserMessage = item.kind === "user" && item.deliveryState === "failed";
  const isPendingUserMessage = item.kind === "user" && item.deliveryState === "pending";
  const showRetry = isFailedUserMessage && item.retryable === true && typeof onRetry === "function";
  const rawErrorDetail =
    item.kind === "error" && typeof item.detailText === "string" && item.detailText.trim().length > 0
      ? item.detailText
      : null;
  const canToggleRawError = rawErrorDetail !== null && rawErrorDetail.trim() !== item.text.trim();
  const segments: WebviewMessageSegment[] =
    item.segments?.length ? item.segments : [{ text: item.text, type: "text" }];
  const historyAttachments: WebviewPendingAttachment[] = (item.attachments ?? []).map(
    (attachment) => ({
      ...attachment,
      label: attachment.filename,
      path: attachment.path ?? null,
    }),
  );

  useEffect(() => {
    setDetailsExpanded(false);
    setCopyState("idle");
  }, [item.id, item.detailText]);

  async function copyRawError(): Promise<void> {
    if (!rawErrorDetail || typeof navigator?.clipboard?.writeText !== "function") {
      setCopyState("failed");
      return;
    }
    try {
      await navigator.clipboard.writeText(rawErrorDetail);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
  }

  return (
    <article
      className={`tc-message tc-message--${item.kind}${isFailedUserMessage ? " tc-message--user-failed" : ""}${isPendingUserMessage ? " tc-message--user-pending" : ""}`}
      data-delivery-state={item.deliveryState}
      data-kind={item.kind}
      data-message-id={item.id}
      data-message-kind={item.kind}
      data-testid="message-block"
    >
      {showHeader ? (
        <div className="tc-message__header">
          <strong>{MESSAGE_LABELS[item.kind]}</strong>
          <span>{item.label ?? item.kind}</span>
        </div>
      ) : null}
      <div className="message-text rendered-markdown" data-testid="message-text">
        {item.kind === "assistant" ? (
          <ChatMarkdown
            markdown={item.text}
            mediaRoots={mediaRoots}
            onOpenFile={onOpenFile ?? NOOP_OPEN_FILE}
            onZoomImage={onZoomImage}
          />
        ) : (
          segments.map((segment, index) =>
            segment.type === "text" ? (
              <span className="tc-message__text-segment" key={`${item.id}-text-${index}`}>
                {segment.text}
              </span>
            ) : (
              <ReferenceChip
                key={`${item.id}-reference-${index}`}
                reference={segment}
                testId="history-reference-chip"
              />
            ),
          )
        )}
      </div>
      <AttachmentStrip
        attachments={historyAttachments}
        onOpen={(attachment) => {
          if (attachment.kind === "image") {
            onOpenImagePreview?.(attachment.id);
            return;
          }
          if (attachment.path) {
            onOpenFile?.(attachment.path);
          }
        }}
        readonly
      />
      {canToggleRawError ? (
        <div className="tc-message__detail-actions" data-testid="error-detail-actions">
          <button
            aria-expanded={detailsExpanded}
            className="tc-message__detail-button"
            data-testid="toggle-error-detail"
            onClick={() => setDetailsExpanded((value) => !value)}
            type="button"
          >
            <span
              aria-hidden="true"
              className={`codicon ${detailsExpanded ? "codicon-chevron-down" : "codicon-chevron-right"}`}
            />
            <span>{detailsExpanded ? "Hide original error" : "Show original error"}</span>
          </button>
          <button
            className="tc-message__detail-button"
            data-testid="copy-error-detail"
            onClick={() => {
              void copyRawError();
            }}
            type="button"
          >
            <span aria-hidden="true" className="codicon codicon-copy" />
            <span>
              {copyState === "copied"
                ? "Copied"
                : copyState === "failed"
                  ? "Copy failed"
                  : "Copy original"}
            </span>
          </button>
        </div>
      ) : null}
      {detailsExpanded && rawErrorDetail ? (
        <pre className="tc-message__detail" data-testid="error-detail-text">
          {rawErrorDetail}
        </pre>
      ) : null}
      {isPendingUserMessage ? (
        <div className="tc-message__status" data-testid="user-message-status">
          <span>Sending...</span>
        </div>
      ) : null}
      {isFailedUserMessage ? (
        <div className="tc-message__status" data-testid="user-message-status">
          <span>{item.deliveryError ?? "Send failed."}</span>
          {showRetry ? (
            <button
              className="tc-message__retry"
              data-testid="retry-user-message"
              onClick={() => onRetry?.(item.id)}
              type="button"
            >
              Retry
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function areMessageBubblePropsEqual(prev: MessageBubbleProps, next: MessageBubbleProps): boolean {
  return (
    prev.mediaRoots === next.mediaRoots &&
    prev.item === next.item &&
    prev.onOpenFile === next.onOpenFile &&
    prev.onOpenImagePreview === next.onOpenImagePreview &&
    prev.onRetry === next.onRetry &&
    prev.onZoomImage === next.onZoomImage
  );
}

export const MessageBubble = memo(MessageBubbleComponent, areMessageBubblePropsEqual);
MessageBubble.displayName = "MessageBubble";
