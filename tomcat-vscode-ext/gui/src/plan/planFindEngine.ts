export interface PlanFindMatch {
  /** Text node containing the complete non-overlapping match. */
  node: Text;
  /** Zero-based inclusive offset within `node.data`. */
  start: number;
  /** Zero-based exclusive offset within `node.data`. */
  end: number;
}

/**
 * Tags and element states that do not represent searchable plan copy. In
 * particular, Mermaid renders an SVG beside its source markdown, so searching
 * it would duplicate visible text and produce inaccessible results.
 */
const NON_SEARCHABLE_ANCESTOR_SELECTOR = [
  "script",
  "style",
  "template",
  "svg",
  "[hidden]",
  '[aria-hidden="true"]',
  ".tc-visually-hidden",
].join(", ");

function isSearchableTextNode(root: HTMLElement, node: Text): boolean {
  let element = node.parentElement;
  if (!element || !root.contains(element)) {
    return false;
  }

  const view = root.ownerDocument.defaultView;
  while (element && root.contains(element)) {
    if (element.matches(NON_SEARCHABLE_ANCESTOR_SELECTOR)) {
      return false;
    }
    const style = view?.getComputedStyle(element);
    if (
      style?.display === "none" ||
      style?.visibility === "hidden" ||
      style?.visibility === "collapse"
    ) {
      return false;
    }
    if (element === root) {
      break;
    }
    element = element.parentElement;
  }
  return true;
}

/**
 * Finds all case-insensitive, non-overlapping substring matches in rendered
 * plan copy. A match is deliberately confined to one text node: searching
 * across inline formatting would require reconstructing DOM ranges and risks
 * highlighting unrelated markup. Results follow document order.
 */
export function collectPlanFindMatches(
  root: HTMLElement,
  query: string,
): PlanFindMatch[] {
  if (query.length === 0) {
    return [];
  }

  const needle = query.toLowerCase();
  const matches: PlanFindMatch[] = [];
  const nodeFilter = root.ownerDocument.defaultView?.NodeFilter ?? NodeFilter;
  const walker = root.ownerDocument.createTreeWalker(
    root,
    nodeFilter.SHOW_TEXT,
    {
      acceptNode(node) {
        return isSearchableTextNode(root, node as Text)
          ? nodeFilter.FILTER_ACCEPT
          : nodeFilter.FILTER_REJECT;
      },
    },
  );

  for (let current = walker.nextNode(); current; current = walker.nextNode()) {
    const node = current as Text;
    const haystack = node.data.toLowerCase();
    let start = haystack.indexOf(needle);
    while (start !== -1) {
      matches.push({ end: start + query.length, node, start });
      start = haystack.indexOf(needle, start + query.length);
    }
  }

  return matches;
}
