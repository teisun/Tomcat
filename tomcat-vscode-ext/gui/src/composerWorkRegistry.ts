export interface ComposerWorkTicket {
  operationId: string;
  sequence: number;
  sessionId: string;
}

interface PendingWork extends ComposerWorkTicket {
  promise: Promise<void>;
  resolve(): void;
}

/**
 * Tracks only work started before a New Session click for one source session.
 * Work created after the recorded cutoff is deliberately not awaited.
 */
export class ComposerWorkRegistry {
  private operationCounter = 0;
  private readonly nextSequenceBySession = new Map<string, number>();
  private readonly pendingByOperation = new Map<string, PendingWork>();

  begin(sessionId: string, kind: "attach" | "drop" | "paste" | "picker" | "reference"): ComposerWorkTicket {
    const sequence = (this.nextSequenceBySession.get(sessionId) ?? 0) + 1;
    this.nextSequenceBySession.set(sessionId, sequence);
    const operationId = `${kind}-${Date.now()}-${++this.operationCounter}`;
    let resolve!: () => void;
    const promise = new Promise<void>((next) => {
      resolve = next;
    });
    const ticket = { operationId, sequence, sessionId };
    this.pendingByOperation.set(operationId, { ...ticket, promise, resolve });
    return ticket;
  }

  cutoff(sessionId: string): number {
    return this.nextSequenceBySession.get(sessionId) ?? 0;
  }

  complete(operationId: string): boolean {
    const pending = this.pendingByOperation.get(operationId);
    if (!pending) return false;
    this.pendingByOperation.delete(operationId);
    pending.resolve();
    return true;
  }

  async waitForCutoff(sessionId: string, cutoff: number): Promise<void> {
    const work = [...this.pendingByOperation.values()]
      .filter((pending) => pending.sessionId === sessionId && pending.sequence <= cutoff)
      .map((pending) => pending.promise);
    await Promise.all(work);
  }

  pendingCount(sessionId?: string): number {
    return [...this.pendingByOperation.values()].filter(
      (pending) => sessionId === undefined || pending.sessionId === sessionId,
    ).length;
  }
}
