#!/usr/bin/env node

import readline from "node:readline";
import { appendFile } from "node:fs/promises";

const TINY_PNG_B64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9p8qAAAAAASUVORK5CYII=";

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
const mode = process.argv.includes("--hang")
  ? "hang"
  : process.argv.includes("--die-midcall")
    ? "die-midcall"
    : "normal";
const hangStartup = process.argv.includes("--hang-startup");
const recordIndex = process.argv.indexOf("--record");
const recordPath = recordIndex >= 0 ? process.argv[recordIndex + 1] : undefined;

function reply(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

async function record(method) {
  if (recordPath) {
    await appendFile(recordPath, `${method}\n`);
  }
}

for await (const line of input) {
  if (!line.trim()) continue;
  const message = JSON.parse(line);
  if (message.method === "initialize") {
    if (hangStartup) {
      await new Promise(() => {});
    }
    reply(message.id, {
      protocolVersion: message.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: "tomcat-fake-mcp", version: "1.0.0" },
    });
  } else if (message.method === "tools/list") {
    await record("tools/list");
    reply(message.id, {
      tools: [
        {
          name: "capture",
          description: "Returns a text result and a tiny PNG",
          inputSchema: { type: "object", properties: {} },
        },
        {
          name: "status",
          description: "Returns fake server status",
          inputSchema: { type: "object", properties: {} },
        },
      ],
    });
  } else if (message.method === "tools/call") {
    await record("tools/call");
    if (mode === "die-midcall") {
      process.exit(0);
    }
    if (mode === "hang") {
      await new Promise(() => {});
    }
    reply(message.id, {
      content: [
        { type: "text", text: "fake capture complete" },
        { type: "image", mimeType: "image/png", data: TINY_PNG_B64 },
      ],
    });
  }
}
