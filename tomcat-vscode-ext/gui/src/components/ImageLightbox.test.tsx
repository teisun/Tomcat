import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ImageLightbox, type ZoomedImage } from "./ImageLightbox";

const IMAGE: ZoomedImage = {
  alt: "diagram",
  src: "vscode-webview://workspace/docs/diagram.png",
};

describe("ImageLightbox", () => {
  it("renders nothing when closed", () => {
    const { container } = render(<ImageLightbox image={null} onClose={() => undefined} />);
    expect(container.childElementCount).toBe(0);
  });

  it("renders an accessible dialog and moves focus to the close button", () => {
    render(<ImageLightbox image={IMAGE} onClose={() => undefined} />);
    const dialog = screen.getByRole("dialog", { name: "Image preview" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(screen.getByTestId("image-lightbox-close"));
  });

  it("restores focus to the previously active element when it closes", () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const { rerender, unmount } = render(
      <ImageLightbox image={IMAGE} onClose={() => undefined} />,
    );
    expect(document.activeElement).toBe(screen.getByTestId("image-lightbox-close"));

    rerender(<ImageLightbox image={null} onClose={() => undefined} />);
    expect(document.activeElement).toBe(trigger);

    trigger.focus();
    rerender(<ImageLightbox image={IMAGE} onClose={() => undefined} />);
    unmount();
    expect(document.activeElement).toBe(trigger);

    trigger.remove();
  });

  it("closes on overlay, dialog blank area, close button, and Escape", () => {
    const onClose = vi.fn();
    render(<ImageLightbox image={IMAGE} onClose={onClose} />);

    fireEvent.mouseDown(screen.getByTestId("image-lightbox-overlay"));
    fireEvent.mouseDown(screen.getByTestId("image-lightbox"));
    fireEvent.click(screen.getByTestId("image-lightbox-close"));
    fireEvent.keyDown(document, { key: "Escape" });

    expect(onClose).toHaveBeenCalledTimes(4);
  });

  it("does not close when clicking the enlarged image itself", () => {
    const onClose = vi.fn();
    render(<ImageLightbox image={IMAGE} onClose={onClose} />);

    fireEvent.mouseDown(screen.getByTestId("image-lightbox-image"));

    expect(onClose).not.toHaveBeenCalled();
  });
});
