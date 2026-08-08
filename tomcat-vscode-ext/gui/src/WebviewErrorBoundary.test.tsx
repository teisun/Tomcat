import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { WebviewErrorBoundary } from "./WebviewErrorBoundary";

function ThrowOnRender(): never {
  throw new Error("render exploded");
}

describe("WebviewErrorBoundary", () => {
  it("renders a recoverable fallback and reports a child render failure", () => {
    const reportError = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    try {
      render(
        <WebviewErrorBoundary reportError={reportError}>
          <ThrowOnRender />
        </WebviewErrorBoundary>,
      );

      expect(screen.getByTestId("webview-error-fallback").textContent).toContain("render exploded");
      expect(reportError).toHaveBeenCalledWith(expect.objectContaining({ message: "render exploded" }));
    } finally {
      consoleError.mockRestore();
    }
  });

  it("reports a global browser error instead of leaving the webview blank", () => {
    const reportError = vi.fn();
    render(
      <WebviewErrorBoundary reportError={reportError}>
        <div>healthy view</div>
      </WebviewErrorBoundary>,
    );

    fireEvent(
      window,
      new ErrorEvent("error", {
        error: new Error("async initialization exploded"),
        message: "async initialization exploded",
      }),
    );

    expect(screen.getByTestId("webview-error-fallback").textContent).toContain(
      "async initialization exploded",
    );
    expect(reportError).toHaveBeenCalledWith(
      expect.objectContaining({ message: "async initialization exploded" }),
    );
  });
});
