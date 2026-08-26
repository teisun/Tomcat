import { afterEach, describe, expect, it, vi } from "vitest";

import type { PlanFindMatch } from "./planFindEngine";
import {
  clearPlanFindHighlights,
  paintPlanFindHighlights,
  scrollPlanFindMatchIntoView,
} from "./planFindHighlight";

function makeMatch(text = "A plan match", start = 2, end = 6): PlanFindMatch {
  return { end, node: document.createTextNode(text), start };
}

const originalCss = globalThis.CSS;
const originalHighlight = (globalThis as typeof globalThis & {
  Highlight?: unknown;
}).Highlight;

afterEach(() => {
  Object.defineProperty(globalThis, "CSS", {
    configurable: true,
    value: originalCss,
    writable: true,
  });
  Object.defineProperty(globalThis, "Highlight", {
    configurable: true,
    value: originalHighlight,
    writable: true,
  });
  document.body.replaceChildren();
});

describe("Plan find highlights", () => {
  it("does nothing when CSS Custom Highlight is unavailable", () => {
    Object.defineProperty(globalThis, "CSS", {
      configurable: true,
      value: undefined,
      writable: true,
    });
    Object.defineProperty(globalThis, "Highlight", {
      configurable: true,
      value: undefined,
      writable: true,
    });

    expect(() => paintPlanFindHighlights([makeMatch()], 0)).not.toThrow();
    expect(() => clearPlanFindHighlights()).not.toThrow();
  });

  it("registers all matches and the active match separately", () => {
    const registry = new Map<string, unknown>();
    const Highlight = vi.fn(function (...ranges: Range[]) {
      return { ranges };
    });
    Object.defineProperty(globalThis, "CSS", {
      configurable: true,
      value: { highlights: registry },
      writable: true,
    });
    Object.defineProperty(globalThis, "Highlight", {
      configurable: true,
      value: Highlight,
      writable: true,
    });

    const first = makeMatch("first plan", 6, 10);
    const second = makeMatch("second plan", 7, 11);
    paintPlanFindHighlights([first, second], 1);

    expect(registry.get("tc-plan-find")).toEqual({
      ranges: expect.arrayContaining([expect.any(Range)]),
    });
    expect((registry.get("tc-plan-find") as { ranges: Range[] }).ranges).toHaveLength(2);
    expect((registry.get("tc-plan-find-active") as { ranges: Range[] }).ranges).toHaveLength(1);
    expect(Highlight).toHaveBeenCalledTimes(2);

    clearPlanFindHighlights();
    expect(registry.has("tc-plan-find")).toBe(false);
    expect(registry.has("tc-plan-find-active")).toBe(false);
  });

  it("centres the active match in the content scroller", () => {
    const container = document.createElement("main");
    const text = document.createTextNode("plan");
    container.append(text);
    document.body.append(container);
    const match = { end: 4, node: text, start: 0 };
    Object.defineProperty(container, "clientHeight", { value: 200 });
    vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
      bottom: 200,
      height: 200,
      left: 0,
      right: 0,
      toJSON: () => ({}),
      top: 0,
      width: 0,
      x: 0,
      y: 0,
    });
    const rangePrototype = Range.prototype as Range & {
      getBoundingClientRect?: () => DOMRect;
    };
    Object.defineProperty(rangePrototype, "getBoundingClientRect", {
      configurable: true,
      value: () => ({
        bottom: 130,
        height: 20,
        left: 0,
        right: 0,
        toJSON: () => ({}),
        top: 110,
        width: 10,
        x: 0,
        y: 110,
      }),
      writable: true,
    });

    scrollPlanFindMatchIntoView(match, container);

    expect(container.scrollTop).toBe(20);
  });
});
