import { Apple, Download, Terminal as TerminalIcon } from "lucide-react";

import { CopyButton } from "@/components/CopyButton";
import { WindowsIcon } from "@/components/icons";
import type { Dictionary } from "@/i18n";
import { formatBytes } from "@/lib/format";
import { archName, installerRank } from "@/lib/installers";
import type { Platform, Release, ReleaseAsset } from "@/lib/releases";

import "./products.css";

const PLATFORMS: Platform[] = ["windows", "macos", "linux"];

function PlatformIcon({ platform }: { platform: Platform }) {
  if (platform === "macos") return <Apple size={20} aria-hidden="true" />;
  if (platform === "windows") return <WindowsIcon size={18} />;
  return <TerminalIcon size={20} aria-hidden="true" />;
}

/** Desktop installers by platform and command-line archives of one release. */
export function ProductDownloads({
  release,
  dict,
  tag,
  id,
}: {
  release: Release;
  dict: Dictionary;
  /** BCP 47 tag for sizes: `tr-TR`. */
  tag: string;
  /** Prefix of the section anchors, unique on the page. */
  id: string;
}) {
  const d = dict.download;
  const desktop = release.assets.filter((a) => a.kind !== "cli").sort((a, b) => installerRank(a) - installerRank(b));
  const cli = release.assets.filter((a) => a.kind === "cli");
  const label = (asset: ReleaseAsset) => {
    if (asset.kind === "cli") return `${d.kinds.cli} · ${asset.target ?? ""}`;
    const kind = asset.offline ? d.kinds.exeOffline : d.kinds[asset.kind];
    return asset.arch ? `${kind} · ${archName(asset.platform, asset.arch, d.arch)}` : kind;
  };
  return (
    <>
      {desktop.length > 0 && (
        <section id={`${id}-desktop`} aria-labelledby={`${id}-desktop-title`}>
          <h3 id={`${id}-desktop-title`} className="downloads-title">
            {d.desktop}
          </h3>
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
      )}

      {cli.length > 0 && (
        <section aria-labelledby={`${id}-cli-title`}>
          <h3 id={`${id}-cli-title`} className="downloads-title">
            {d.cli}
          </h3>
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
  );
}
