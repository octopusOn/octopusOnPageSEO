import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { Markdown } from "@/components/Markdown";
import { hasLocale, localeTags } from "@/i18n/config";
import { getDictionary } from "@/i18n";
import { formatDate } from "@/lib/format";
import { listReleases } from "@/lib/releases";
import { pageMetadata } from "@/lib/seo";

export const revalidate = 3600;

export async function generateMetadata({ params }: PageProps<"/[lang]/changelog">): Promise<Metadata> {
  const { lang } = await params;
  if (!hasLocale(lang)) return {};
  const dict = getDictionary(lang);
  return pageMetadata(lang, "/changelog", dict.changelog.title, dict.changelog.subtitle);
}

export default async function Changelog({ params }: PageProps<"/[lang]/changelog">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  const releases = await listReleases();
  return (
    <>
      <section className="page-hero">
        <div className="container">
          <h1>{dict.changelog.title}</h1>
          <p className="lead">{dict.changelog.subtitle}</p>
        </div>
      </section>
      <div className="container narrow timeline">
        {releases.length === 0 && <p className="card notice">{dict.changelog.empty}</p>}
        {releases.map((release) => (
          <article key={release.version} className="release">
            <header>
              <h2>{release.name}</h2>
              <time dateTime={release.date}>{formatDate(release.date, localeTags[lang])}</time>
            </header>
            <Markdown text={release.notes} />
            {release.url && (
              <a href={release.url} rel="noopener">
                {dict.changelog.viewOnGithub} →
              </a>
            )}
          </article>
        ))}
      </div>
    </>
  );
}
