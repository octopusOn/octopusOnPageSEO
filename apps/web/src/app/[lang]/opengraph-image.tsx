import { ImageResponse } from "next/og";

import { fallbackLanguages } from "@/i18n/config";
import { getLanguages, resolvePage } from "@/lib/cms";

export const alt = "OctopusOn SEO";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export async function generateStaticParams() {
  try {
    return (await getLanguages()).map((language) => ({ lang: language.code }));
  } catch {
    return fallbackLanguages.map((language) => ({ lang: language.code }));
  }
}

/** Social card per language: brand, the home page's title and description from the CMS. */
export default async function Image({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  const home = await resolvePage(lang, "").catch(() => null);
  const page = home && "page" in home ? home.page : null;
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
          color: "#f5f5f7",
          background: "#0a0a0c",
          fontFamily: "sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 20, fontSize: 40, fontWeight: 700 }}>
          <svg width="76" height="76" viewBox="0 0 64 64">
            <path d="M31.95 3.93 L46.21 12.70 L46.21 28.72 L60.73 37.39 L49.34 43.59 L34.02 34.57 L34.07 18.85 L20.96 10.63 Z" fill="#eef1f6" />
            <path d="M17.69 12.39 L29.83 19.80 L29.83 34.67 L13.65 44.25 L13.71 59.01 L1.81 51.66 L1.86 38.05 L17.69 28.47 Z" fill="#eef1f6" />
            <path d="M31.95 38.10 L48.48 47.98 L62.19 40.27 L62.19 51.66 L48.08 60.07 L31.90 50.80 L16.28 59.62 L16.33 47.27 Z" fill="#eef1f6" />
          </svg>
          OctopusOn SEO
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          <div style={{ fontSize: 64, fontWeight: 700, lineHeight: 1.08, letterSpacing: -2, maxWidth: 1000 }}>
            {page?.seo.title ?? "OctopusOn SEO"}
          </div>
          <div style={{ fontSize: 30, color: "#a1a1a6" }}>{page?.seo.description ?? ""}</div>
        </div>
      </div>
    ),
    size,
  );
}
