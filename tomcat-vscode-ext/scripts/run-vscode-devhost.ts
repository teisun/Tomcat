import * as fs from "node:fs/promises";
import * as net from "node:net";
import * as os from "node:os";
import * as path from "node:path";
import { execFileSync } from "node:child_process";

import { runTests } from "@vscode/test-electron";

import {
  createHostE2eFixture,
  resolveVsCodeExecutable,
  seedChatUserSettings,
} from "./e2eHostFixture";

async function reserveLoopbackPort(): Promise<number> {
  const server = net.createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  if (port <= 0) {
    throw new Error("Failed to reserve a loopback port for the VS Code workbench driver");
  }
  return port;
}

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, "..");
  const extensionTestsPath = path.resolve(
    extensionDevelopmentPath,
    "out/test/suite/index.js",
  );
  const transientServeFailures = Math.max(
    0,
    Number(process.env.TOMCAT_VSCODE_TEST_TRANSIENT_SERVE_FAILURES ?? "0") || 0,
  );
  const fixture = await createHostE2eFixture({ transientServeFailures });
  const extensionTestsEnv = { ...fixture.env };
  for (const name of [
    "TOMCAT_E2E_GREP",
    "TOMCAT_E2E_SCREENSHOT",
    "TOMCAT_E2E_PLAN_FIND_CAPTURE_ONLY",
    "TOMCAT_EXPECT_TRANSIENT_SERVE_RECOVERY",
    "TOMCAT_VSCODE_TEST_TRANSIENT_SERVE_FAILURES",
    "TOMCAT_VSIX_VISUAL_ARTIFACTS_DIR",
  ]) {
    const value = process.env[name];
    if (value) {
      extensionTestsEnv[name] = value;
    }
  }
  // Coding-agent shells may run Electron as Node; never leak that mode into VS Code.
  delete extensionTestsEnv.ELECTRON_RUN_AS_NODE;
  const electronRunAsNode = process.env.ELECTRON_RUN_AS_NODE;
  delete process.env.ELECTRON_RUN_AS_NODE;
  const userDataDir = await fs.mkdtemp(path.join(os.tmpdir(), "tdev-"));
  const cdpPort = await reserveLoopbackPort();

  try {
    execFileSync("npm", ["run", "build"], {
      cwd: extensionDevelopmentPath,
      stdio: "inherit",
    });
    await seedChatUserSettings(userDataDir);

    await fs.access(extensionTestsPath);
    await runTests({
      extensionDevelopmentPath,
      extensionTestsEnv: {
        ...extensionTestsEnv,
        TOMCAT_E2E_CDP_PORT: String(cdpPort),
      },
      extensionTestsPath,
      launchArgs: [
        `--remote-debugging-port=${cdpPort}`,
        `--user-data-dir=${userDataDir}`,
        path.resolve(extensionDevelopmentPath, ".."),
      ],
      vscodeExecutablePath: resolveVsCodeExecutable(),
    });
  } finally {
    if (electronRunAsNode === undefined) {
      delete process.env.ELECTRON_RUN_AS_NODE;
    } else {
      process.env.ELECTRON_RUN_AS_NODE = electronRunAsNode;
    }
    await fixture.cleanup();
    await fs.rm(userDataDir, { force: true, recursive: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
