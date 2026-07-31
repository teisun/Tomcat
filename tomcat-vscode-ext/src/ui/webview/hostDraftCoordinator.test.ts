import { describe, expect, it } from "vitest";

import { HostDraftCoordinator } from "./hostDraftCoordinator";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((next) => { resolve = next; });
  return { promise, resolve };
}

describe("HostDraftCoordinator", () => {
  it("serializes one source session through the fork fence", async () => {
    const coordinator = new HostDraftCoordinator();
    const gate = deferred();
    const order: string[] = [];
    const first = coordinator.run("source", async () => {
      order.push("ingest:start");
      await gate.promise;
      order.push("ingest:end");
    });
    const fence = coordinator.fence("source").then(() => order.push("fence"));
    const later = coordinator.run("source", async () => { order.push("later"); });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(order).toEqual(["ingest:start"]);
    gate.resolve();
    await Promise.all([first, fence, later]);
    expect(order).toEqual(["ingest:start", "ingest:end", "fence", "later"]);
  });

  it("does not block unrelated sessions and recovers after failure", async () => {
    const coordinator = new HostDraftCoordinator();
    const sourceGate = deferred();
    const source = coordinator.run("source", () => sourceGate.promise);
    let otherRan = false;
    await coordinator.run("other", async () => { otherRan = true; });
    expect(otherRan).toBe(true);
    sourceGate.resolve();
    await source;

    await expect(coordinator.run("source", async () => { throw new Error("boom"); })).rejects.toThrow("boom");
    await expect(coordinator.run("source", async () => "recovered")).resolves.toBe("recovered");
  });
});
