import { describe, expect, it } from "vitest";

import { normalizeConnectorView } from "./connectorsProtocol";

describe("normalizeConnectorView", () => {
  it("shows global configuration for both current and legacy serve payloads", () => {
    const legacy = normalizeConnectorView({
      name: "playwright",
      source: "User",
      state: "connected",
    });
    const current = normalizeConnectorView({
      configPath: "~/.tomcat/mcp.json",
      name: "github",
      source: "Global",
      state: "connected",
    });

    expect(legacy).toMatchObject({
      configPath: "~/.tomcat/mcp.json",
      source: "Global",
    });
    expect(current).toMatchObject({
      configPath: "~/.tomcat/mcp.json",
      source: "Global",
    });
  });
});
