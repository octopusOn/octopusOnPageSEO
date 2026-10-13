import { describe, expect, it } from "vitest";

import { classifyAsset, fromManifest } from "./releases";

describe("classifyAsset", () => {
  it("recognises Tauri installers with their architecture", () => {
    expect(classifyAsset("octopusSEO.Log.Analyser_1.2.0_universal.dmg")).toMatchObject({
      kind: "dmg",
      platform: "macos",
      arch: "universal",
    });
    expect(classifyAsset("Clash.Verge_2.5.8_aarch64.dmg")).toMatchObject({ kind: "dmg", arch: "arm64" });
    expect(classifyAsset("App_1.0.0_x64-setup.exe")).toMatchObject({
      kind: "exe",
      platform: "windows",
      arch: "x64",
      offline: false,
    });
    expect(classifyAsset("App_1.0.0_x64_fixed_webview2-setup.exe")).toMatchObject({ kind: "exe", offline: true });
    expect(classifyAsset("App_1.0.0_x64_en-US.msi")).toMatchObject({ kind: "msi", arch: "x64" });
    expect(classifyAsset("App_1.0.0_amd64.AppImage")).toMatchObject({ kind: "appimage", platform: "linux", arch: "x64" });
    expect(classifyAsset("App_1.0.0_arm64.deb")).toMatchObject({ kind: "deb", arch: "arm64" });
    expect(classifyAsset("App-1.0.0-1.armhfp.rpm")).toMatchObject({ kind: "rpm", arch: "arm32" });
    expect(classifyAsset("App-1.0.0.dmg")).toMatchObject({ kind: "dmg", arch: null });
  });

  it("recognises CLI archives by their target triple", () => {
    expect(classifyAsset("snx-log-v0.1.0-aarch64-apple-darwin.tar.gz")).toEqual({
      kind: "cli",
      platform: "macos",
      arch: "arm64",
      offline: false,
      target: "aarch64-apple-darwin",
    });
    expect(classifyAsset("snx-log-v0.1.0-x86_64-pc-windows-msvc.zip")).toMatchObject({
      platform: "windows",
      arch: "x64",
      target: "x86_64-pc-windows-msvc",
    });
    expect(classifyAsset("snx-log-v0.1.0-x86_64-unknown-linux-gnu.tar.gz")).toMatchObject({ platform: "linux" });
  });

  it("skips signatures, updater bundles and manifests", () => {
    for (const name of [
      "App_1.0.0_x64-setup.exe.sig",
      "App_universal.app.tar.gz",
      "App_1.0.0_amd64.AppImage.tar.gz",
      "App_1.0.0_x64-setup.nsis.zip",
      "latest.json",
      "SHA256SUMS",
      "source.tar.gz",
    ]) {
      expect(classifyAsset(name), name).toBeNull();
    }
  });
});

describe("fromManifest", () => {
  it("links collected files below /downloads and drops unknown ones", () => {
    const [release, older] = fromManifest({
      releases: [
        { version: "0.1.0", name: "App 0.1.0", date: "2026-10-01T10:00:00Z", notes: "", assets: [] },
        {
          version: "0.2.0",
          name: "App 0.2.0",
          date: "2026-11-01T10:00:00Z",
          notes: "New",
          assets: [
            { name: "octopusSEO.Log.Analyser_0.2.0_universal.dmg", size: 10, sha256: "ab" },
            { name: "snx-log-v0.2.0-aarch64-apple-darwin.tar.gz", size: 5, sha256: "cd" },
            { name: "notes.txt", size: 1, sha256: "ef" },
          ],
        },
      ],
    });
    expect(release?.version).toBe("0.2.0");
    expect(release?.url).toBeNull();
    expect(release?.assets.map((a) => a.url)).toEqual([
      "/downloads/v0.2.0/octopusSEO.Log.Analyser_0.2.0_universal.dmg",
      "/downloads/v0.2.0/snx-log-v0.2.0-aarch64-apple-darwin.tar.gz",
    ]);
    expect(release?.assets[0]).toMatchObject({ kind: "dmg", arch: "universal", sha256: "ab" });
    expect(older?.version).toBe("0.1.0");
  });
});
