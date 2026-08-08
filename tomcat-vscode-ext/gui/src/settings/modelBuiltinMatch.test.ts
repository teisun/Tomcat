import { describe, expect, it } from "vitest";

import type { SettingsModelView } from "../../../src/shared/settingsProtocol";
import { findBuiltinModelByName } from "./modelBuiltinMatch";

function builtinModel(overrides: Partial<SettingsModelView> = {}): SettingsModelView {
  return {
    api: "openai",
    apiKeyEnv: "TEST_API_KEY",
    capabilities: {
      files: false,
      reasoning: true,
      tools: true,
      vision: false,
      webSearch: false,
    },
    id: "deepseek-v4-flash",
    keyPresent: false,
    modelName: "deepseek-v4-flash",
    provider: "deepseek",
    source: "builtin",
    ...overrides,
  };
}

describe("findBuiltinModelByName", () => {
  it("matches a relay-facing model name instead of its Tomcat id", () => {
    const match = findBuiltinModelByName(
      [builtinModel({ id: "gpt-5.6", modelName: "gpt-5.6" })],
      "gpt-5.6",
    );

    expect(match?.id).toBe("gpt-5.6");
  });

  it("uses the first built-in entry in catalog order when names collide", () => {
    const first = builtinModel({
      contextWindow: 128000,
      id: "deepseek-v4-flash",
    });
    const second = builtinModel({
      contextWindow: 256000,
      id: "utility-flash",
    });

    expect(findBuiltinModelByName([first, second], "deepseek-v4-flash")).toBe(first);
  });

  it("ignores user models and returns null for blank or unknown names", () => {
    const models = [
      builtinModel({ id: "relay/deepseek-v4-flash", source: "user" }),
      builtinModel({ id: "deepseek-v4-pro", modelName: "deepseek-v4-pro" }),
    ];

    expect(findBuiltinModelByName(models, "deepseek-v4-flash")).toBeNull();
    expect(findBuiltinModelByName(models, "  ")).toBeNull();
    expect(findBuiltinModelByName(models, "unknown")).toBeNull();
  });
});
