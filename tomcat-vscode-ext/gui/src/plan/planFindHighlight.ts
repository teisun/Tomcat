import type { PlanFindMatch } from "./planFindEngine";

const ALL_MATCHES_HIGHLIGHT_NAME = "tc-plan-find";
const ACTIVE_MATCH_HIGHLIGHT_NAME = "tc-plan-find-active";

type HighlightRegistryLike = {
  delete(name: string): boolean;
  set(name: string, highlight: unknown): unknown;
};

type HighlightConstructor = new (...ranges: Range[]) => unknown;

function getHighlightRegistry(): HighlightRegistryLike | null {
  const css = globalThis.CSS as (typeof CSS & {
    highlights?: HighlightRegistryLike;
  }) | undefined;
  return css?.highlights ?? null;
}

function getHighlightConstructor(): HighlightConstructor | null {
  return (
    (globalThis as typeof globalThis & { Highlight?: HighlightConstructor })
      .Highlight ?? null
  );
}

function rangeFor(match: PlanFindMatch): Range {
  const range = match.node.ownerDocument.createRange();
  range.setStart(match.node, match.start);
  range.setEnd(match.node, match.end);
  return range;
}

/** Remove the two named CSS Custom Highlights owned by Plan Preview Find. */
export function clearPlanFindHighlights(): void {
  const registry = getHighlightRegistry();
  registry?.delete(ALL_MATCHES_HIGHLIGHT_NAME);
  registry?.delete(ACTIVE_MATCH_HIGHLIGHT_NAME);
}

/**
 * Draw all matches plus the active match without changing the plan DOM.
 * Browsers without CSS Custom Highlight support safely retain the search UI and
 * match count, but do not receive visual highlights.
 */
export function paintPlanFindHighlights(
  matches: readonly PlanFindMatch[],
  activeIndex: number,
): void {
  const registry = getHighlightRegistry();
  const HighlightConstructor = getHighlightConstructor();
  if (!registry || !HighlightConstructor) {
    return;
  }

  clearPlanFindHighlights();
  if (matches.length === 0) {
    return;
  }

  registry.set(
    ALL_MATCHES_HIGHLIGHT_NAME,
    new HighlightConstructor(...matches.map(rangeFor)),
  );
  const activeMatch = matches[activeIndex];
  if (activeMatch) {
    registry.set(
      ACTIVE_MATCH_HIGHLIGHT_NAME,
      new HighlightConstructor(rangeFor(activeMatch)),
    );
  }
}

/** Scroll the active match to the vertical centre of the plan content viewport. */
export function scrollPlanFindMatchIntoView(
  match: PlanFindMatch,
  container: HTMLElement,
): void {
  const range = rangeFor(match);
  // jsdom (and older webview Chromium builds) may expose Range without layout
  // geometry. Finding and count still work; only the optional auto-scroll skips.
  if (typeof range.getBoundingClientRect !== "function") {
    return;
  }
  const matchRect = range.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();
  const viewportHeight = container.clientHeight || containerRect.height;
  if (viewportHeight <= 0) {
    return;
  }

  const absoluteTop =
    matchRect.top - containerRect.top + container.scrollTop;
  const nextScrollTop = Math.max(
    0,
    absoluteTop - (viewportHeight - matchRect.height) / 2,
  );
  container.scrollTop = nextScrollTop;
}
