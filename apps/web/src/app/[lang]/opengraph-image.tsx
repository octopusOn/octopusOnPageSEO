import { ImageResponse } from "next/og";

import { hasLocale, locales } from "@/i18n/config";
import { getDictionary } from "@/i18n";

export const alt = "octopusSEO Log Analyser";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export function generateStaticParams() {
  return locales.map((lang) => ({ lang }));
}

/** Social card per language: brand, headline and the main promise. */
export default async function Image({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  const dict = getDictionary(hasLocale(lang) ? lang : "en");
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          padding: 72,
          color: "white",
          background: "linear-gradient(135deg, #1e1b4b 0%, #4c1d95 55%, #0f766e 100%)",
          fontFamily: "sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 20, fontSize: 40, fontWeight: 700 }}>
          <svg width="76" height="76" viewBox="0 0 64 64">
            <path
              d="M32 6c-12.2 0-21 8.6-21 20.2 0 6.4 2.7 11.3 6.6 14.6L13 52c-.7 1.6.9 3.2 2.5 2.4l5.9-3.1 2.4 6.3c.6 1.7 3 1.7 3.6 0L32 47l4.6 10.6c.6 1.7 3 1.7 3.6 0l2.4-6.3 5.9 3.1c1.6.8 3.2-.8 2.5-2.4l-4.6-11.2C50.3 37.5 53 32.6 53 26.2 53 14.6 44.2 6 32 6z"
              fill="#a78bfa"
            />
            <circle cx="24.5" cy="25" r="4.4" fill="#fff" />
            <circle cx="39.5" cy="25" r="4.4" fill="#fff" />
            <circle cx="25.4" cy="25.9" r="2.1" fill="#1e1b4b" />
            <circle cx="40.4" cy="25.9" r="2.1" fill="#1e1b4b" />
          </svg>
          octopusSEO
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          <div style={{ fontSize: 64, fontWeight: 800, lineHeight: 1.08, maxWidth: 1000 }}>
            {`${dict.hero.title} ${dict.hero.highlight}`}
          </div>
          <div style={{ fontSize: 30, opacity: 0.85 }}>{dict.hero.note}</div>
        </div>
      </div>
    ),
    size,
  );
}
