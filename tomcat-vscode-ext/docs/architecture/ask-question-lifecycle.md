# Ask Question lifecycle

`ask_question` is a durable tool round with a live, host-owned answer channel. It has **no wall-clock timeout** and no fallback result.

```text
assistant message (tool call, toolCallId)
                  |
                  v
Rust runtime -> serve control_request (sessionId, requestId)
                  |
                  v
Extension host pending map (sessionId + requestId + child generation)
                  |
                  v
Webview fixed “Needs your answer” panel
                  |
       answer / skip / interrupt / disconnect
                  v
serve control_response or terminal control_cancel
                  |
                  v
tool result message (same toolCallId) -> history AnswerCard
```

## Identities

| Identity | Lifetime | Purpose |
| --- | --- | --- |
| `toolCallId` | durable transcript | Joins the assistant tool declaration and tool-result companion. |
| `sessionId` + `requestId` | one live question | Routes answers and prevents a response from crossing sessions. |
| child generation | one `tomcat serve` process | Stops a deferred handler from writing to a replacement child after restart/exit. |

## Terminal outcomes

| Outcome | Meaning |
| --- | --- |
| `answered` | User submitted the request; individual questions may still carry `skipped: true`. |
| `skipped` | User skipped the entire request. |
| `interrupted` | The current turn was explicitly interrupted. |
| `host_disconnected` | The CLI input channel, serve child, pipe, Extension Host, or window closed irrecoverably. |
| `cancelled_unknown` | Read-only compatibility label for an old `cancelled: true` payload without a typed outcome. |

A webview DOM reload is not terminal. The Provider owns pending questions and reprojects them into the new DOM. A serve child/window/Extension Host restart is terminal for the old live channel and becomes `host_disconnected`.

## UI state

- Unresolved questions render outside the scrolling transcript, immediately above the composer controls.
- The answer draft is keyed by `sessionId + requestId`, persisted with VS Code webview state, and guarded against duplicate submission.
- Background sessions expose a pending count badge. Switching to one focuses the first answer control and announces the question once through `aria-live`.
- History cards always show every question and option, including answered, question-level skip, whole-request skip, interrupt, disconnect, and legacy unknown states.

## Durable history

The assistant tool declaration and its tool result are the source of truth. `get_messages` expands a page that begins with a tool result to include its assistant companion, joined by `toolCallId`. Old custom ask-question entries are rendered only when the standard companion is missing.

A dangling assistant `ask_question` call discovered during hydration is repaired with a structured `host_disconnected` tool result.

## Removed timeout configuration

These old settings are detected only to emit a migration warning and are otherwise ignored:

- `ask_question.timeout_ms`
- `TOMCAT_ASK_QUESTION_TIMEOUT_MS`
- `TOMCAT__ASK_QUESTION__TIMEOUT_MS`

They never create a deadline. There is no replacement timeout option.
