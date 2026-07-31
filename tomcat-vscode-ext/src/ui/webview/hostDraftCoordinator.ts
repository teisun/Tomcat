/**
 * One serialization lane per source session. Draft-affecting host work enters the lane;
 * a fork fence is just another lane item, so it observes every earlier mutation and
 * excludes every later mutation without globally blocking unrelated sessions.
 */
export class HostDraftCoordinator {
  private readonly tails = new Map<string, Promise<void>>();

  run<T>(sessionId: string, work: () => Promise<T>): Promise<T> {
    const previous = this.tails.get(sessionId) ?? Promise.resolve();
    const current = previous.catch(() => undefined).then(work);
    const tail = current.then(() => undefined, () => undefined);
    this.tails.set(sessionId, tail);
    void tail.finally(() => {
      if (this.tails.get(sessionId) === tail) this.tails.delete(sessionId);
    });
    return current;
  }

  fence(sessionId: string): Promise<void> {
    return this.run(sessionId, async () => undefined);
  }

  isPending(sessionId: string): boolean {
    return this.tails.has(sessionId);
  }
}
