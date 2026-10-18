import type { Metadata, Viewport } from "next";
import { draftMode } from "next/headers";
import { notFound } from "next/navigation";

import "@octopusseo/design/tokens.css";
import "../globals.css";
import "@/components/blocks/blocks.css";
import { Footer } from "@/components/Footer";
import { Header } from "@/components/Header";
import { ProgramIconDefs } from "@/components/ProgramIcon";
import { fallbackLanguages, LANGUAGE_CODE } from "@/i18n/config";
import { getDictionary } from "@/i18n";
import { getLanguages, getSite } from "@/lib/cms";
import { site } from "@/lib/site";

/** The CMS's languages; a language added later renders on its first visit. */
export async function generateStaticParams() {
  try {
    const languages = await getLanguages();
    return (languages.length ? languages : fallbackLanguages).map((language) => ({ lang: language.code }));
  } catch {
    return fallbackLanguages.map((language) => ({ lang: language.code }));
  }
}

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f5f5f7" },
    { media: "(prefers-color-scheme: dark)", color: "#0a0a0c" },
  ],
};

export const metadata: Metadata = {
  metadataBase: new URL(site.url),
  title: { default: site.name, template: `%s · ${site.name}` },
  applicationName: site.name,
  creator: site.name,
};

export default async function RootLayout({ children, params }: LayoutProps<"/[lang]">) {
  const { lang } = await params;
  if (!LANGUAGE_CODE.test(lang)) notFound();
  const [cms, preview] = await Promise.all([getSite(lang), draftMode()]);
  if (!cms.languages.some((language) => language.code === lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <html lang={lang}>
      <body>
        <ProgramIconDefs />
        <a className="skip" href="#main">
          {dict.nav.skip}
        </a>
        {preview.isEnabled && (
          <form className="preview-banner" method="post" action="/api/preview">
            <span>{dict.preview.banner}</span>
            <button type="submit">{dict.preview.exit}</button>
          </form>
        )}
        <Header lang={lang} dict={dict} navigation={cms.navigation} languages={cms.languages} />
        <main id="main">{children}</main>
        <Footer lang={lang} dict={dict} navigation={cms.navigation} />
      </body>
    </html>
  );
}
