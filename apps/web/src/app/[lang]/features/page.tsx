import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Bot, CheckCircle2, Database, Gauge, Link2, ShieldX, Sparkles } from "lucide-react";
import type { ComponentType } from "react";

import { hasLocale } from "@/i18n/config";
import { getDictionary, type Dictionary } from "@/i18n";
import { pageMetadata } from "@/lib/seo";

type SectionKey = keyof Dictionary["featuresPage"]["sections"];

const ICONS: Record<SectionKey, ComponentType<{ size?: number }>> = {
  bots: Bot,
  ai: Sparkles,
  robots: ShieldX,
  crawl: Gauge,
  urls: Link2,
  data: Database,
};

export async function generateMetadata({ params }: PageProps<"/[lang]/features">): Promise<Metadata> {
  const { lang } = await params;
  if (!hasLocale(lang)) return {};
  const dict = getDictionary(lang);
  return pageMetadata(lang, "/features", dict.featuresPage.title, dict.featuresPage.subtitle);
}

export default async function Features({ params }: PageProps<"/[lang]/features">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  const page = dict.featuresPage;
  return (
    <>
      <section className="page-hero">
        <div className="container">
          <h1>{page.title}</h1>
          <p className="lead">{page.subtitle}</p>
          <nav className="chips" aria-label={page.title}>
            {(Object.keys(page.sections) as SectionKey[]).map((key) => (
              <a key={key} href={`#${key}`} className="chip">
                {page.sections[key].title}
              </a>
            ))}
          </nav>
        </div>
      </section>
      <div className="container feature-sections">
        {(Object.keys(page.sections) as SectionKey[]).map((key, index) => {
          const section = page.sections[key];
          const Icon = ICONS[key];
          return (
            <section key={key} id={key} className={`feature-section ${index % 2 ? "reverse" : ""}`}>
              <div>
                <span className="feature-icon large">
                  <Icon size={28} />
                </span>
                <h2>{section.title}</h2>
                <p className="lead">{section.text}</p>
              </div>
              <ul className="checks card">
                {section.bullets.map((bullet) => (
                  <li key={bullet}>
                    <CheckCircle2 size={18} aria-hidden="true" /> {bullet}
                  </li>
                ))}
              </ul>
            </section>
          );
        })}
      </div>
      <section className="cta">
        <div className="container cta-box">
          <h2>{dict.cta.title}</h2>
          <p>{dict.cta.text}</p>
          <Link href={`/${lang}/download`} className="button button-primary button-large">
            {dict.cta.button}
          </Link>
        </div>
      </section>
    </>
  );
}
