import { describe, expect, it } from "vitest";

import { classifyAsset, fromManifest } from "./releases";
import { parseTag, productById, productOfTag } from "./site";

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

  it("recognises the command-line archives of every product", () => {
    expect(classifyAsset("snx-spider-spider-v0.1.0-x86_64-pc-windows-msvc.zip")).toMatchObject({
      kind: "cli",
      target: "x86_64-pc-windows-msvc",
    });
    expect(classifyAsset("snx-serp-serp-v0.1.0-aarch64-apple-darwin.tar.gz")).toMatchObject({
      kind: "cli",
      platform: "macos",
      target: "aarch64-apple-darwin",
    });
    expect(classifyAsset("snx-other-v0.1.0-aarch64-apple-darwin.tar.gz")).toBeNull();
    expect(classifyAsset("octopuson-keywords-keywords-v0.2.0-aarch64-apple-darwin.tar.gz")).toMatchObject({
      kind: "cli",
      target: "aarch64-apple-darwin",
    });
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

describe("productOfTag", () => {
  it("tells products apart by the longest tag prefix", () => {
    expect(productOfTag("v0.2.0")?.id).toBe("log-analyser");
    expect(productOfTag("log-v0.2.0")?.id).toBe("log-analyser");
    expect(productOfTag("keywords-v0.1.0")?.id).toBe("seo-keyword-research");
    expect(productOfTag("kw-v0.1.0")?.id).toBe("seo-keyword-research");
    expect(productOfTag("index-v0.1.0")?.id).toBe("index-checker");
    expect(productOfTag("crawler-v0.2.0")?.id).toBe("seo-crawler");
    expect(productOfTag("spider-v0.1.0")?.id).toBe("seo-crawler");
    expect(productOfTag("serp-v1.0.0-beta.1")?.id).toBe("serp-checker");
    for (const tag of ["spider-0.1.0", "vnext", "serp-v", "finder-v0.1.0"]) expect(productOfTag(tag), tag).toBeNull();
  });

  it("reads plain version tags in a program's own repository", () => {
    const crawler = productById("seo-crawler");
    expect(parseTag("v0.3.0", crawler)).toEqual({ product: crawler, version: "0.3.0" });
    expect(parseTag("crawler-v0.3.0", crawler)?.version).toBe("0.3.0");
    expect(parseTag("nightly", crawler)).toBeNull();
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
      "/downloads/log-v0.2.0/octopusSEO.Log.Analyser_0.2.0_universal.dmg",
      "/downloads/log-v0.2.0/snx-log-v0.2.0-aarch64-apple-darwin.tar.gz",
    ]);
    expect(release?.assets[0]).toMatchObject({ kind: "dmg", arch: "universal", sha256: "ab" });
    expect(older?.version).toBe("0.1.0");
  });

  it("keeps every product's files in a folder named after its tag", () => {
    const [serp, log] = fromManifest({
      releases: [
        { version: "0.1.0", name: "octopusSEO Log Analyser 0.1.0", date: "2026-10-01T10:00:00Z", notes: "", assets: [] },
        {
          product: "serp-checker",
          version: "0.1.0",
          name: "octopusSEO SERP Finder 0.1.0",
          date: "2026-10-18T10:00:00Z",
          notes: "",
          assets: [{ name: "octopusSEO.SERP.Finder_0.1.0_universal.dmg", size: 10, sha256: "ab" }],
        },
      ],
    });
    expect(serp).toMatchObject({ product: "serp-checker", tag: "serp-v0.1.0", version: "0.1.0" });
    expect(serp?.assets[0]?.url).toBe("/downloads/serp-v0.1.0/octopusSEO.SERP.Finder_0.1.0_universal.dmg");
    expect(log).toMatchObject({ product: "log-analyser", tag: "log-v0.1.0" });
  });
});
