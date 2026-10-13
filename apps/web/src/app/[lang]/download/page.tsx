import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Apple, Download, Hammer, Terminal as TerminalIcon } from "lucide-react";

import { CopyButton } from "@/components/CopyButton";
import { DownloadButton } from "@/components/DownloadButton";
import { WindowsIcon } from "@/components/icons";
import { hasLocale, localeTags } from "@/i18n/config";
import { getDictionary } from "@/i18n";
import { format, formatBytes, formatDate, formatNumber } from "@/lib/format";
import { archName, downloadChoices, installerRank } from "@/lib/installers";
import { latestRelease, type Platform, type ReleaseAsset } from "@/lib/releases";
import { pageMetadata } from "@/lib/seo";
import { githubUrl } from "@/lib/site";

export const revalidate = 3600;

const PLATFORMS: Platform[] = ["windows", "macos", "linux"];

export async function generateMetadata({ params }: PageProps<"/[lang]/download">): Promise<Metadata> {
  const { lang } = await params;
  if (!hasLocale(lang)) return {};
  const dict = getDictionary(lang);
  return pageMetadata(lang, "/download", dict.download.title, dict.download.subtitle);
}

function PlatformIcon({ platform }: { platform: Platform }) {
  if (platform === "macos") return <Apple size={20} aria-hidden="true" />;
  if (platform === "windows") return <WindowsIcon size={18} />;
  return <TerminalIcon size={20} aria-hidden="true" />;
}

export default async function DownloadPage({ params }: PageProps<"/[lang]/download">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  const d = dict.download;
  const tag = localeTags[lang];
  const { release, totalDownloads } = await latestRelease();
  const releasesUrl = githubUrl("/releases");
  const sourceUrl = githubUrl();
  const desktop = (release?.assets.filter((a) => a.kind !== "cli") ?? []).sort(
    (a, b) => installerRank(a) - installerRank(b),
  );
  const cli = release?.assets.filter((a) => a.kind === "cli") ?? [];
  const label = (asset: ReleaseAsset) => {
    if (asset.kind === "cli") return `${d.kinds.cli} · ${asset.target ?? ""}`;
    const kind = asset.offline ? d.kinds.exeOffline : d.kinds[asset.kind];
    return asset.arch ? `${kind} · ${archName(asset.platform, asset.arch, d.arch)}` : kind;
  };

  return (
    <>
      <section className="page-hero">
        <div className="container">
          <h1>{d.title}</h1>
          <p className="lead">{d.subtitle}</p>
          {release ? (
            <div className="release-head">
              <DownloadButton
                choices={downloadChoices(desktop, d.arch)}
                fallbackHref="#desktop"
                label={dict.hero.primary}
                genericLabel={d.getIt}
                platformNames={d.platforms}
              />
              <p className="muted">
                {format(d.version, { version: release.version })} · {format(d.released, { date: formatDate(release.date, tag) })}
                {totalDownloads > 0 && <> · {format(d.totalDownloads, { count: formatNumber(totalDownloads, tag) })}</>}
              </p>
            </div>
          ) : (
            <div className="card notice">
              <h2>{d.unavailableTitle}</h2>
              <p>{d.unavailableText}</p>
              {releasesUrl && (
                <a className="button button-ghost" href={releasesUrl} rel="noopener">
                  {d.allReleases}
                </a>
              )}
            </div>
          )}
        </div>
      </section>

      <div className="container download-body">
        {release && (
          <>
            <section id="desktop" aria-labelledby="desktop-title">
              <h2 id="desktop-title">{d.desktop}</h2>
              <div className="platforms">
                {PLATFORMS.map((platform) => {
                  const assets = desktop.filter((a) => a.platform === platform);
                  if (assets.length === 0) return null;
                  return (
                    <article key={platform} className="card platform">
                      <h3>
                        <PlatformIcon platform={platform} /> {d.platforms[platform]}
                      </h3>
                      <ul>
                        {assets.map((asset) => (
                          <li key={asset.name}>
                            <a href={asset.url} className="asset">
                              <Download size={16} aria-hidden="true" />
                              <span>
                                <b>{label(asset)}</b>
                                <small>
                                  {asset.name} · {formatBytes(asset.size, tag)}
                                </small>
                              </span>
                            </a>
                            {asset.sha256 && (
                              <code className="sha" title={d.checksum}>
                                {asset.sha256.slice(0, 16)}…
                              </code>
                            )}
                          </li>
                        ))}
                      </ul>
                      <p className="platform-note">{d.platformNotes[platform]}</p>
                    </article>
                  );
                })}
              </div>
            </section>

            {cli.length > 0 && (
              <section aria-labelledby="cli-title">
                <h2 id="cli-title">{d.cli}</h2>
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{d.file}</th>
                        <th>{d.size}</th>
                        <th>{d.checksum}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {cli.map((asset) => (
                        <tr key={asset.name}>
                          <td>
                            <a href={asset.url}>{asset.name}</a>
                          </td>
                          <td>{formatBytes(asset.size, tag)}</td>
                          <td>
                            {asset.sha256 ? (
                              <span className="sha-cell">
                                <code>{asset.sha256.slice(0, 20)}…</code>
                                <CopyButton text={asset.sha256} label={dict.common.copy} done={dict.common.copied} />
                              </span>
                            ) : (
                              "–"
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
            )}
          </>
        )}

        <div className="grid-2">
          <section className="card" aria-labelledby="req-title">
            <h2 id="req-title">{d.requirementsTitle}</h2>
            <ul className="plain">
              {d.requirements.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </section>
          <section className="card" aria-labelledby="source-title">
            <h2 id="source-title">
              <Hammer size={20} aria-hidden="true" /> {d.sourceTitle}
            </h2>
            <p>{d.sourceText}</p>
            <pre className="code">
              <code>{"cargo xtask ci\npnpm install && pnpm desktop:build\ncargo build --release -p snx-log"}</code>
            </pre>
            {sourceUrl && (
              <a href={sourceUrl} rel="noopener">
                {dict.nav.github} →
              </a>
            )}
          </section>
        </div>
      </div>
    </>
  );
}
