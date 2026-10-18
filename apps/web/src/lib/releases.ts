// Releases offered for download: the GitHub releases of the repository, read through the REST
// API and cached for an hour, so new versions appear without a redeploy. Every download link
// leads to GitHub. A site run without GitHub can serve the files that `pnpm --filter
// @octopusseo/web downloads` copied into public/downloads (OCTOPUSSEO_LOCAL_DOWNLOADS=1). Every
// program is released from the same repository; the tag says which (`log-v0.2.0`,
// `spider-v0.2.0`, `serp-v…`, `keywords-v…`, `index-v…`; see products.json).

import { readFile } from "node:fs/promises";
import path from "node:path";

import { parseTag, products, site, type Product, type ProductId } from "./site";

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
  product: ProductId;
  /** Git tag, e.g. `spider-v0.1.0`. */
  tag: string;
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
    /** Missing in manifests written before the site offered several products. */
    product?: ProductId;
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

/** `octopuson-serp-serp-v0.1.0-aarch64-apple-darwin.tar.gz`: any product's command-line archive
 * (also the `snx-*` archives of releases made before the rename). */
const CLI_ARCHIVE = new RegExp(
  `^(?:${[...products.map((product) => product.cli), "snx-(?:log|spider|serp|kw|finder)"].join("|")})-.*?((?:x86_64|aarch64|i686)-[a-z0-9_]+-[a-z0-9_]+(?:-[a-z0-9_]+)?)\\.(?:tar\\.gz|zip)$`,
);

/** Platform, kind and architecture of a release file; `null` for signatures, updater bundles
 * and other files that are not meant to be downloaded by people. */
export function classifyAsset(name: string): Classified | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".sig") || lower.endsWith(".app.tar.gz") || lower.endsWith(".json")) return null;
  const cli = CLI_ARCHIVE.exec(lower);
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

function toRelease(raw: GithubRelease, within?: Product): Release | null {
  const parsed = parseTag(raw.tag_name, within);
  if (!parsed) return null;
  const { product, version } = parsed;
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
    product: product.id,
    tag: raw.tag_name,
    version,
    name: raw.name || raw.tag_name,
    date: raw.published_at ?? "",
    notes: raw.body ?? "",
    url: raw.html_url,
    prerelease: raw.prerelease,
    assets,
  };
}

async function github<T>(path: string, repo: string | null = site.repo): Promise<T | null> {
  if (!site.repo || !repo) return null;
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  const token = process.env.GITHUB_TOKEN?.trim();
  if (token) headers.Authorization = `Bearer ${token}`;
  try {
    const response = await fetch(`https://api.github.com/repos/${repo}${path}`, {
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
    .map((entry) => {
      const product = products.find((item) => item.id === (entry.product ?? "log-analyser")) ?? products[0]!;
      const tag = `${product.tagPrefix}${entry.version}`;
      return {
        product: product.id,
        tag,
        version: entry.version,
        name: entry.name,
        date: entry.date,
        notes: entry.notes,
        url: null,
        prerelease: entry.version.includes("-"),
        assets: entry.assets.flatMap((asset) => {
          const kind = classifyAsset(asset.name);
          if (!kind) return [];
          const url = `${LOCAL_DOWNLOADS}/${encodeURIComponent(tag)}/${encodeURIComponent(asset.name)}`;
          return [{ name: asset.name, url, size: asset.size, downloads: 0, sha256: asset.sha256, ...kind }];
        }),
      };
    })
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

const published = (raw: GithubRelease[] | null, within?: Product) =>
  (raw ?? [])
    .filter((release) => !release.draft && release.published_at)
    .map((release) => toRelease(release, within))
    .filter((release): release is Release => release !== null);

/**
 * Published releases, newest first (drafts excluded), of one product or of all. Each program
 * publishes in its own GitHub project (products.json `repo`); releases made before that, in the
 * source repository, still count.
 */
export async function listReleases(product?: ProductId, limit = 30): Promise<Release[]> {
  const wanted = products.filter((item) => !product || item.id === product);
  const [source, ...own] = await Promise.all([
    github<GithubRelease[]>("/releases?per_page=100"),
    ...wanted.map((item) => github<GithubRelease[]>("/releases?per_page=30", item.repo)),
  ]);
  const byTag = new Map<string, Release>();
  for (const release of [...wanted.flatMap((item, index) => published(own[index] ?? null, item)), ...published(source)]) {
    const key = `${release.product}@${release.version}`;
    if (!byTag.has(key)) byTag.set(key, release);
  }
  const remote = [...byTag.values()].sort((a, b) => b.date.localeCompare(a.date));
  // Downloads come from GitHub. Files copied into public/downloads are offered only when
  // OCTOPUSSEO_LOCAL_DOWNLOADS=1 (a site run without GitHub, see docs/release.md).
  const local = remote.length === 0 && process.env.OCTOPUSSEO_LOCAL_DOWNLOADS?.trim() === "1";
  const all = local ? await localReleases() : remote;
  return all.filter((release) => !product || release.product === product).slice(0, limit);
}

/** The newest stable release of a product with at least one download, if any. */
export async function latestRelease(
  product: ProductId,
): Promise<{ release: Release | null; totalDownloads: number }> {
  const releases = await listReleases(product);
  const release = releases.find((r) => !r.prerelease && r.assets.length > 0) ?? null;
  const totalDownloads = releases.reduce((sum, r) => sum + r.assets.reduce((s, a) => s + a.downloads, 0), 0);
  return { release, totalDownloads };
}

/** Stars of a repository (the source repository by default), or `null` when GitHub cannot be reached or it is missing. */
export async function repoStars(repo: string | null = site.repo): Promise<number | null> {
  const data = await github<{ stargazers_count?: number }>("", repo);
  return typeof data?.stargazers_count === "number" ? data.stargazers_count : null;
}
