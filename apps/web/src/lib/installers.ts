// Which installer suits a visitor best; shared by the server-rendered lists and the
// client-side download button.

import type { Dictionary } from "@/i18n/dictionaries/en";
import type { Arch, AssetKind, Platform } from "./releases";

export interface Installer {
  platform: Platform;
  kind: AssetKind;
  arch: Arch | null;
  offline: boolean;
}

/** Installer types in order of preference per platform. */
const KINDS: Record<Platform, AssetKind[]> = {
  macos: ["dmg"],
  windows: ["exe", "msi"],
  linux: ["appimage", "deb", "rpm"],
};

/** Architectures in order of preference when the visitor's is unknown. */
const ARCHES: Record<Platform, (Arch | null)[]> = {
  macos: ["universal", "arm64", null, "x64"],
  windows: ["x64", null, "universal", "arm64", "arm32"],
  linux: ["x64", null, "universal", "arm64", "arm32"],
};

/** Sort key of an installer within its platform: lower is better. */
export function installerRank(installer: Installer): number {
  const kind = KINDS[installer.platform].indexOf(installer.kind);
  const arch = ARCHES[installer.platform].indexOf(installer.arch);
  return (kind < 0 ? 99 : kind) * 100 + (arch < 0 ? 49 : arch) * 2 + (installer.offline ? 1 : 0);
}

/**
 * The best installer for `platform`; with a known `arch`, only installers that run on it
 * natively are considered.
 */
export function pickInstaller<T extends Installer>(choices: T[], platform: Platform, arch: Arch | null = null): T | undefined {
  return choices
    .filter((c) => c.platform === platform && KINDS[platform].includes(c.kind))
    .filter((c) => !arch || c.arch === arch || c.arch === "universal" || c.arch === null)
    .sort((a, b) => installerRank(a) - installerRank(b))[0];
}

/** Display name of an architecture; Macs are named by chip family. */
export function archName(platform: Platform, arch: Arch, names: Dictionary["download"]["arch"]): string {
  if (platform === "macos" && arch === "arm64") return names.appleSilicon;
  if (platform === "macos" && arch === "x64") return names.intel;
  return names[arch];
}

/** Desktop installers of a release as download-button choices. */
export function downloadChoices(
  assets: (Installer & { url: string })[],
  names: Dictionary["download"]["arch"],
): (Installer & { url: string; archLabel: string | null })[] {
  return assets
    .filter((asset) => asset.kind !== "cli")
    .map(({ platform, kind, arch, offline, url }) => ({
      platform,
      kind,
      arch,
      offline,
      url,
      archLabel: arch ? archName(platform, arch, names) : null,
    }));
}

/**
 * The Terminal command that lets macOS open a program that is not notarized yet: it removes the
 * quarantine flag the browser set on the downloaded app. `app` is the program's name, which is
 * also its bundle name (tauri.conf.json `productName`).
 */
export function quarantineCommand(app: string): string {
  return `xattr -dr com.apple.quarantine "/Applications/${app}.app"`;
}
