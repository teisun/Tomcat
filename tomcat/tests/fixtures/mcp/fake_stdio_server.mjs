#!/usr/bin/env node

import readline from "node:readline";

const TINY_PNG_B64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9p8qAAAAAASUVORK5CYII=";

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

function reply(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

for await (const line of input) {
  if (!line.trim()) continue;
  const message = JSON.parse(line);
  if (message.method === "initialize") {
    reply(message.id, {
      protocolVersion: message.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: "tomcat-fake-mcp", version: "1.0.0" },
    });
  } else if (message.method === "tools/list") {
    reply(message.id, {
      tools: [
        {
          name: "capture",
          description: "Returns a text result and a tiny PNG",
          inputSchema: { type: "object", properties: {} },
        },
      ],
    });
  } else if (message.method === "tools/call") {
    reply(message.id, {
      content: [
        { type: "text", text: "fake capture complete" },
        { type: "image", mimeType: "image/png", data: TINY_PNG_B64 },
      ],
    });
  }
}
