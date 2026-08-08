import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";

import { describe, expect, it } from "vitest";

import {
  assertPrebuiltArtifactsFresh,
  assertPublishableFiles,
  buildVscePackageArgs,
  buildVsixOutPath,
  bundledExecutableRelativePath,
  listPublishableFiles,
  packageVsix,
  packageVsixOrReuse,
  preparePublishDirectory,
} from "../scripts/package-vsix";

describe("VSIX packaging", () => {
  it(
    "packages non-interactively and excludes source-only directories",
    async () => {
      const extensionRoot = path.resolve(__dirname, "..");
      const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "tomcat-vsix-test-"));
      const vsixPath = path.join(tempRoot, "tomcat-vscode-ext.vsix");
      let publishRoot: string | undefined;

      try {
        const packaged = packageVsixOrReuse({ extensionRoot, outPath: vsixPath });
        publishRoot = preparePublishDirectory(extensionRoot);
        const fileList = listPublishableFiles(publishRoot, extensionRoot);
        assertPublishableFiles(fileList);
        expect(fileList).toContain("CHANGELOG.md");
        expect(fileList).toContain("README.md");
        expect(fileList).toContain("LICENSE");
        expect(fileList).toContain("gui/dist/index.js");
        expect(fileList).toContain("media/icon.png");
        expect(fileList).toContain("media/tomcat.svg");
        expect(fileList).not.toContain("src/extension.ts");
        expect(fileList).not.toContain("gui/src/App.tsx");
        expect(fileList).not.toContain("tests/serve_e2e.test.ts");

        const stat = await fs.stat(packaged);
        expect(stat.isFile()).toBe(true);
      } finally {
        if (publishRoot) {
          await fs.rm(publishRoot, { force: true, recursive: true });
        }
        await fs.rm(tempRoot, { force: true, recursive: true });
      }
    },
    300_000,
  );

  it("keeps bundling opt-in and stages the bundled executable when requested", async () => {
    const extensionRoot = path.resolve(__dirname, "..");
    const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "tomcat-vsix-bundle-"));
    const fakeBinaryPath = path.join(tempRoot, "fake-tomcat");
    const vsixPath = path.join(tempRoot, "tomcat-vscode-ext-bundled.vsix");
    let plainPublishRoot: string | undefined;
    let bundledPublishRoot: string | undefined;

    try {
      await fs.writeFile(fakeBinaryPath, "#!/usr/bin/env bash\nprintf 'fake'\n", "utf8");
      await fs.chmod(fakeBinaryPath, 0o755);

      plainPublishRoot = preparePublishDirectory(extensionRoot);
      const plainFileList = listPublishableFiles(plainPublishRoot, extensionRoot);
      expect(plainFileList).not.toContain(bundledExecutableRelativePath("linux-x64"));

      bundledPublishRoot = preparePublishDirectory(extensionRoot, {
        bundleBinaryPath: fakeBinaryPath,
        target: "linux-x64",
      });
      const bundledFileList = listPublishableFiles(bundledPublishRoot, extensionRoot);
      assertPublishableFiles(bundledFileList, {
        bundleBinaryPath: fakeBinaryPath,
        target: "linux-x64",
      });
      expect(bundledFileList).toContain("bin/tomcat");

      const packaged = packageVsix({
        bundleBinaryPath: fakeBinaryPath,
        extensionRoot,
        outPath: vsixPath,
        target: "linux-x64",
      });
      const stat = await fs.stat(packaged);
      expect(stat.isFile()).toBe(true);
    } finally {
      if (plainPublishRoot) {
        await fs.rm(plainPublishRoot, { force: true, recursive: true });
      }
      if (bundledPublishRoot) {
        await fs.rm(bundledPublishRoot, { force: true, recursive: true });
      }
      await fs.rm(tempRoot, { force: true, recursive: true });
    }
  }, 300_000);

  it("builds target-aware package args and default output paths", () => {
    const extensionRoot = path.resolve(__dirname, "..");

    expect(
      buildVsixOutPath(extensionRoot, { name: "tomcat-vscode-ext", version: "0.1.3" }, "linux-x64"),
    ).toBe(path.join(extensionRoot, "tomcat-vscode-ext-0.1.3-linux-x64.vsix"));
    expect(
      buildVscePackageArgs("/tmp/tomcat-vscode-ext.vsix", "linux-x64"),
    ).toEqual([
      "package",
      "--no-dependencies",
      "--target",
      "linux-x64",
      "--out",
      "/tmp/tomcat-vscode-ext.vsix",
    ]);
  });

  it("rejects skip-build packaging when source is newer than its artifact", async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "tomcat-vsix-freshness-"));
    const sourcePath = path.join(root, "gui", "src", "App.tsx");
    const artifactPath = path.join(root, "gui", "dist", "index.js");
    const coreSourcePath = path.join(root, "src", "extension.ts");
    const coreArtifactPath = path.join(root, "out", "extension.js");

    try {
      await fs.mkdir(path.dirname(sourcePath), { recursive: true });
      await fs.mkdir(path.dirname(artifactPath), { recursive: true });
      await fs.mkdir(path.dirname(coreSourcePath), { recursive: true });
      await fs.mkdir(path.dirname(coreArtifactPath), { recursive: true });
      await Promise.all([
        fs.writeFile(sourcePath, "export {};\n"),
        fs.writeFile(artifactPath, "export {};\n"),
        fs.writeFile(coreSourcePath, "export {};\n"),
        fs.writeFile(coreArtifactPath, "export {};\n"),
      ]);

      const now = Date.now();
      await fs.utimes(artifactPath, now / 1000 - 10, now / 1000 - 10);
      await fs.utimes(coreArtifactPath, now / 1000, now / 1000);
      await fs.utimes(coreSourcePath, now / 1000 - 10, now / 1000 - 10);
      await fs.utimes(sourcePath, now / 1000, now / 1000);

      expect(() => assertPrebuiltArtifactsFresh(root)).toThrow(
        "source gui/src/App.tsx is newer than artifact gui/dist/index.js",
      );
    } finally {
      await fs.rm(root, { force: true, recursive: true });
    }
  });
});
