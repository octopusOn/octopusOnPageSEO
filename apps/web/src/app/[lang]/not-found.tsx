import Link from "next/link";
import { lang } from "next/root-params";

import { LogoMark } from "@/components/Logo";
import { defaultLocale, hasLocale } from "@/i18n/config";
import { getDictionary } from "@/i18n";

export default async function NotFound() {
  const current = await lang();
  const locale = hasLocale(current) ? current : defaultLocale;
  const dict = getDictionary(locale);
  return (
    <section className="page-hero not-found">
      {/* Metadata exports are not applied to not-found pages; React hoists this into <head>. */}
      <title>{`${dict.notFound.title} · octopusSEO`}</title>
      <div className="container center">
        <LogoMark size={88} />
        <h1>404 · {dict.notFound.title}</h1>
        <p className="lead">{dict.notFound.text}</p>
        <Link href={`/${locale}`} className="button button-primary">
          {dict.notFound.home}
        </Link>
      </div>
    </section>
  );
}
