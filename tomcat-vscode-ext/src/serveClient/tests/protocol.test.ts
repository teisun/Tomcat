import { describe, expect, it } from "vitest";

import { parseInitializePayload } from "../protocol";

describe("serve client protocol helpers", () => {
  it("parses serverVersion when the initialize payload includes it", () => {
    expect(
      parseInitializePayload({
        capabilities: ["prompt", "ask_question"],
        protocolVersion: 1,
        serverVersion: "0.1.20",
        sessionId: "s1",
      }),
    ).toEqual({
      attachmentRoot: null,
      capabilities: ["prompt", "ask_question"],
      protocolVersion: 1,
      serverVersion: "0.1.20",
      sessionId: "s1",
    });
  });

  it("parses the attachment root, which the host needs before it renders the webview", () => {
    expect(
      parseInitializePayload({
        attachmentRoot: "/home/u/.tomcat/sessions/attachments",
        capabilities: ["prompt", "ask_question"],
        protocolVersion: 1,
      }).attachmentRoot,
    ).toBe("/home/u/.tomcat/sessions/attachments");

    // An older server simply omits it; images then have nowhere to load from, which the
    // host degrades to "unavailable" rather than guessing at a path.
    expect(
      parseInitializePayload({
        capabilities: ["prompt", "ask_question"],
        protocolVersion: 1,
      }).attachmentRoot,
    ).toBeNull();
  });

  it("treats missing or invalid serverVersion as null", () => {
    expect(
      parseInitializePayload({
        capabilities: ["prompt", "ask_question"],
        protocolVersion: 1,
      }).serverVersion,
    ).toBeNull();

    expect(
      parseInitializePayload({
        capabilities: ["prompt", "ask_question"],
        protocolVersion: 1,
        serverVersion: 114,
      }).serverVersion,
    ).toBeNull();
  });
});
