import { fetch, WebSocket } from "undici";

const CDP_PORT_ENV = "TOMCAT_E2E_CDP_PORT";

type CdpTarget = {
  title?: string;
  type?: string;
  url?: string;
  webSocketDebuggerUrl?: string;
};

type CdpReply = {
  error?: { code: number; message: string };
  id?: number;
  result?: unknown;
};

type FindWidgetState = {
  hasMatch: boolean;
  open: boolean;
  text: string;
  value: string;
};

class CdpClient {
  private nextId = 1;
  private readonly pending = new Map<
    number,
    { reject(error: Error): void; resolve(value: unknown): void }
  >();

  private constructor(private readonly socket: WebSocket) {
    socket.addEventListener("message", (event) => {
      if (typeof event.data !== "string") {
        return;
      }
      let reply: CdpReply;
      try {
        reply = JSON.parse(event.data) as CdpReply;
      } catch {
        return;
      }
      if (reply.id === undefined) {
        return;
      }
      const waiter = this.pending.get(reply.id);
      if (!waiter) {
        return;
      }
      this.pending.delete(reply.id);
      if (reply.error) {
        waiter.reject(new Error(`CDP ${reply.error.code}: ${reply.error.message}`));
      } else {
        waiter.resolve(reply.result);
      }
    });
    socket.addEventListener("close", () => {
      for (const waiter of this.pending.values()) {
        waiter.reject(new Error("CDP websocket closed"));
      }
      this.pending.clear();
    });
  }

  static async connect(url: string): Promise<CdpClient> {
    const socket = new WebSocket(url);
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("Timed out opening CDP websocket")), 10_000);
      socket.addEventListener("open", () => {
        clearTimeout(timeout);
        resolve();
      }, { once: true });
      socket.addEventListener("error", () => {
        clearTimeout(timeout);
        reject(new Error("Failed to open CDP websocket"));
      }, { once: true });
    });
    return new CdpClient(socket);
  }

  async send(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    const id = this.nextId++;
    const result = new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, { reject, resolve });
    });
    this.socket.send(JSON.stringify({ id, method, params }));
    return result;
  }

  close(): void {
    this.socket.close();
  }
}

async function waitFor<T>(
  read: () => Promise<T>,
  accept: (value: T) => boolean,
  errorMessage: string,
  timeoutMs = 10_000,
): Promise<T> {
  const startedAt = Date.now();
  let lastValue: T | undefined;
  while (Date.now() - startedAt < timeoutMs) {
    lastValue = await read();
    if (accept(lastValue)) {
      return lastValue;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`${errorMessage}; lastValue=${JSON.stringify(lastValue)}`);
}

async function discoverWorkbenchTarget(port: number): Promise<CdpTarget> {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`);
  if (!response.ok) {
    throw new Error(`CDP target discovery failed: HTTP ${response.status}`);
  }
  const targets = (await response.json()) as CdpTarget[];
  const pages = targets.filter(
    (target) => target.type === "page" && typeof target.webSocketDebuggerUrl === "string",
  );
  const selected =
    pages.find((target) => /Extension Development Host/iu.test(target.title ?? ""))
    ?? pages.find((target) => !/^devtools:/u.test(target.url ?? ""))
    ?? pages[0];
  if (!selected?.webSocketDebuggerUrl) {
    throw new Error(`No VS Code workbench CDP target found: ${JSON.stringify(targets)}`);
  }
  return selected;
}

function findWidgetExpression(): string {
  return `(() => {
    const visible = (element) => {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0;
    };
    const inputs = Array.from(document.querySelectorAll("input"));
    const input = inputs.find((candidate) => {
      if (!visible(candidate)) return false;
      const ownText = [candidate.getAttribute("aria-label"), candidate.getAttribute("placeholder"), candidate.title]
        .filter(Boolean).join(" ").toLowerCase();
      let parent = candidate;
      let classText = "";
      for (let index = 0; index < 7 && parent; index += 1, parent = parent.parentElement) {
        classText += " " + String(parent.className || "").toLowerCase();
      }
      return ownText.includes("find") || classText.includes("find-widget") || classText.includes("find-part");
    });
    if (!input) return { hasMatch: false, open: false, text: "", value: "" };
    let widget = input.parentElement || input;
    let parent = input.parentElement;
    for (let index = 0; index < 10 && parent; index += 1, parent = parent.parentElement) {
      const classText = String(parent.className || "").toLowerCase();
      if (
        classText.includes("find-widget")
        || classText.includes("simple-find-part")
        || classText.includes("webview-find")
      ) {
        widget = parent;
      }
    }
    const searchableText = [
      widget.textContent,
      ...Array.from(widget.querySelectorAll("[aria-label], [title]"), (element) =>
        [element.getAttribute("aria-label"), element.getAttribute("title")].filter(Boolean).join(" ")
      ),
    ].filter(Boolean).join(" ");
    const matchNavigation = Array.from(widget.querySelectorAll("button, [role=button], .action-item"))
      .filter((element) => /(?:previous|next) match/i.test(
        [element.getAttribute("aria-label"), element.getAttribute("title")].filter(Boolean).join(" ")
      ));
    const hasMatch = matchNavigation.some((element) =>
      !element.hasAttribute("disabled")
      && element.getAttribute("aria-disabled") !== "true"
      && !String(element.className || "").toLowerCase().includes("disabled")
    );
    return {
      hasMatch,
      open: true,
      text: String(searchableText).replace(/\\s+/g, " ").trim(),
      value: String(input.value || ""),
    };
  })()`;
}

export class WorkbenchFindDriver {
  private constructor(private readonly cdp: CdpClient) {}

  static async connectFromEnvironment(): Promise<WorkbenchFindDriver> {
    const rawPort = process.env[CDP_PORT_ENV];
    const port = Number(rawPort);
    if (!Number.isInteger(port) || port <= 0) {
      throw new Error(`${CDP_PORT_ENV} must contain the Dev Host CDP port`);
    }
    const target = await waitFor(
      async () => discoverWorkbenchTarget(port).catch(() => null),
      (candidate): candidate is CdpTarget => candidate !== null,
      "Timed out discovering the VS Code workbench CDP target",
      20_000,
    );
    if (!target?.webSocketDebuggerUrl) {
      throw new Error("Discovered VS Code workbench target has no websocket URL");
    }
    return new WorkbenchFindDriver(await CdpClient.connect(target.webSocketDebuggerUrl));
  }

  private async evaluate<T>(expression: string): Promise<T> {
    const raw = await this.cdp.send("Runtime.evaluate", {
      awaitPromise: true,
      expression,
      returnByValue: true,
    }) as { exceptionDetails?: unknown; result?: { value?: T } };
    if (raw.exceptionDetails) {
      throw new Error(`Workbench evaluation failed: ${JSON.stringify(raw.exceptionDetails)}`);
    }
    return raw.result?.value as T;
  }

  private async readFindWidget(): Promise<FindWidgetState> {
    return this.evaluate<FindWidgetState>(findWidgetExpression());
  }

  private async dispatchKey(
    type: "keyDown" | "keyUp" | "rawKeyDown",
    key: string,
    code: string,
    modifiers = 0,
  ): Promise<void> {
    await this.cdp.send("Input.dispatchKeyEvent", { code, key, modifiers, type });
  }

  async findUniqueText(query: string): Promise<FindWidgetState> {
    const accelerator = process.platform === "darwin" ? 4 : 2;
    const current = await this.readFindWidget();
    if (!current.open) {
      await this.dispatchKey("rawKeyDown", "f", "KeyF", accelerator);
      await this.dispatchKey("keyUp", "f", "KeyF", accelerator);
    }
    await waitFor(
      () => this.readFindWidget(),
      (state) => state.open,
      "Platform Find shortcut did not open VS Code's native Find Widget",
    );

    await this.dispatchKey("rawKeyDown", "a", "KeyA", accelerator);
    await this.dispatchKey("keyUp", "a", "KeyA", accelerator);
    await this.cdp.send("Input.insertText", { text: query });

    return waitFor(
      () => this.readFindWidget(),
      (state) => state.open && state.value === query && state.hasMatch,
      `Native Find Widget did not report the unique match for ${query}`,
    );
  }

  async closeFind(): Promise<void> {
    await this.cdp.send("Input.dispatchKeyEvent", {
      code: "Escape",
      key: "Escape",
      nativeVirtualKeyCode: 27,
      type: "keyDown",
      windowsVirtualKeyCode: 27,
    });
    await this.cdp.send("Input.dispatchKeyEvent", {
      code: "Escape",
      key: "Escape",
      nativeVirtualKeyCode: 27,
      type: "keyUp",
      windowsVirtualKeyCode: 27,
    });
    await waitFor(
      () => this.readFindWidget(),
      (state) => !state.open,
      "Escape did not close VS Code's native Find Widget",
    );
  }

  async captureScreenshot(): Promise<string> {
    const result = await this.cdp.send("Page.captureScreenshot", {
      format: "png",
      fromSurface: true,
    }) as { data?: unknown };
    if (typeof result.data !== "string" || result.data.length === 0) {
      throw new Error("CDP did not return a PNG screenshot");
    }
    return result.data;
  }

  close(): void {
    this.cdp.close();
  }
}
