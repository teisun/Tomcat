import { describe, expect, it } from "vitest";

import {
  detectInlineFilePath,
  inferLanguageFromPath,
  looksLikeFilePathToken,
  splitInlinePathLocation,
} from "./inlinePath";

describe("inlinePath helpers", () => {
  it("accepts relative and absolute-looking file paths", () => {
    expect(looksLikeFilePathToken("src/app.ts")).toBe(true);
    expect(looksLikeFilePathToken("./src/app.ts")).toBe(true);
    expect(looksLikeFilePathToken("/workspace/src/app.ts")).toBe(true);
  });

  it("rejects urls and whitespace-heavy text", () => {
    expect(looksLikeFilePathToken("https://example.com")).toBe(false);
    expect(looksLikeFilePathToken("foo bar")).toBe(false);
    expect(looksLikeFilePathToken("plain text")).toBe(false);
  });

  it("parses colon line suffixes", () => {
    expect(splitInlinePathLocation("src/app.ts:42")).toEqual({
      line: 42,
      originalText: "src/app.ts:42",
      path: "src/app.ts",
    });
  });

  it("parses hash-style line suffixes", () => {
    expect(splitInlinePathLocation("a.rs#L9")).toEqual({
      line: 9,
      originalText: "a.rs#L9",
      path: "a.rs",
    });
  });

  it("parses colon line ranges and navigates to their first line", () => {
    expect(splitInlinePathLocation("src/AnswerCard.tsx:59-103")).toEqual({
      line: 59,
      originalText: "src/AnswerCard.tsx:59-103",
      path: "src/AnswerCard.tsx",
    });
  });

  it("parses hash-style line ranges and navigates to their first line", () => {
    expect(splitInlinePathLocation("src/lib.rs#L10-L20")).toEqual({
      line: 10,
      originalText: "src/lib.rs#L10-L20",
      path: "src/lib.rs",
    });
  });

  it("detects clickable inline file paths", () => {
    expect(detectInlineFilePath("gui/App.tsx")).toMatchObject({
      originalText: "gui/App.tsx",
      path: "gui/App.tsx",
    });
  });

  it("infers highlight languages from extensions", () => {
    expect(inferLanguageFromPath("src/app.ts")).toBe("typescript");
    expect(inferLanguageFromPath("src/lib.rs")).toBe("rust");
    expect(inferLanguageFromPath("README.md")).toBe("markdown");
  });
});
