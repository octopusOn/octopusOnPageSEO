// Child pages of a category (a blog, the docs, a product group), with page links at
// `<category>/page/2`; and the header of pages that start without one (posts, docs).

import Link from "next/link";
import { ArrowLeft, ArrowRight } from "lucide-react";

import { format, formatDate } from "@/lib/format";
import { text } from "@/lib/cms";
import type { BlockContext } from "./Blocks";
import { CmsIcon, hasIcon } from "./icons";
import { Breadcrumbs } from "./ReleaseBlocks";

export function ChildrenBlock({ data, ctx }: { data: Record<string, unknown>; ctx: BlockContext }) {
  const listing = ctx.page.children;
  const { dict } = ctx;
  if (!listing) return null;
  const onlyHeader = ctx.page.blocks.every((block) => ["page_header", "hero", "children"].includes(block.type));
  if (listing.total === 0) {
    return onlyHeader ? <p className="card notice">{dict.listing.empty}</p> : null;
  }
  const pageUrl = (n: number) => (n <= 1 ? listing.base_url : `${listing.base_url}/page/${n}`);
  const style = text(data, "style") === "list" ? "list" : "cards";
  return (
    <section className="listing-section" aria-label={text(data, "title") || ctx.page.title}>
      {text(data, "title") && <h2>{text(data, "title")}</h2>}
      <ul className={`listing ${style}`}>
        {listing.items.map((item) => (
          <li key={item.url}>
            <Link href={item.url} className="card listing-card">
              {item.image && style === "cards" && (
                // eslint-disable-next-line @next/next/no-img-element
                <img src={item.image.url} alt="" width={item.image.width ?? undefined} height={item.image.height ?? undefined} loading="lazy" />
              )}
              <span className="listing-title">
                {hasIcon(item.icon) && <CmsIcon name={item.icon} size={18} />} {item.title}
              </span>
              {item.excerpt && <span className="listing-excerpt">{item.excerpt}</span>}
              <span className="listing-meta">
                {item.type === "post" && item.published_at && <time dateTime={item.published_at}>{formatDate(item.published_at, ctx.tag)}</time>}
                <span className="product-link">
                  {dict.listing.readMore} <ArrowRight size={15} aria-hidden="true" />
                </span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
      {listing.last_page > 1 && (
        <nav className="pagination" aria-label={format(dict.listing.page, { page: listing.page, pages: listing.last_page })}>
          {listing.page > 1 ? (
            <Link href={pageUrl(listing.page - 1)} rel="prev" className="button button-ghost">
              <ArrowLeft size={16} aria-hidden="true" /> {dict.listing.newer}
            </Link>
          ) : (
            <span />
          )}
          <span className="muted">{format(dict.listing.page, { page: listing.page, pages: listing.last_page })}</span>
          {listing.page < listing.last_page ? (
            <Link href={pageUrl(listing.page + 1)} rel="next" className="button button-ghost">
              {dict.listing.older} <ArrowRight size={16} aria-hidden="true" />
            </Link>
          ) : (
            <span />
          )}
        </nav>
      )}
    </section>
  );
}

/** Title, summary and date of a page whose first block is not a header (posts, docs). */
export function ArticleHeader({ ctx }: { ctx: BlockContext }) {
  const { page } = ctx;
  return (
    <header className="container article-head">
      <Breadcrumbs ctx={ctx} />
      <h1>{page.title}</h1>
      {page.excerpt && <p className="lead">{page.excerpt}</p>}
      {page.type === "post" && page.published_at && (
        <p className="muted">
          <time dateTime={page.published_at}>{formatDate(page.published_at, ctx.tag)}</time>
        </p>
      )}
    </header>
  );
}
