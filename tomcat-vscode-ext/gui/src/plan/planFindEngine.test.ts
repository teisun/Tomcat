import { describe, expect, it } from "vitest";

import { collectPlanFindMatches } from "./planFindEngine";

function createRoot(html: string): HTMLElement {
  const root = document.createElement("div");
  root.innerHTML = html;
  document.body.append(root);
  return root;
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("collectPlanFindMatches", () => {
  it("returns no matches for an empty query", () => {
    const root = createRoot("<p>Find me</p>");

    expect(collectPlanFindMatches(root, "")).toEqual([]);
  });

  it("finds every non-overlapping substring in document order, ignoring case", () => {
    const root = createRoot("<p>Update update UPDATES update</p>");

    const matches = collectPlanFindMatches(root, "update");

    expect(matches).toHaveLength(4);
    expect(matches.map((match) => [match.start, match.end])).toEqual([
      [0, 6],
      [7, 13],
      [14, 20],
      [22, 28],
    ]);
    expect(matches.map((match) => match.node.data)).toEqual([
      "Update update UPDATES update",
      "Update update UPDATES update",
      "Update update UPDATES update",
      "Update update UPDATES update",
    ]);
  });

  it("finds matches in separate rendered text nodes in document order", () => {
    const root = createRoot(
      '<div data-testid="plan-markdown-body"><p>First PLAN match</p></div><ul data-testid="plan-todo-list"><li>Second plan match</li></ul>',
    );

    const matches = collectPlanFindMatches(root, "plan");

    expect(matches).toHaveLength(2);
    expect(matches.map((match) => match.node.parentElement?.textContent)).toEqual([
      "First PLAN match",
      "Second plan match",
    ]);
    expect(matches.map((match) => match.start)).toEqual([6, 7]);
  });

  it("skips text that is not visible plan copy", () => {
    const root = createRoot(
      '<p>Visible plan text</p><span class="tc-visually-hidden">plan</span><span hidden>plan</span><span aria-hidden="true">plan</span><style>.x { display: none; }</style><script>plan</script><svg><text>plan</text></svg>',
    );

    const matches = collectPlanFindMatches(root, "plan");

    expect(matches).toHaveLength(1);
    expect(matches[0].node.parentElement?.textContent).toBe("Visible plan text");
  });

  it("does not join text across inline formatting boundaries", () => {
    const root = createRoot("<p><strong>up</strong>date</p>");

    expect(collectPlanFindMatches(root, "update")).toEqual([]);
  });
});
