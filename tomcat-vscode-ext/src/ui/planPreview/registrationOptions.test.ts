import { describe, expect, it } from "vitest";

import { PLAN_PREVIEW_EDITOR_OPTIONS } from "./registrationOptions";

describe("Plan preview editor registration options", () => {
  it("enables the native VS Code Find Widget while retaining hidden state", () => {
    expect(PLAN_PREVIEW_EDITOR_OPTIONS).toEqual({
      supportsMultipleEditorsPerDocument: false,
      webviewOptions: {
        enableFindWidget: true,
        retainContextWhenHidden: true,
      },
    });
  });
});
