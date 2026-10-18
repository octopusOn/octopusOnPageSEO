import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { en } from "@/i18n/dictionaries/en";
import { archName, downloadChoices, pickInstaller, quarantineCommand, type Installer } from "./installers";
import { products } from "./site";

const installer = (platform: Installer["platform"], kind: Installer["kind"], arch: Installer["arch"], offline = false) => ({
  platform,
  kind,
  arch,
  offline,
  url: `${platform}-${kind}-${arch ?? "any"}${offline ? "-offline" : ""}`,
});

const release = [
  installer("macos", "dmg", "x64"),
  installer("macos", "dmg", "arm64"),
  installer("windows", "msi", "x64"),
  installer("windows", "exe", "arm64"),
  installer("windows", "exe", "x64", true),
  installer("windows", "exe", "x64"),
  installer("linux", "rpm", "x64"),
  installer("linux", "deb", "arm64"),
  installer("linux", "deb", "x64"),
];

describe("pickInstaller", () => {
  it("prefers the platform's usual installer when the CPU is unknown", () => {
    expect(pickInstaller(release, "macos")?.url).toBe("macos-dmg-arm64");
    expect(pickInstaller(release, "windows")?.url).toBe("windows-exe-x64");
    expect(pickInstaller(release, "linux")?.url).toBe("linux-deb-x64");
  });

  it("follows a detected CPU architecture", () => {
    expect(pickInstaller(release, "macos", "x64")?.url).toBe("macos-dmg-x64");
    expect(pickInstaller(release, "windows", "arm64")?.url).toBe("windows-exe-arm64");
    expect(pickInstaller(release, "linux", "arm64")?.url).toBe("linux-deb-arm64");
  });

  it("prefers a universal build and offers nothing that cannot run", () => {
    const mac = [installer("macos", "dmg", "x64"), installer("macos", "dmg", "universal")];
    expect(pickInstaller(mac, "macos")?.url).toBe("macos-dmg-universal");
    expect(pickInstaller([installer("macos", "dmg", "x64")], "macos", "arm64")).toBeUndefined();
    expect(pickInstaller(release, "linux", "arm32")).toBeUndefined();
  });
});

describe("downloadChoices", () => {
  it("drops CLI archives and names architectures per platform", () => {
    const choices = downloadChoices([...release, installer("linux", "cli", "x64")], en.download.arch);
    expect(choices).toHaveLength(release.length);
    expect(choices.find((c) => c.url === "macos-dmg-arm64")?.archLabel).toBe("Apple Silicon");
    expect(choices.find((c) => c.url === "linux-deb-arm64")?.archLabel).toBe("ARM64");
    expect(archName("macos", "x64", en.download.arch)).toBe("Intel");
  });
});

describe("quarantineCommand", () => {
  it("names each program's app bundle as macOS installs it", () => {
    for (const product of products) {
      const config = JSON.parse(readFileSync(new URL(`../../../../${product.app}/src-tauri/tauri.conf.json`, import.meta.url), "utf8"));
      expect(config.productName, product.id).toBe(product.name);
    }
    expect(quarantineCommand("OctopusOn SEO Keyword Research")).toBe('xattr -dr com.apple.quarantine "/Applications/OctopusOn SEO Keyword Research.app"');
  });
});
