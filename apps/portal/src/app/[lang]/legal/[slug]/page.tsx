// A legal text from the CMS, with the seller's details filled in. Asked for by its key or by
// another language's slug, it redirects to its slug in the page's language.

import Link from "next/link";
import { notFound, redirect } from "next/navigation";

import { BrandMark } from "@/components/BrandMark";
import { LanguageMenu } from "@/components/LanguageMenu";
import { Markdown } from "@/components/Markdown";
import { fill, getDictionary } from "@/i18n";
import { messagesFor } from "@/i18n/catalog";
import { languages } from "@/i18n/languages";
import { routeUrl } from "@/i18n/routes";
import { legalMarkdown, legalText, legalTexts } from "@/legal/cms";
import { company, fillCompany, legalApproved, legalLanguage } from "@/legal/company";

type Props = { params: Promise<{ slug: string }> };

export async function generateMetadata({ params }: Props) {
  const [{ slug }, { locale }] = await Promise.all([params, getDictionary()]);
  const text = await legalText(locale, slug);
  return text ? { title: text.title } : {};
}

export default async function Legal({ params }: Props) {
  const [{ slug }, { t, locale, tag, url, languages: offered }] = await Promise.all([params, getDictionary()]);
  const [text, list, { fallback }] = await Promise.all([legalText(locale, slug), legalTexts(locale), languages()]);
  if (text === null) notFound();
  if (text && text.slug !== slug && (text.locale === locale || slug === text.key)) redirect(url("legal/[slug]", { slug: text.slug }));

  // The same text in every language: its slug there, else its key (which redirects).
  const hrefs = text
    ? Object.fromEntries(
        offered.map(({ code }) => [
          code,
          routeUrl(code, messagesFor(code, fallback).routes, "legal/[slug]", {
            slug: text.alternates.find((alternate) => alternate.locale === code)?.slug ?? text.key,
          }),
        ]),
      )
    : undefined;

  return (
    <div className="oc-ground legal-ground">
      <article className="legal">
        <Link href={url("")} className="oc-brand legal-brand">
          <BrandMark />
          <span className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </span>
        </Link>
        {text ? (
          <>
            {!legalApproved() ? <div className="oc-banner warning">{t.legal.draft}</div> : null}
            {text.locale !== locale ? <div className="oc-banner info">{t.legal.fallback}</div> : null}
            {text.locale !== legalLanguage() ? <div className="oc-banner info">{t.legal.binding}</div> : null}
            <h1 lang={text.locale}>{text.title}</h1>
            <div className="legal-body" lang={text.locale}>
              <Markdown text={fillCompany(legalMarkdown(text), company(), t.legal.missing)} article />
            </div>
            {text.updated_at ? (
              <p className="muted small">
                {fill(t.legal.updated, { date: new Intl.DateTimeFormat(tag, { dateStyle: "long" }).format(new Date(text.updated_at)) })}
              </p>
            ) : null}
          </>
        ) : (
          <div className="oc-banner warning">{t.legal.unavailable}</div>
        )}
        <nav className="legal-nav small" aria-label={t.legal.title}>
          {list.map((item) => (
            <Link key={item.key} href={url("legal/[slug]", { slug: item.slug })} aria-current={item.key === text?.key ? "page" : undefined}>
              {item.label}
            </Link>
          ))}
          <LanguageMenu hrefs={hrefs} className="up" />
        </nav>
      </article>
    </div>
  );
}
