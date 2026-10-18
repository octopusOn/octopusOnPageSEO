// Renders a CMS page's blocks (apps/cms/app/Cms/Blocks.php defines their fields). Section blocks
// (hero, feature grids, FAQs…) bring their own full-width section; consecutive body blocks
// (text, code, notes, downloads…) share one column.

import Link from "next/link";
import { ArrowRight, BotMessageSquare, CheckCircle2, CircleMinus, CircleX, Info, Lock, Zap } from "lucide-react";
import type { ReactNode } from "react";

import { CopyButton } from "@/components/CopyButton";
import { ProgramIcon } from "@/components/ProgramIcon";
import { Markdown } from "@/components/Markdown";
import { Terminal } from "@/components/Terminal";
import type { Dictionary } from "@/i18n";
import { flag, list, media, text, type Block, type CmsPage, type ProductCard } from "@/lib/cms";
import { findProduct } from "@/lib/site";
import { CmsIcon, hasIcon } from "./icons";
import { CreditCalculator, type CalculatorRow } from "./CreditCalculator";
import { ArticleHeader, ChildrenBlock } from "./Listing";
import { ChangelogBlock, CtaBlock, DownloadsBlock, HeroBlock, PageHeaderBlock, ProductDownloadsBlock } from "./ReleaseBlocks";

import "@/components/products.css";
import "./blocks.css";

export interface BlockContext {
  page: CmsPage;
  lang: string;
  /** BCP 47 tag for dates and numbers: `tr-TR`. */
  tag: string;
  dict: Dictionary;
  /** The page with every download, for download buttons without a release. */
  downloadUrl: string | null;
  /** Every program's page in this language, in menu order. */
  programs: ProductCard[];
  /** The account site (credits, API connections). */
  accountUrl: string;
}

type Data = Record<string, unknown>;

/** Blocks that draw their own full-width section; the others share a column. */
const SECTIONS = new Set(["hero", "page_header", "formats", "feature_grid", "split", "steps", "stats", "terminal", "products", "faq", "cta", "sections", "api_options", "credit_calculator", "comparison"]);

export function Blocks({ ctx }: { ctx: BlockContext }) {
  const { page } = ctx;
  const blocks = [...page.blocks];
  // Categories list their children even without a `children` block.
  if (page.children && !blocks.some((b) => b.type === "children") && (page.children.total > 0 || page.type === "category")) {
    blocks.push({ id: "children", type: "children", data: { title: "", style: page.type === "category" ? "cards" : "list" } });
  }
  const out: ReactNode[] = [];
  // Posts and docs usually start with text: give them a title.
  if (!["hero", "page_header"].includes(page.blocks[0]?.type ?? "")) out.push(<ArticleHeader key="article-head" ctx={ctx} />);
  let column: ReactNode[] = [];
  const close = () => {
    if (column.length) out.push(<div key={`column-${out.length}`} className="container block-stack">{column}</div>);
    column = [];
  };
  blocks.forEach((block, index) => {
    const node = <BlockView key={block.id ?? index} block={block} ctx={ctx} />;
    if (SECTIONS.has(block.type)) {
      close();
      out.push(node);
    } else {
      column.push(node);
    }
  });
  close();
  return <>{out}</>;
}

function BlockView({ block, ctx }: { block: Block; ctx: BlockContext }) {
  const data = block.data;
  switch (block.type) {
    case "hero":
      return <HeroBlock data={data} ctx={ctx} />;
    case "page_header":
      return <PageHeaderBlock data={data} ctx={ctx} />;
    case "formats":
      return <Formats data={data} />;
    case "feature_grid":
      return <FeatureGrid data={data} id={block.id} />;
    case "split":
      return <Split data={data} id={block.id} />;
    case "steps":
      return <Steps data={data} id={block.id} />;
    case "stats":
      return <Stats data={data} />;
    case "terminal":
      return <TerminalSection data={data} id={block.id} />;
    case "products":
      return <Products data={data} cards={block.products ?? []} ctx={ctx} id={block.id} />;
    case "faq":
      return <Faq data={data} id={block.id} />;
    case "cta":
      return <CtaBlock data={data} ctx={ctx} />;
    case "api_options":
      return <ApiOptions data={data} ctx={ctx} id={block.id} />;
    case "credit_calculator":
      return <Calculator data={data} ctx={ctx} id={block.id} />;
    case "comparison":
      return <Comparison data={data} ctx={ctx} id={block.id} />;
    case "sections":
      return <Sections data={data} />;
    case "note":
      return <Note data={data} />;
    case "code":
      return <CodeBlock data={data} ctx={ctx} />;
    case "rich_text":
      return (
        <div className="prose">
          <Markdown text={text(data, "body")} article />
        </div>
      );
    case "image":
      return <Figure data={data} />;
    case "query_examples":
      return <QueryExamples data={data} />;
    case "token_lists":
      return <TokenLists data={data} />;
    case "links":
      return <Links data={data} />;
    case "downloads":
      return <DownloadsBlock data={data} cards={block.products ?? []} ctx={ctx} />;
    case "product_downloads":
      return <ProductDownloadsBlock data={data} ctx={ctx} />;
    case "changelog":
      return <ChangelogBlock ctx={ctx} />;
    case "children":
      return <ChildrenBlock data={data} ctx={ctx} />;
    default:
      return null;
  }
}

/** A stable id for a section heading's `aria-labelledby`. */
const headingId = (id: string | null, suffix: string) => `${id ?? "block"}-${suffix}`;

function SectionHead({ data, id }: { data: Data; id: string }) {
  const eyebrow = text(data, "eyebrow");
  const subtitle = text(data, "subtitle");
  if (!text(data, "title") && !eyebrow) return null;
  return (
    <header className="section-head">
      {eyebrow && <span className="eyebrow">{eyebrow}</span>}
      {text(data, "title") && <h2 id={id}>{text(data, "title")}</h2>}
      {subtitle && <p>{subtitle}</p>}
    </header>
  );
}

function Formats({ data }: { data: Data }) {
  return (
    <section className="formats">
      <div className="container">
        {text(data, "title") && <h2 className="formats-title">{text(data, "title")}</h2>}
        <ul className="format-list">
          {list(data, "items").map((name) => (
            <li key={name}>{name}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}

interface FeatureItem {
  icon: string;
  title: string;
  text: string;
  link: string;
}

function FeatureGrid({ data, id }: { data: Data; id: string | null }) {
  const hid = headingId(id, "title");
  return (
    <section className="section" aria-labelledby={hid}>
      <div className="container">
        <SectionHead data={data} id={hid} />
        <div className="feature-grid">
          {list<FeatureItem>(data, "items").map((item) => {
            const body = (
              <>
                {hasIcon(item.icon) && (
                  <span className="feature-icon">
                    <CmsIcon name={item.icon} />
                  </span>
                )}
                <h3>{item.title}</h3>
                {item.text && <p>{item.text}</p>}
              </>
            );
            return item.link ? (
              <Link key={item.title} href={item.link} className="card feature product-card">
                {body}
              </Link>
            ) : (
              <article key={item.title} className="card feature">
                {body}
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}

const AI_BARS: [string, string, number, string][] = [
  ["GPTBot", "OpenAI", 92, "train"],
  ["OAI-SearchBot", "OpenAI", 64, "search"],
  ["ChatGPT-User", "OpenAI", 48, "assist"],
  ["ClaudeBot", "Anthropic", 71, "train"],
  ["PerplexityBot", "Perplexity", 39, "search"],
  ["Gemini → /pricing", "referral", 27, "visit"],
];

function Split({ data, id }: { data: Data; id: string | null }) {
  const hid = headingId(id, "title");
  const image = media(data, "image");
  const visual = text(data, "visual");
  return (
    <section className={`section ${flag(data, "tinted") ? "section-tinted" : ""}`} aria-labelledby={hid}>
      <div className="container split">
        <div>
          {text(data, "eyebrow") && <span className="eyebrow">{text(data, "eyebrow")}</span>}
          <h2 id={hid}>{text(data, "title")}</h2>
          {text(data, "text") && <p className="lead">{text(data, "text")}</p>}
          <Checks items={list(data, "bullets")} />
        </div>
        {visual === "ai-bots" && (
          <div className="ai-visual" aria-hidden="true">
            {AI_BARS.map(([name, owner, value, type]) => (
              <div key={name} className="ai-row">
                <span className="ai-name">
                  <BotMessageSquare size={16} /> {name}
                  <small>{owner}</small>
                </span>
                <span className="ai-bar">
                  <span className={`ai-fill ${type}`} style={{ width: `${value}%` }} />
                </span>
              </div>
            ))}
          </div>
        )}
        {visual === "image" && image && <Picture file={image} />}
      </div>
    </section>
  );
}

function Checks({ items, card = false }: { items: string[]; card?: boolean }) {
  if (items.length === 0) return null;
  return (
    <ul className={`checks ${card ? "card" : ""}`}>
      {items.map((item) => (
        <li key={item}>
          <CheckCircle2 size={18} aria-hidden="true" /> {item}
        </li>
      ))}
    </ul>
  );
}

function Steps({ data, id }: { data: Data; id: string | null }) {
  const hid = headingId(id, "title");
  return (
    <section className="section" aria-labelledby={hid}>
      <div className="container">
        {text(data, "title") && (
          <header className="section-head">
            <h2 id={hid}>{text(data, "title")}</h2>
          </header>
        )}
        <ol className="steps">
          {list<{ title: string; text: string }>(data, "items").map((step, index) => (
            <li key={step.title} className="card step">
              <span className="step-number">{index + 1}</span>
              <h3>{step.title}</h3>
              {step.text && <p>{step.text}</p>}
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function Stats({ data }: { data: Data }) {
  return (
    <section className="section stats-section">
      <div className="container">
        <div className="stats">
          {list<{ value: string; label: string }>(data, "items").map((item) => (
            <div key={`${item.value}${item.label}`}>
              <b>{item.value}</b>
              <span>{item.label}</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function TerminalSection({ data, id }: { data: Data; id: string | null }) {
  const hid = headingId(id, "title");
  const points = list(data, "points");
  const code = text(data, "code");
  return (
    <section className="section section-dark" aria-labelledby={hid}>
      <div className="container split">
        <div>
          {text(data, "eyebrow") && <span className="eyebrow">{text(data, "eyebrow")}</span>}
          <h2 id={hid}>{text(data, "title")}</h2>
          {text(data, "text") && <p className="lead">{text(data, "text")}</p>}
          {points.length > 0 && (
            <ul className="mini-points">
              {points.map((point, index) => (
                <li key={point}>
                  {index % 2 ? <Lock size={16} aria-hidden="true" /> : <Zap size={16} aria-hidden="true" />} {point}
                </li>
              ))}
            </ul>
          )}
        </div>
        {code ? (
          <pre className="code terminal-code">
            <code>{code}</code>
          </pre>
        ) : (
          <Terminal />
        )}
      </div>
    </section>
  );
}

/** Every program, each card on its own ground with its icon and command-line name. */
function Products({ data, cards, ctx, id }: { data: Data; cards: ProductCard[]; ctx: BlockContext; id: string | null }) {
  const hid = headingId(id, "title");
  return (
    <section className="section" id="programs" aria-labelledby={hid}>
      <div className="container">
        <SectionHead data={data} id={hid} />
        <div className="program-grid">
          {cards.map((card) => {
            const product = findProduct(card.product);
            return (
              <Link key={card.url} href={card.url} className="card program-card product-card" data-product={product?.ground ?? "log"}>
                <span className="program-card-head">
                  <ProgramIcon product={card.product} size={56} />
                  <span className="program-free">{ctx.dict.common.freeOpenSource}</span>
                </span>
                <h3>{card.name}</h3>
                {card.excerpt && <p>{card.excerpt}</p>}
                <span className="product-link">
                  {ctx.dict.common.learnMore} <ArrowRight size={15} aria-hidden="true" />
                </span>
              </Link>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function Faq({ data, id }: { data: Data; id: string | null }) {
  const hid = headingId(id, "title");
  return (
    <section className="section" aria-labelledby={hid}>
      <div className="container narrow">
        {text(data, "title") && (
          <h2 id={hid} className="center">
            {text(data, "title")}
          </h2>
        )}
        <div className="faq">
          {list<{ question: string; answer: string }>(data, "items").map((item) => (
            <details key={item.question}>
              <summary>{item.question}</summary>
              <p>{item.answer}</p>
            </details>
          ))}
        </div>
      </div>
    </section>
  );
}

interface SectionItem {
  anchor: string;
  icon: string;
  title: string;
  text: string;
  bullets: string[];
}

function Sections({ data }: { data: Data }) {
  return (
    <div className="container feature-sections">
      {list<SectionItem>(data, "items").map((section, index) => (
        <section key={section.title} id={section.anchor || undefined} className={`feature-section ${index % 2 ? "reverse" : ""}`}>
          <div>
            {hasIcon(section.icon) && (
              <span className="feature-icon large">
                <CmsIcon name={section.icon} size={28} />
              </span>
            )}
            <h2>{section.title}</h2>
            {section.text && <p className="lead">{section.text}</p>}
          </div>
          <Checks items={section.bullets ?? []} card />
        </section>
      ))}
    </div>
  );
}

function Note({ data }: { data: Data }) {
  return (
    <aside className="card notice product-note">
      {text(data, "title") && (
        <h2>
          <Info size={20} aria-hidden="true" /> {text(data, "title")}
        </h2>
      )}
      {text(data, "text") && <p>{text(data, "text")}</p>}
    </aside>
  );
}

function CodeBlock({ data, ctx }: { data: Data; ctx: BlockContext }) {
  const code = text(data, "code");
  return (
    <section className="block-code">
      {text(data, "title") && <h2>{text(data, "title")}</h2>}
      {text(data, "text") && <p>{text(data, "text")}</p>}
      <div className="code-block">
        <pre className="code">
          <code>{code}</code>
        </pre>
        {flag(data, "copy") && <CopyButton text={code} label={ctx.dict.common.copy} done={ctx.dict.common.copied} />}
      </div>
    </section>
  );
}

function Picture({ file, wide = false }: { file: NonNullable<ReturnType<typeof media>>; wide?: boolean }) {
  return (
    // Images are served by the CMS at their final size.
    // eslint-disable-next-line @next/next/no-img-element
    <img
      className={`block-image ${wide ? "wide" : ""}`}
      src={file.url}
      alt={file.alt ?? ""}
      width={file.width ?? undefined}
      height={file.height ?? undefined}
      loading="lazy"
    />
  );
}

function Figure({ data }: { data: Data }) {
  const file = media(data, "image");
  if (!file) return null;
  return (
    <figure className={`block-figure ${flag(data, "wide") ? "wide" : ""}`}>
      <Picture file={file} wide={flag(data, "wide")} />
      {text(data, "caption") && <figcaption>{text(data, "caption")}</figcaption>}
    </figure>
  );
}

function QueryExamples({ data }: { data: Data }) {
  return (
    <section className="block-docs">
      {text(data, "title") && <h2>{text(data, "title")}</h2>}
      {text(data, "text") && <p>{text(data, "text")}</p>}
      {text(data, "examples_title") && <h3>{text(data, "examples_title")}</h3>}
      <ul className="examples">
        {list<{ query: string; text: string }>(data, "items").map((item) => (
          <li key={item.query} className="card">
            <code>{item.query}</code>
            {item.text && <span>{item.text}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

function TokenLists({ data }: { data: Data }) {
  return (
    <div className="grid-2">
      {list<{ title: string; tokens: string[] }>(data, "items").map((item) => (
        <div key={item.title} className="card">
          <h3>{item.title}</h3>
          <p className="tokens">
            {(item.tokens ?? []).map((token) => (
              <code key={token}>{token}</code>
            ))}
          </p>
        </div>
      ))}
    </div>
  );
}

function Links({ data }: { data: Data }) {
  const items = list<{ label: string; url: string }>(data, "items");
  if (items.length === 0) return null;
  return (
    <p className="links">
      {items.map((item) =>
        item.url.startsWith("/") ? (
          <Link key={item.url} href={item.url}>
            {item.label} →
          </Link>
        ) : (
          <a key={item.url} href={item.url} rel="noopener">
            {item.label} →
          </a>
        ),
      )}
    </p>
  );
}

/** An outside link opens normally; a site link goes through the router. */
function SmartLink({ href, className, children }: { href: string; className?: string; children: ReactNode }) {
  return /^https?:\/\//.test(href) ? (
    <a href={href} className={className} rel="noopener">
      {children}
    </a>
  ) : (
    <Link href={href} className={className}>
      {children}
    </Link>
  );
}

interface ApiOption {
  icon: string;
  badge: string;
  title: string;
  text: string;
  bullets: string[];
  button_label: string;
  link: string;
  highlight: boolean;
}

/** Free, your own API keys, or one OctopusOn connection: the last leads to the account site. */
function ApiOptions({ data, ctx, id }: { data: Data; ctx: BlockContext; id: string | null }) {
  const hid = headingId(id, "title");
  return (
    <section className="section section-tinted" aria-labelledby={hid}>
      <div className="container">
        <SectionHead data={data} id={hid} />
        <div className="api-options">
          {list<ApiOption>(data, "items").map((option) => (
            <article key={option.title} className={`card api-option ${option.highlight ? "highlight" : ""}`}>
              <span className="api-option-head">
                {hasIcon(option.icon) && (
                  <span className="feature-icon">
                    <CmsIcon name={option.icon} />
                  </span>
                )}
                {option.badge && <span className="option-badge">{option.badge}</span>}
              </span>
              <h3>{option.title}</h3>
              {option.text && <p>{option.text}</p>}
              <Checks items={option.bullets ?? []} />
              {option.button_label && (
                <SmartLink href={option.link || ctx.accountUrl} className={`button ${option.highlight ? "button-primary" : "button-ghost"}`}>
                  {option.button_label} <ArrowRight size={16} aria-hidden="true" />
                </SmartLink>
              )}
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

interface ComparisonRow {
  feature: string;
  ours_state: string;
  ours: string;
  theirs_state: string;
  theirs: string;
}

function CompareCell({ state, value, ctx }: { state: string; value: string; ctx: BlockContext }) {
  const labels = ctx.dict.compare;
  const mark =
    state === "yes" ? (
      <CheckCircle2 size={18} className="mark-yes" aria-label={labels.yes} />
    ) : state === "partial" ? (
      <CircleMinus size={18} className="mark-partial" aria-label={labels.partial} />
    ) : state === "no" ? (
      <CircleX size={18} className="mark-no" aria-label={labels.no} />
    ) : null;
  return (
    <td>
      <span className="compare-cell">
        {mark}
        {value && <span>{value}</span>}
      </span>
    </td>
  );
}

/** A feature-by-feature table of one of our programs against another tool. */
function Comparison({ data, ctx, id }: { data: Data; ctx: BlockContext; id: string | null }) {
  const hid = headingId(id, "title");
  const rows = list<ComparisonRow>(data, "rows");
  const note = text(data, "note");
  return (
    <section className="section" aria-labelledby={hid}>
      <div className="container">
        <SectionHead data={data} id={hid} />
        <div className="compare-wrap card">
          <table className="compare">
            <thead>
              <tr>
                <th scope="col">{text(data, "feature_label") || ctx.dict.compare.feature}</th>
                <th scope="col" className="compare-ours">
                  {text(data, "ours")}
                </th>
                <th scope="col">{text(data, "theirs")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.feature}>
                  <th scope="row">{row.feature}</th>
                  <CompareCell state={row.ours_state} value={row.ours} ctx={ctx} />
                  <CompareCell state={row.theirs_state} value={row.theirs} ctx={ctx} />
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {note && <p className="compare-note">{note}</p>}
      </div>
    </section>
  );
}

function Calculator({ data, ctx, id }: { data: Data; ctx: BlockContext; id: string | null }) {
  const hid = headingId(id, "title");
  const rows: CalculatorRow[] = list<Record<string, unknown>>(data, "items").map((item) => ({
    label: String(item.label ?? ""),
    unit: String(item.unit ?? ""),
    credits: Number(item.credits ?? 0) || 0,
    quantity: Number(item.quantity ?? 0) || 0,
    max: Number(item.max ?? 0) || 0,
  }));
  const creditUsd = Number(data.credit_usd ?? 0.001) || 0.001;
  return (
    <section className="section" id="credits" aria-labelledby={hid}>
      <div className="container narrow">
        <header className="section-head">
          {text(data, "eyebrow") && <span className="eyebrow">{text(data, "eyebrow")}</span>}
          <h2 id={hid}>{text(data, "title")}</h2>
          {text(data, "text") && <p>{text(data, "text")}</p>}
        </header>
        <CreditCalculator rows={rows} creditUsd={creditUsd} locale={ctx.tag} labels={ctx.dict.calculator} />
        {text(data, "note") && <p className="calculator-note muted">{text(data, "note")}</p>}
        {text(data, "button_label") && (
          <p className="center">
            <SmartLink href={text(data, "link") || ctx.accountUrl} className="button button-primary button-large">
              {text(data, "button_label")} <ArrowRight size={18} aria-hidden="true" />
            </SmartLink>
          </p>
        )}
      </div>
    </section>
  );
}
