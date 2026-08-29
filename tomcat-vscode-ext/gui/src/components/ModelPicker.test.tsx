import type { ComponentProps } from "react";

import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ModelPicker, type ModelPickerModel } from "./ModelPicker";

const MODELS: ModelPickerModel[] = [
  {
    contextWindow: 400_000,
    contextWindowOptions: [400_000, 1_000_000],
    id: "fcodex/gpt-5.6-terra",
    modelName: "gpt-5.6-terra",
    selectedContextWindow: 400_000,
    selectedReasoningLevel: "xhigh",
    supportedReasoningLevels: ["low", "high", "xhigh"],
  },
  {
    contextWindow: 256_000,
    id: "plain-model",
    modelName: "Plain model",
    selectedReasoningLevel: "high",
    supportedReasoningLevels: ["high"],
  },
];

function renderPicker(overrides: Partial<ComponentProps<typeof ModelPicker>> = {}) {
  const onSelectContextWindow = vi.fn();
  const onSelectModel = vi.fn();
  const onSelectThinkingLevel = vi.fn();
  render(
    <ModelPicker
      models={MODELS}
      onSelectContextWindow={onSelectContextWindow}
      onSelectModel={onSelectModel}
      onSelectThinkingLevel={onSelectThinkingLevel}
      selectedModelId="fcodex/gpt-5.6-terra"
      {...overrides}
    />,
  );
  return { onSelectContextWindow, onSelectModel, onSelectThinkingLevel };
}

function modelOption(modelId: string): HTMLElement {
  const option = screen
    .getByText(modelId)
    .closest(".tc-model-picker-option");
  if (!(option instanceof HTMLElement)) {
    throw new Error(`Model option not found: ${modelId}`);
  }
  return option;
}

function viewportRect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    bottom: top + height,
    height,
    left,
    right: left + width,
    toJSON: () => ({}),
    top,
    width,
    x: left,
    y: top,
  } as DOMRect;
}

describe("ModelPicker", () => {
  function openConfig(row: HTMLElement) {
    fireEvent.mouseEnter(row);
    fireEvent.click(
      within(row).getByRole("button", { name: /^edit /i }),
    );
  }

  it("shows a combined label, swaps the selected check for Edit, and searches flexibly", () => {
    renderPicker();
    expect(screen.getByTestId("model-select").textContent).toContain("fcodex/gpt-5.6-terra Xhigh");
    fireEvent.click(screen.getByTestId("model-select"));

    const selectedOption = modelOption("fcodex/gpt-5.6-terra");
    expect(
      within(selectedOption).getByLabelText("Selected").classList.contains("codicon-check"),
    ).toBe(true);
    expect(
      within(selectedOption).getByTestId("model-edit-fcodex/gpt-5.6-terra").getAttribute("aria-hidden"),
    ).toBe("true");
    fireEvent.mouseEnter(selectedOption);
    expect(
      within(selectedOption).getByRole("button", { name: "Edit gpt-5.6-terra" }),
    ).toBeTruthy();
    expect(within(selectedOption).queryByLabelText("Selected")).toBeNull();
    openConfig(selectedOption);
    expect(screen.queryByTestId("model-info-card")).toBeNull();
    expect(
      screen.getByTestId("thinking-level-dropdown").classList.contains("is-side-right"),
    ).toBe(true);

    fireEvent.change(screen.getByLabelText("Search models"), { target: { value: "5.6 terra" } });
    expect(screen.getByText("fcodex/gpt-5.6-terra")).toBeTruthy();
    expect(screen.queryByText("plain-model")).toBeNull();
  });

  it("scrolls the selected model into view when the dropdown opens", () => {
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const scrollIntoView = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    try {
      renderPicker();
      fireEvent.click(screen.getByTestId("model-select"));

      expect(scrollIntoView).toHaveBeenCalledTimes(1);
      expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });

      fireEvent.change(screen.getByLabelText("Search models"), {
        target: { value: "5.6 terra" },
      });
      expect(scrollIntoView).toHaveBeenCalledTimes(1);
    } finally {
      if (originalScrollIntoView === undefined) {
        delete (HTMLElement.prototype as Partial<HTMLElement>).scrollIntoView;
      } else {
        HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
      }
    }
  });

  it("keeps the independent configuration portal open while Context and Effort options are clicked", () => {

    const { onSelectContextWindow, onSelectThinkingLevel } = renderPicker();
    fireEvent.click(screen.getByTestId("model-select"));
    const selectedOption = modelOption("fcodex/gpt-5.6-terra");
    openConfig(selectedOption);

    expect(screen.getByTestId("model-dropdown")).toBeTruthy();
    const config = screen.getByTestId("thinking-level-dropdown");
    expect(config.textContent).toContain("Context");
    expect(config.textContent).toContain("Effort");
    expect(config.textContent).not.toContain("fcodex/gpt-5.6-terra");
    expect(screen.getByTestId("model-dropdown").contains(config)).toBe(false);

    const contextOption = within(config).getByText("400K");
    fireEvent.mouseDown(contextOption);
    fireEvent.click(contextOption);
    const effortOption = within(config).getByText("Xhigh");
    fireEvent.mouseDown(effortOption);
    fireEvent.click(effortOption);

    expect(onSelectContextWindow).toHaveBeenCalledWith("fcodex/gpt-5.6-terra", 400_000);
    expect(onSelectThinkingLevel).toHaveBeenCalledWith("fcodex/gpt-5.6-terra", "xhigh");
    expect(screen.getByTestId("thinking-level-dropdown")).toBeTruthy();
  });

  it("closes only the configuration for picker clicks, and closes both layers for outside clicks", () => {
    renderPicker();
    fireEvent.click(screen.getByTestId("model-select"));
    const selectedOption = modelOption("fcodex/gpt-5.6-terra");
    openConfig(selectedOption);

    fireEvent.mouseDown(screen.getByLabelText("Search models"));
    expect(screen.queryByTestId("thinking-level-dropdown")).toBeNull();
    expect(screen.getByTestId("model-dropdown")).toBeTruthy();

    openConfig(selectedOption);
    fireEvent.mouseDown(document.body);
    expect(screen.queryByTestId("thinking-level-dropdown")).toBeNull();
    expect(screen.queryByTestId("model-dropdown")).toBeNull();
  });

  it("keeps Edit available for a single-tier model and shows its fallback Context option", () => {
    renderPicker({ selectedModelId: "plain-model" });
    fireEvent.click(screen.getByTestId("model-select"));
    const plainOption = modelOption("plain-model");
    fireEvent.mouseEnter(plainOption);
    fireEvent.click(within(plainOption).getByRole("button", { name: /edit plain model/i }));
    const config = screen.getByTestId("thinking-level-dropdown");
    expect(config.textContent).toContain("Context");
    expect(config.textContent).toContain("256K");
    expect(config.textContent).toContain("Effort");
    expect(within(config).getAllByLabelText("Selected")).toHaveLength(2);

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByTestId("thinking-level-dropdown")).toBeNull();
    expect(screen.getByTestId("model-dropdown")).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByTestId("model-dropdown")).toBeNull();
  });

  it("keeps Edit reachable even when a model exposes no configurable capability", () => {
    renderPicker({
      models: [
        {
          id: "no-options",
          modelName: "No options",
        },
      ],
      selectedModelId: "no-options",
    });
    fireEvent.click(screen.getByTestId("model-select"));
    const option = screen
      .getByTestId("model-dropdown")
      .querySelector<HTMLElement>('[data-model-id="no-options"]');
    expect(option).toBeTruthy();
    if (!(option instanceof HTMLElement)) {
      throw new Error("Expected the no-options model row.");
    }
    fireEvent.mouseEnter(option);

    expect(
      within(option).getByRole("button", { name: /edit no options/i }),
    ).toBeTruthy();
  });

  it("portals the configuration beside its source row, centers it vertically, clamps it, and closes on scroll", () => {
    const originalInnerWidth = window.innerWidth;
    const originalInnerHeight = window.innerHeight;
    let rowRect = viewportRect(100, 200, 120, 30);
    const rect = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function mockRect(this: HTMLElement) {
        return this.classList.contains("tc-model-config-popover")
          ? viewportRect(0, 0, 200, 180)
          : rowRect;
      });
    try {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: 800,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: 600,
      });
      renderPicker();
      fireEvent.click(screen.getByTestId("model-select"));
      openConfig(modelOption("fcodex/gpt-5.6-terra"));
      const config = screen.getByTestId("thinking-level-dropdown");
      expect(config.classList.contains("is-side-right")).toBe(true);
      expect(config.style.left).toBe("228px");
      // Source row center is 215px; 180px config is centered at top 125px.
      expect(config.style.top).toBe("125px");
      expect(screen.getByTestId("model-dropdown").contains(config)).toBe(false);

      rowRect = viewportRect(620, 200, 120, 30);
      fireEvent(window, new Event("resize"));
      expect(config.classList.contains("is-side-left")).toBe(true);
      expect(config.style.left).toBe("412px");

      rowRect = viewportRect(620, 570, 120, 30);
      fireEvent(window, new Event("resize"));
      // Row center is below the viewport; 412px is the bottom-clamped top
      // for a 180px menu in a 600px viewport with 8px padding.
      expect(config.style.top).toBe("412px");

      rowRect = viewportRect(100, 4, 120, 30);
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: 350,
      });
      fireEvent(window, new Event("resize"));
      expect(config.classList.contains("is-side-right")).toBe(true);
      expect(config.style.left).toBe("142px");
      expect(config.style.top).toBe("8px");

      fireEvent.scroll(document);
      expect(screen.queryByTestId("thinking-level-dropdown")).toBeNull();
    } finally {
      rect.mockRestore();
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: originalInnerWidth,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
    }
  });
});
