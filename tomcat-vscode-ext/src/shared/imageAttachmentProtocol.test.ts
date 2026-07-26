import { describe, expect, it } from "vitest";

import {
  IMAGE_MAX_BYTES,
  decodeBase64Strict,
  safeAttachmentFilename,
  validateImageCandidate,
} from "./imageAttachmentProtocol";

describe("imageAttachmentProtocol utilities", () => {
  it("strictly decodes canonical base64 and rejects malformed input", () => {
    expect(decodeBase64Strict(Buffer.from("hello").toString("base64"))?.toString()).toBe(
      "hello",
    );
    expect(decodeBase64Strict("not base64")) .toBeNull();
    expect(decodeBase64Strict("YQ=")) .toBeNull();
    expect(decodeBase64Strict("")) .toBeNull();
  });

  it("normalizes unsafe filenames to a basename or fallback", () => {
    expect(safeAttachmentFilename("../../screen.png", "fallback.png")).toBe(
      "screen.png",
    );
    expect(safeAttachmentFilename("..", "fallback.png")).toBe("fallback.png");
    expect(safeAttachmentFilename("\u0000shot.png", "fallback.png")).toBe(
      "shot.png",
    );
  });

  // Regression guard for the blacklist this replaced. It refused `style=`, `<style>`
  // and `url(`, which is to say it refused nearly everything Figma, Illustrator and
  // Sketch export — while a namespace-aliased `x:href` sailed past it. Design-tool SVG
  // must be attachable; the security story is `<img>` secure static mode, asserted in
  // the webview suite instead.
  it("accepts real design-tool SVG that the old text blacklist rejected", () => {
    const designToolSvgs = [
      '<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:red" width="8" height="8"/></svg>',
      '<svg xmlns="http://www.w3.org/2000/svg"><style>.a{fill:#0af}</style><rect class="a" width="8" height="8"/></svg>',
      '<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="grad"/></defs><rect fill="url(#grad)" width="8" height="8"/></svg>',
    ];
    for (const svg of designToolSvgs) {
      expect(
        validateImageCandidate({
          dataBase64: Buffer.from(svg).toString("base64"),
          filename: "icon.svg",
          mimeType: "image/svg+xml",
        }),
      ).toMatchObject({ filename: "icon.svg", ok: true });
    }
  });

  it("validates MIME, base64 shape and decoded byte size", () => {
    const png = validateImageCandidate({
      dataBase64: Buffer.from([1, 2, 3]).toString("base64"),
      filename: null,
      mimeType: "image/png",
    });
    expect(png).toMatchObject({ filename: "pasted-image.png", ok: true });

    expect(
      validateImageCandidate({
        dataBase64: Buffer.from([1]).toString("base64"),
        mimeType: "image/bmp",
      }),
    ).toMatchObject({ ok: false });

    expect(
      validateImageCandidate({
        dataBase64: Buffer.alloc(IMAGE_MAX_BYTES + 1).toString("base64"),
        mimeType: "image/png",
      }),
    ).toMatchObject({ error: "exceeds 4.5 MB", ok: false });
  });
});
