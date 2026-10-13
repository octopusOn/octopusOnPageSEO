import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { CopyButton } from "@/components/CopyButton";
import { hasLocale } from "@/i18n/config";
import { getDictionary } from "@/i18n";
import { pageMetadata } from "@/lib/seo";
import { githubUrl } from "@/lib/site";

const QUICKSTART = `snx-log client add "Acme"
snx-log site add acme.com --client "Acme"
snx-log import acme.com /var/log/nginx/
snx-log overview acme.com --period last30d
snx-log query acme.com --template url-summary -o urls.xlsx
snx-log robots acme.com`;

const FIELDS = [
  "date", "ip", "method", "url", "path", "directory", "params", "status", "bytes", "response_ms", "ua",
  "referer", "host", "protocol", "bot", "bot_category", "verification", "content_type", "referer_host",
  "referer_type", "country", "asn", "as_org",
];

const METRICS = [
  "hits", "bytes", "avg_bytes", "avg_ms", "max_ms", "p95_ms", "unique_urls", "unique_ips", "first_seen",
  "last_seen", "days_since_last", "last_status", "status_codes", "unique_statuses", "inconsistent", "errors",
  "hits_1xx…hits_5xx", "co2_mg", "bot_hits", "search_hits", "ai_hits", "googlebot_hits", "bingbot_hits", "yandex_hits",
];

export async function generateMetadata({ params }: PageProps<"/[lang]/docs">): Promise<Metadata> {
  const { lang } = await params;
  if (!hasLocale(lang)) return {};
  const dict = getDictionary(lang);
  return pageMetadata(lang, "/docs", dict.docs.title, dict.docs.subtitle);
}

export default async function Docs({ params }: PageProps<"/[lang]/docs">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  const d = dict.docs;
  const reference = githubUrl("/blob/main/docs/query-language.md");
  const guide = githubUrl("/blob/main/docs/kullanim-kilavuzu.md");
  return (
    <>
      <section className="page-hero">
        <div className="container">
          <h1>{d.title}</h1>
          <p className="lead">{d.subtitle}</p>
        </div>
      </section>
      <div className="container docs">
        <section aria-labelledby="quick-title">
          <h2 id="quick-title">{d.quickstart}</h2>
          <p>{d.quickstartText}</p>
          <div className="code-block">
            <pre className="code">
              <code>{QUICKSTART}</code>
            </pre>
            <CopyButton text={QUICKSTART} label={dict.common.copy} done={dict.common.copied} />
          </div>
        </section>
        <section aria-labelledby="query-title">
          <h2 id="query-title">{d.query}</h2>
          <p>{d.queryText}</p>
          <h3>{d.examples}</h3>
          <ul className="examples">
            {d.exampleItems.map((item) => (
              <li key={item.query} className="card">
                <code>{item.query}</code>
                <span>{item.text}</span>
              </li>
            ))}
          </ul>
          <div className="grid-2">
            <div className="card">
              <h3>{d.fields}</h3>
              <p className="tokens">
                {FIELDS.map((field) => (
                  <code key={field}>{field}</code>
                ))}
              </p>
            </div>
            <div className="card">
              <h3>{d.metrics}</h3>
              <p className="tokens">
                {METRICS.map((metric) => (
                  <code key={metric}>{metric}</code>
                ))}
              </p>
            </div>
          </div>
          {(reference || guide) && (
            <p className="links">
              {reference && (
                <a href={reference} rel="noopener">
                  {d.more} →
                </a>
              )}
              {guide && (
                <a href={guide} rel="noopener">
                  {d.guide} →
                </a>
              )}
            </p>
          )}
        </section>
      </div>
    </>
  );
}
