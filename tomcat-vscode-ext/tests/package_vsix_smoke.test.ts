import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";

import { describe, expect, it } from "vitest";

import { crc32 } from "node:zlib";

import {
  assertPrebuiltArtifactsFresh,
  assertPublishableFiles,
  assertVsixExtractable,
  buildVscePackageArgs,
  buildVsixOutPath,
  bundledExecutableRelativePath,
  listPublishableFiles,
  packageVsix,
  packageVsixOrReuse,
  preparePublishDirectory,
} from "../scripts/package-vsix";
import { extractVsixLikeCursor } from "../scripts/vsix-extractable";

function writeU16(value: number): Buffer {
  const buffer = Buffer.alloc(2);
  buffer.writeUInt16LE(value);
  return buffer;
}

function writeU32(value: number): Buffer {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(value);
  return buffer;
}

function makeStoredZip(fileName: string, content: Buffer): Buffer {
  const name = Buffer.from(fileName);
  const crc = crc32(content) >>> 0;
  const local = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x03, 0x04]),
    writeU16(20),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU32(crc),
    writeU32(content.length),
    writeU32(content.length),
    writeU16(name.length),
    writeU16(0),
    name,
    content,
  ]);
  const central = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x01, 0x02]),
    writeU16(0x031e),
    writeU16(20),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU32(crc),
    writeU32(content.length),
    writeU32(content.length),
    writeU16(name.length),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU16(0),
    writeU32(0),
    writeU32(0),
    name,
  ]);
  const eocd = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x05, 0x06]),
    writeU16(0),
    writeU16(0),
    writeU16(1),
    writeU16(1),
    writeU32(central.length),
    writeU32(local.length),
    writeU16(0),
  ]);
  return Buffer.concat([local, central, eocd]);
}

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
        expect(() => assertVsixExtractable(packaged)).not.toThrow();
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

  it("rejects a VSIX that Cursor's unzipper cannot extract", async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "tomcat-vsix-integrity-"));
    const intactPath = path.join(root, "intact.vsix");
    const mismatchPath = path.join(root, "mismatch.vsix");
    const truncatedPath = path.join(root, "truncated.vsix");

    try {
      const intact = makeStoredZip("hello.txt", Buffer.from("hi\n"));
      await fs.writeFile(intactPath, intact);
      await expect(extractVsixLikeCursor(intactPath)).resolves.toBeUndefined();

      const mismatch = Buffer.from(intact);
      mismatch.writeUInt32LE(0xdeadbeef, 0);
      await fs.writeFile(mismatchPath, mismatch);
      await expect(extractVsixLikeCursor(mismatchPath)).rejects.toThrow(
        /invalid local file header signature/,
      );

      await fs.writeFile(truncatedPath, intact.subarray(0, intact.length - 10));
      await expect(extractVsixLikeCursor(truncatedPath)).rejects.toThrow();
    } finally {
      await fs.rm(root, { force: true, recursive: true });
    }
  });
});
