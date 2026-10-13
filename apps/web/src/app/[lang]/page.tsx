import Link from "next/link";
import { notFound } from "next/navigation";
import {
  ArrowRight,
  Bell,
  BotMessageSquare,
  ChartColumnBig,
  CheckCircle2,
  FileSearch,
  Gauge,
  Globe2,
  Lock,
  ShieldCheck,
  ShieldX,
  Sparkles,
  Zap,
} from "lucide-react";
import type { ComponentType } from "react";

import { AppMockup } from "@/components/AppMockup";
import { DownloadButton } from "@/components/DownloadButton";
import { Terminal } from "@/components/Terminal";
import { hasLocale } from "@/i18n/config";
import { getDictionary, type Dictionary } from "@/i18n";
import { downloadChoices } from "@/lib/installers";
import { latestRelease } from "@/lib/releases";
import { site } from "@/lib/site";

export const revalidate = 3600;

const FORMATS = ["Nginx", "Apache", "IIS", "CloudFront", "AWS ALB", "HAProxy", "Traefik", "Caddy", "Cloudflare", "Google Cloud", "JSON", "CSV"];

const FEATURE_ICONS: Record<keyof Dictionary["features"]["items"], ComponentType<{ size?: number }>> = {
  verification: ShieldCheck,
  ai: Sparkles,
  robots: ShieldX,
  budget: Gauge,
  orphans: FileSearch,
  reports: ChartColumnBig,
  geo: Globe2,
  alerts: Bell,
};

export default async function Home({ params }: PageProps<"/[lang]">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  const { release } = await latestRelease();
  const choices = downloadChoices(release?.assets ?? [], dict.download.arch);
  const downloadButton = (label: string) => (
    <DownloadButton
      choices={choices}
      fallbackHref={`/${lang}/download`}
      label={dict.hero.primary}
      genericLabel={label}
      platformNames={dict.download.platforms}
    />
  );
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: site.product,
    applicationCategory: "BusinessApplication",
    operatingSystem: "Windows, macOS, Linux",
    description: dict.meta.description,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    publisher: { "@type": "Organization", name: site.name, url: site.url },
    ...(release ? { softwareVersion: release.version, downloadUrl: `${site.url}/${lang}/download` } : {}),
  };

  return (
    <>
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd).replace(/</g, "\\u003c") }} />
      <section className="hero">
        <div className="hero-glow" aria-hidden="true" />
        <div className="container hero-grid">
          <div className="hero-copy">
            <Link href={`/${lang}/features`} className="badge">
              <Sparkles size={14} aria-hidden="true" /> {dict.hero.badge} <ArrowRight size={14} aria-hidden="true" />
            </Link>
            <h1>
              {dict.hero.title} <span className="gradient-text">{dict.hero.highlight}</span>
            </h1>
            <p className="lead">{dict.hero.subtitle}</p>
            <div className="actions">
              {downloadButton(dict.hero.primaryGeneric)}
              <Link href={`/${lang}/features`} className="button button-ghost button-large">
                {dict.hero.secondary} <ArrowRight size={18} aria-hidden="true" />
              </Link>
            </div>
            <p className="hero-note">{dict.hero.note}</p>
          </div>
          <AppMockup dict={dict} />
        </div>
      </section>

      <section className="formats" aria-labelledby="formats-title">
        <div className="container">
          <h2 id="formats-title" className="formats-title">
            {dict.formats.title}
          </h2>
          <ul className="format-list">
            {FORMATS.map((name) => (
              <li key={name}>{name}</li>
            ))}
          </ul>
        </div>
      </section>

      <section className="section" aria-labelledby="features-title">
        <div className="container">
          <header className="section-head">
            <span className="eyebrow">{dict.features.eyebrow}</span>
            <h2 id="features-title">{dict.features.title}</h2>
            <p>{dict.features.subtitle}</p>
          </header>
          <div className="feature-grid">
            {(Object.keys(dict.features.items) as (keyof Dictionary["features"]["items"])[]).map((key) => {
              const Icon = FEATURE_ICONS[key];
              const item = dict.features.items[key];
              return (
                <article key={key} className="card feature">
                  <span className="feature-icon">
                    <Icon size={22} />
                  </span>
                  <h3>{item.title}</h3>
                  <p>{item.text}</p>
                </article>
              );
            })}
          </div>
        </div>
      </section>

      <section className="section section-tinted" aria-labelledby="ai-title">
        <div className="container split">
          <div>
            <span className="eyebrow">{dict.ai.eyebrow}</span>
            <h2 id="ai-title">{dict.ai.title}</h2>
            <p className="lead">{dict.ai.text}</p>
            <ul className="checks">
              {dict.ai.bullets.map((bullet) => (
                <li key={bullet}>
                  <CheckCircle2 size={18} aria-hidden="true" /> {bullet}
                </li>
              ))}
            </ul>
          </div>
          <div className="ai-visual" aria-hidden="true">
            {[
              ["GPTBot", "OpenAI", 92, "train"],
              ["OAI-SearchBot", "OpenAI", 64, "search"],
              ["ChatGPT-User", "OpenAI", 48, "assist"],
              ["ClaudeBot", "Anthropic", 71, "train"],
              ["PerplexityBot", "Perplexity", 39, "search"],
              ["Gemini → /pricing", "referral", 27, "visit"],
            ].map(([name, owner, value, type]) => (
              <div key={name as string} className="ai-row">
                <span className="ai-name">
                  <BotMessageSquare size={16} /> {name}
                  <small>{owner}</small>
                </span>
                <span className="ai-bar">
                  <span className={`ai-fill ${type as string}`} style={{ width: `${value as number}%` }} />
                </span>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="section" aria-labelledby="steps-title">
        <div className="container">
          <header className="section-head">
            <h2 id="steps-title">{dict.steps.title}</h2>
          </header>
          <ol className="steps">
            {dict.steps.items.map((step, index) => (
              <li key={step.title} className="card step">
                <span className="step-number">{index + 1}</span>
                <h3>{step.title}</h3>
                <p>{step.text}</p>
              </li>
            ))}
          </ol>
          <div className="stats">
            <div>
              <b>110+</b>
              <span>{dict.stats.bots}</span>
            </div>
            <div>
              <b>12+</b>
              <span>{dict.stats.formats}</span>
            </div>
            <div>
              <b>39</b>
              <span>{dict.stats.reports}</span>
            </div>
            <div>
              <b>750K</b>
              <span>{dict.stats.speed}</span>
            </div>
          </div>
        </div>
      </section>

      <section className="section section-dark" aria-labelledby="cli-title">
        <div className="container split">
          <div>
            <span className="eyebrow">{dict.terminal.eyebrow}</span>
            <h2 id="cli-title">{dict.terminal.title}</h2>
            <p className="lead">{dict.terminal.text}</p>
            <ul className="mini-points">
              <li>
                <Zap size={16} aria-hidden="true" /> {dict.terminal.points[0]}
              </li>
              <li>
                <Lock size={16} aria-hidden="true" /> {dict.terminal.points[1]}
              </li>
            </ul>
          </div>
          <Terminal />
        </div>
      </section>

      <section className="section" aria-labelledby="faq-title">
        <div className="container narrow">
          <h2 id="faq-title" className="center">
            {dict.faq.title}
          </h2>
          <div className="faq">
            {dict.faq.items.map((item) => (
              <details key={item.q}>
                <summary>{item.q}</summary>
                <p>{item.a}</p>
              </details>
            ))}
          </div>
        </div>
      </section>

      <section className="cta">
        <div className="container cta-box">
          <h2>{dict.cta.title}</h2>
          <p>{dict.cta.text}</p>
          {downloadButton(dict.cta.button)}
        </div>
      </section>
    </>
  );
}
