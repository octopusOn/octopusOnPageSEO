// Blocks that show releases from GitHub (or the files collected for this site): download buttons,
// download tables and the changelog. The CMS gives their texts; releases come from lib/releases.

import Link from "next/link";
import { ArrowRight, Hammer, Sparkles } from "lucide-react";

import { DownloadButton } from "@/components/DownloadButton";
import { Markdown } from "@/components/Markdown";
import { ProductDownloads } from "@/components/ProductDownloads";
import { AppMockup } from "@/components/AppMockup";
import { GithubIcon } from "@/components/icons";
import { ProgramIcon } from "@/components/ProgramIcon";
import { ProgramShowcase } from "./ProgramShowcase";
import { format, formatDate, formatNumber } from "@/lib/format";
import { downloadChoices } from "@/lib/installers";
import { latestRelease, listReleases, type Release } from "@/lib/releases";
import { findProduct, githubUrl, products } from "@/lib/site";
import { flag, list, media, text, type ProductCard } from "@/lib/cms";
import type { BlockContext } from "./Blocks";

type Data = Record<string, unknown>;

const external = (href: string) => /^https?:\/\//.test(href);

async function releaseOf(product: string | null | undefined): Promise<Release | null> {
  const known = findProduct(product);
  return known ? (await latestRelease(known.id)).release : null;
}

/** The OS-aware download button of a program's latest release. */
function Download({ release, ctx, label, generic, fallback }: { release: Release | null; ctx: BlockContext; label: string; generic: string; fallback: string }) {
  const d = ctx.dict.download;
  return (
    <DownloadButton
      choices={downloadChoices(release?.assets ?? [], d.arch)}
      fallbackHref={fallback}
      label={label}
      genericLabel={generic}
      platformNames={d.platforms}
    />
  );
}

export async function HeroBlock({ data, ctx }: { data: Data; ctx: BlockContext }) {
  const release = await releaseOf(text(data, "product"));
  const badge = text(data, "badge");
  const secondary = text(data, "secondary_label");
  const image = media(data, "image");
  const visual = text(data, "visual");
  return (
    <section className="hero">
      <div className="hero-glow" aria-hidden="true" />
      <div className="container hero-grid">
        <div className="hero-copy">
          {badge && (
            <a href={text(data, "badge_link") || "#main"} className="badge" rel={external(text(data, "badge_link")) ? "noopener" : undefined}>
              <Sparkles size={14} aria-hidden="true" /> {badge} <ArrowRight size={14} aria-hidden="true" />
            </a>
          )}
          <h1>
            {text(data, "title")} {text(data, "highlight") && <span className="gradient-text">{text(data, "highlight")}</span>}
          </h1>
          {text(data, "subtitle") && <p className="lead">{text(data, "subtitle")}</p>}
          <div className="actions">
            {text(data, "product") ? (
              <Download
                release={release}
                ctx={ctx}
                label={ctx.dict.download.forOs}
                generic={text(data, "download_label") || ctx.dict.nav.download}
                fallback={ctx.downloadUrl ?? `/${ctx.lang}`}
              />
            ) : (
              text(data, "primary_link") && (
                <Link href={text(data, "primary_link")} className="button button-primary button-large">
                  {text(data, "download_label") || ctx.dict.nav.download}
                </Link>
              )
            )}
            {secondary && (
              <a href={text(data, "secondary_link") || "#main"} className="button button-ghost button-large" rel={external(text(data, "secondary_link")) ? "noopener" : undefined}>
                {external(text(data, "secondary_link")) && <GithubIcon size={18} />} {secondary} <ArrowRight size={18} aria-hidden="true" />
              </a>
            )}
          </div>
          {text(data, "note") && <p className="hero-note">{text(data, "note")}</p>}
        </div>
        {visual === "log-mockup" && <AppMockup dict={ctx.dict} />}
        {visual === "screenshot" && (
          <ProgramShowcase
            name={findProduct(text(data, "product") || ctx.page.product)?.name ?? ctx.page.title}
            image={image}
            commands={text(data, "code")}
            cli={findProduct(text(data, "product") || ctx.page.product)?.cli ?? ""}
            labels={ctx.dict.showcase}
          />
        )}
        {visual === "programs" && (
          <nav className="program-wall" aria-label={ctx.dict.nav.programs}>
            {ctx.programs.map((program) => (
              <Link key={program.url} href={program.url}>
                <ProgramIcon product={program.product} size={64} />
                {program.name}
              </Link>
            ))}
          </nav>
        )}
        {visual === "image" && image && (
          // eslint-disable-next-line @next/next/no-img-element
          <img className="block-image" src={image.url} alt={image.alt ?? ""} width={image.width ?? undefined} height={image.height ?? undefined} />
        )}
      </div>
    </section>
  );
}

/** Anchors the page offers: feature sections with a name and the products of a downloads block. */
function anchors(ctx: BlockContext): { href: string; label: string }[] {
  return ctx.page.blocks.flatMap((block) => {
    if (block.type === "sections") {
      return list<{ anchor: string; title: string }>(block.data, "items")
        .filter((item) => item.anchor)
        .map((item) => ({ href: `#${item.anchor}`, label: item.title }));
    }
    if (block.type === "downloads") {
      return (block.products ?? []).filter((card) => findProduct(card.product)).map((card) => ({ href: `#${card.product}`, label: card.name }));
    }
    return [];
  });
}

export async function PageHeaderBlock({ data, ctx }: { data: Data; ctx: BlockContext }) {
  const { page, dict } = ctx;
  const product = flag(data, "download") ? findProduct(page.product) : null;
  const release = product ? (await latestRelease(product.id)).release : null;
  const chips = flag(data, "anchors") ? anchors(ctx) : [];
  const releasesUrl = githubUrl("/releases");
  return (
    <section className="page-hero">
      <div className="container">
        <Breadcrumbs ctx={ctx} />
        {page.product && <ProgramIcon product={page.product} size={84} />}
        <h1>{text(data, "title") || page.title}</h1>
        {text(data, "subtitle") && <p className="lead">{text(data, "subtitle")}</p>}
        {chips.length > 0 && (
          <nav className="chips" aria-label={page.title}>
            {chips.map((chip) => (
              <a key={chip.href} href={chip.href} className="chip">
                {chip.label}
              </a>
            ))}
          </nav>
        )}
        {product &&
          (release ? (
            <div className="release-head">
              <Download
                release={release}
                ctx={ctx}
                label={format(dict.productPage.download, { product: product.short })}
                generic={dict.download.getIt}
                fallback={`#${product.id}-desktop`}
              />
              <p className="muted">
                {format(dict.download.version, { version: release.version })} ·{" "}
                {format(dict.download.released, { date: formatDate(release.date, ctx.tag) })}
                {release.url && (
                  <>
                    {" · "}
                    <a href={release.url} rel="noopener" className="release-github">
                      {dict.download.onGithub} →
                    </a>
                  </>
                )}
              </p>
            </div>
          ) : (
            <div className="card notice">
              <h2>{dict.productPage.notReleasedTitle}</h2>
              <p>{dict.productPage.notReleasedText}</p>
              {releasesUrl && (
                <a className="button button-ghost" href={releasesUrl} rel="noopener">
                  {dict.download.allReleases}
                </a>
              )}
            </div>
          ))}
      </div>
    </section>
  );
}

/** The trail above deeper pages' titles; top-level pages skip it. */
export function Breadcrumbs({ ctx }: { ctx: BlockContext }) {
  const crumbs = ctx.page.breadcrumbs;
  if (crumbs.length < 3) return null;
  return (
    <nav className="breadcrumbs" aria-label={ctx.dict.nav.breadcrumb}>
      <ol>
        {crumbs.map((crumb, index) => (
          <li key={crumb.url}>
            {index < crumbs.length - 1 ? <Link href={crumb.url}>{crumb.title}</Link> : <span aria-current="page">{crumb.title}</span>}
          </li>
        ))}
      </ol>
    </nav>
  );
}

export async function CtaBlock({ data, ctx }: { data: Data; ctx: BlockContext }) {
  const download = text(data, "action") === "download";
  const release = download ? await releaseOf(text(data, "product")) : null;
  const label = text(data, "button_label") || ctx.dict.nav.download;
  return (
    <section className="cta">
      <div className="container cta-box">
        <h2>{text(data, "title")}</h2>
        {text(data, "text") && <p>{text(data, "text")}</p>}
        {download ? (
          <Download release={release} ctx={ctx} label={ctx.dict.download.forOs} generic={label} fallback={ctx.downloadUrl ?? `/${ctx.lang}`} />
        ) : (
          text(data, "link") && (
            <Link href={text(data, "link")} className="button button-primary button-large">
              {label}
            </Link>
          )
        )}
      </div>
    </section>
  );
}

/** Every program's latest release, then system requirements and building from source. */
export async function DownloadsBlock({ data, cards, ctx }: { data: Data; cards: ProductCard[]; ctx: BlockContext }) {
  const { dict, tag } = ctx;
  const d = dict.download;
  const shown = products.map((product) => ({ product, card: cards.find((c) => c.product === product.id) }));
  const releases = await Promise.all(shown.map(async (entry) => ({ ...entry, ...(await latestRelease(entry.product.id)) })));
  const releasesUrl = githubUrl("/releases");
  const sourceUrl = githubUrl();
  const requirements = list(data, "requirements");
  return (
    <div className="download-body">
      {releases.map(({ product, card, release, totalDownloads }) => (
        <section key={product.id} id={product.id} className="product-downloads" aria-labelledby={`${product.id}-title`}>
          <header className="product-downloads-head">
            <div className="product-downloads-name">
              <ProgramIcon product={product.id} size={52} />
              <h2 id={`${product.id}-title`}>{product.name}</h2>
              {card?.excerpt && <p className="muted">{card.excerpt}</p>}
            </div>
            {release && (
              <div className="release-head">
                <Download release={release} ctx={ctx} label={d.forOs} generic={d.getIt} fallback={`#${product.id}-desktop`} />
                <p className="muted">
                  {format(d.version, { version: release.version })} · {format(d.released, { date: formatDate(release.date, tag) })}
                  {totalDownloads > 0 && <> · {format(d.totalDownloads, { count: formatNumber(totalDownloads, tag) })}</>}
                  {release.url && (
                    <>
                      {" · "}
                      <a href={release.url} rel="noopener" className="release-github">
                        {d.onGithub} →
                      </a>
                    </>
                  )}
                </p>
              </div>
            )}
          </header>
          {release ? (
            <ProductDownloads release={release} dict={dict} tag={tag} id={product.id} app={product.name} />
          ) : (
            <div className="card notice">
              <h3>{dict.productPage.notReleasedTitle}</h3>
              <p>{dict.productPage.notReleasedText}</p>
              <div className="actions">
                {card && (
                  <Link className="button button-ghost" href={card.url}>
                    {dict.common.learnMore}
                  </Link>
                )}
                {releasesUrl && (
                  <a className="button button-ghost" href={releasesUrl} rel="noopener">
                    {d.allReleases}
                  </a>
                )}
              </div>
            </div>
          )}
        </section>
      ))}

      {(requirements.length > 0 || text(data, "source_title")) && (
        <div className="grid-2">
          {requirements.length > 0 && (
            <section className="card">
              <h2>{text(data, "requirements_title")}</h2>
              <ul className="plain">
                {requirements.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </section>
          )}
          {text(data, "source_title") && (
            <section className="card">
              <h2>
                <Hammer size={20} aria-hidden="true" /> {text(data, "source_title")}
              </h2>
              {text(data, "source_text") && <p>{text(data, "source_text")}</p>}
              {text(data, "source_code") && (
                <pre className="code">
                  <code>{text(data, "source_code")}</code>
                </pre>
              )}
              {sourceUrl && (
                <a href={sourceUrl} rel="noopener">
                  {dict.nav.github} →
                </a>
              )}
            </section>
          )}
        </div>
      )}
    </div>
  );
}

/** The page's program: every installer and command-line archive of its latest release. */
export async function ProductDownloadsBlock({ data, ctx }: { data: Data; ctx: BlockContext }) {
  const product = findProduct(ctx.page.product);
  const release = product ? (await latestRelease(product.id)).release : null;
  const link = text(data, "link");
  return (
    <>
      {product && release && (
        <section aria-label={text(data, "link_label") || product.name}>
          <ProductDownloads release={release} dict={ctx.dict} tag={ctx.tag} id={product.id} app={product.name} />
        </section>
      )}
      {(link || release?.url) && (
        <p className="links">
          {release?.url && (
            <a href={release.url} rel="noopener">
              {ctx.dict.download.onGithub} →
            </a>
          )}
          {link && <Link href={link}>{text(data, "link_label") || link} →</Link>}
        </p>
      )}
    </>
  );
}

export async function ChangelogBlock({ ctx }: { ctx: BlockContext }) {
  const releases = await listReleases();
  return (
    <div className="timeline">
      {releases.length === 0 && <p className="card notice">{ctx.dict.changelog.empty}</p>}
      {releases.map((release) => (
        <article key={release.tag} className="release">
          <header>
            <h2>{release.name}</h2>
            <time dateTime={release.date}>{formatDate(release.date, ctx.tag)}</time>
          </header>
          <Markdown text={release.notes} />
          {release.url && (
            <a href={release.url} rel="noopener">
              {ctx.dict.changelog.viewOnGithub} →
            </a>
          )}
        </article>
      ))}
    </div>
  );
}
