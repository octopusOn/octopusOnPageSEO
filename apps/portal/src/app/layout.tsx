import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";

import "@octopusseo/design/index.css";
import "./portal.css";
import { getDictionary } from "@/i18n";

export const dynamic = "force-dynamic";

export async function generateMetadata(): Promise<Metadata> {
  const { t } = await getDictionary();
  return {
    title: { default: t.meta.title, template: `%s · octopusSEO` },
    description: t.meta.description,
    robots: { index: false, follow: false },
  };
}

export const viewport: Viewport = { themeColor: "#1e1b4b" };

export default async function RootLayout({ children }: { children: ReactNode }) {
  const { locale } = await getDictionary();
  return (
    <html lang={locale}>
      <body>{children}</body>
    </html>
  );
}
