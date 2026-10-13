// Releases offered for download. GitHub releases (read through the REST API and cached for an
// hour, so new versions appear without a redeploy) take precedence; without them the site
// serves the files that `pnpm --filter @octopusseo/web downloads` copied into
// public/downloads.

import { readFile } from "node:fs/promises";
import path from "node:path";

import { site } from "./site";

export type Platform = "macos" | "windows" | "linux";
export type AssetKind = "dmg" | "exe" | "msi" | "appimage" | "deb" | "rpm" | "cli";
export type Arch = "universal" | "arm64" | "x64" | "arm32";

export interface ReleaseAsset {
  name: string;
  url: string;
  size: number;
  downloads: number;
  sha256: string | null;
  kind: AssetKind;
  platform: Platform;
  arch: Arch | null;
  /** Windows installer bundling WebView2 for offline machines. */
  offline: boolean;
  /** CLI target triple, e.g. `aarch64-apple-darwin`. */
  target: string | null;
}

export interface Release {
  version: string;
  name: string;
  date: string;
  notes: string;
  /** Release page on GitHub; `null` for releases hosted by this site. */
  url: string | null;
  prerelease: boolean;
  assets: ReleaseAsset[];
}

/** `public/downloads/releases.json`, written by scripts/collect-downloads.mjs. */
export interface LocalManifest {
  releases: {
    version: string;
    name: string;
    date: string;
    notes: string;
    assets: { name: string; size: number; sha256: string }[];
  }[];
}

/** URL path of the files hosted by this site. */
export const LOCAL_DOWNLOADS = "/downloads";

interface GithubAsset {
  name: string;
  browser_download_url: string;
  size: number;
  download_count: number;
  digest?: string | null;
}

interface GithubRelease {
  tag_name: string;
  name: string | null;
  published_at: string | null;
  body: string | null;
  html_url: string;
  draft: boolean;
  prerelease: boolean;
  assets: GithubAsset[];
}

const REVALIDATE_SECONDS = 3600;

function archOf(lower: string): Arch | null {
  if (lower.includes("universal")) return "universal";
  if (/aarch64|arm64/.test(lower)) return "arm64";
  if (/armhf|armv7/.test(lower)) return "arm32";
  if (/x86_64|x64|amd64/.test(lower)) return "x64";
  return null;
}

type Classified = Pick<ReleaseAsset, "kind" | "platform" | "arch" | "offline" | "target">;

/** Platform, kind and architecture of a release file; `null` for signatures, updater bundles
 * and other files that are not meant to be downloaded by people. */
export function classifyAsset(name: string): Classified | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".sig") || lower.endsWith(".app.tar.gz") || lower.endsWith(".json")) return null;
  const cli = /^snx-log-.*?((?:x86_64|aarch64|i686)-[a-z0-9_]+-[a-z0-9_]+(?:-[a-z0-9_]+)?)\.(?:tar\.gz|zip)$/.exec(lower);
  if (cli) {
    const target = cli[1] ?? "";
    const platform: Platform = target.includes("apple") ? "macos" : target.includes("windows") ? "windows" : "linux";
    return { kind: "cli", platform, arch: archOf(target), offline: false, target };
  }
  const base = { arch: archOf(lower), offline: false, target: null };
  if (lower.endsWith(".dmg")) return { ...base, kind: "dmg", platform: "macos" };
  if (lower.endsWith(".msi")) return { ...base, kind: "msi", platform: "windows" };
  if (lower.endsWith(".exe")) return { ...base, kind: "exe", platform: "windows", offline: lower.includes("webview2") };
  if (lower.endsWith(".appimage")) return { ...base, kind: "appimage", platform: "linux" };
  if (lower.endsWith(".deb")) return { ...base, kind: "deb", platform: "linux" };
  if (lower.endsWith(".rpm")) return { ...base, kind: "rpm", platform: "linux" };
  return null;
}

function toRelease(raw: GithubRelease): Release {
  const assets: ReleaseAsset[] = [];
  for (const asset of raw.assets) {
    const kind = classifyAsset(asset.name);
    if (!kind) continue;
    const digest = asset.digest?.startsWith("sha256:") ? asset.digest.slice(7) : null;
    assets.push({
      name: asset.name,
      url: asset.browser_download_url,
      size: asset.size,
      downloads: asset.download_count,
      sha256: digest,
      ...kind,
    });
  }
  return {
    version: raw.tag_name.replace(/^v/, ""),
    name: raw.name || raw.tag_name,
    date: raw.published_at ?? "",
    notes: raw.body ?? "",
    url: raw.html_url,
    prerelease: raw.prerelease,
    assets,
  };
}

async function github<T>(path: string): Promise<T | null> {
  if (!site.repo) return null;
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  const token = process.env.GITHUB_TOKEN?.trim();
  if (token) headers.Authorization = `Bearer ${token}`;
  try {
    const response = await fetch(`https://api.github.com/repos/${site.repo}${path}`, {
      headers,
      next: { revalidate: REVALIDATE_SECONDS },
      signal: AbortSignal.timeout(8000),
    });
    if (!response.ok) return null;
    return (await response.json()) as T;
  } catch {
    // Offline builds and API outages fall back to the "coming soon" state.
    return null;
  }
}

/** Releases of a local manifest, newest first, with links to the files next to it. */
export function fromManifest(manifest: LocalManifest): Release[] {
  return manifest.releases
    .map((entry) => ({
      version: entry.version,
      name: entry.name,
      date: entry.date,
      notes: entry.notes,
      url: null,
      prerelease: entry.version.includes("-"),
      assets: entry.assets.flatMap((asset) => {
        const kind = classifyAsset(asset.name);
        if (!kind) return [];
        const url = `${LOCAL_DOWNLOADS}/v${encodeURIComponent(entry.version)}/${encodeURIComponent(asset.name)}`;
        return [{ name: asset.name, url, size: asset.size, downloads: 0, sha256: asset.sha256, ...kind }];
      }),
    }))
    .sort((a, b) => b.date.localeCompare(a.date));
}

async function localReleases(): Promise<Release[]> {
  try {
    const file = path.join(process.cwd(), "public", "downloads", "releases.json");
    return fromManifest(JSON.parse(await readFile(file, "utf8")) as LocalManifest);
  } catch {
    // No downloads collected on this machine.
    return [];
  }
}

/** Published releases, newest first (drafts excluded). */
export async function listReleases(limit = 30): Promise<Release[]> {
  const raw = await github<GithubRelease[]>(`/releases?per_page=${limit}`);
  const remote = (raw ?? []).filter((release) => !release.draft && release.published_at).map(toRelease);
  return remote.length > 0 ? remote : (await localReleases()).slice(0, limit);
}

/** The newest stable release with at least one download, if any. */
export async function latestRelease(): Promise<{ release: Release | null; totalDownloads: number }> {
  const releases = await listReleases();
  const release = releases.find((r) => !r.prerelease && r.assets.length > 0) ?? null;
  const totalDownloads = releases.reduce((sum, r) => sum + r.assets.reduce((s, a) => s + a.downloads, 0), 0);
  return { release, totalDownloads };
}
