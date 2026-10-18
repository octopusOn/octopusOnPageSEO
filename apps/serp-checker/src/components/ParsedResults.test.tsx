import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Organic } from "../api/types";
import { ParsedResults } from "./ParsedResults";

const result = (position: number, url: string, siteOnly = false): Organic => ({
  position,
  page: 1,
  url,
  host: new URL(url).host,
  domain: new URL(url).host.replace(/^www\./, ""),
  title: `Sonuç ${position}`,
  displayed_url: `${url} › ...`,
  ...(siteOnly ? { site_only: true } : {}),
});

it("marks results whose page address the engine hid", () => {
  const html = renderToStaticMarkup(
    <ParsedResults
      organic={[result(1, "https://www.karaca.com/", true), result(2, "https://www.trendyol.com/kahve")]}
      features={[]}
    />,
  );
  expect(html.match(/Yalnızca site/g)).toHaveLength(1);
  expect(html).toContain("adresini gizledi");
});
