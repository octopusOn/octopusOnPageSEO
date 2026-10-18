import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Cell, TablePage } from "../api/types";
import { RenderingComparison } from "./RenderingComparison";

const original = {
  url: "/before", rendered_url: "/after", rendered: true, render_truncated: false,
  raw_title: "Before <script>", title: "After title", raw_meta_description: "Old description", meta_description: "New description",
  raw_canonical: "/old", canonical: "/new", raw_noindex: true, noindex: false, raw_nofollow: true, nofollow: false,
  raw_word_count: 4, word_count: 20, raw_link_count: 1, rendered_link_count: 3,
};
function render(overrides: Record<string, Cell> = {}) {
  const fields = { ...original, ...overrides };
  const facts: TablePage = { columns: Object.keys(fields).map((name) => ({ name, kind: "text" })), rows: [Object.values(fields)], total: 1 };
  return renderToStaticMarkup(<RenderingComparison facts={facts} />);
}
it("compares original and rendered SEO signals while escaping captured content", () => {
  const html = render();
  for (const text of ["İlk yanıt", "JavaScript sonrası", "Old description", "New description", "/old", "/new", "Değişti", "JavaScript ilk yanıttaki noindex"]) expect(html).toContain(text);
  expect(html).toContain("Before &lt;script&gt;");
  expect(html).not.toContain("<script>");
});
it("does not present the raw fallback as a successful rendered result", () => {
  const html = render({ rendered: false, render_error: "Browser failed" });
  expect(html).toContain("Yalnızca ilk yanıt kullanılabilir");
  expect(html).toContain("Browser failed");
  expect(html).toContain("Alınamadı");
  expect(html).not.toContain("After title");
  expect(html).not.toContain("Değişti");
  expect(html).not.toContain("JavaScript ilk yanıttaki noindex");
});
it("keeps legacy metadata unknown while retaining the saved title comparison", () => {
  const html = render({ raw_noindex: null, raw_nofollow: null, raw_meta_description: null, raw_canonical: null });
  expect(html).toContain("Bu eski taramada");
  expect(html.match(/Kaydedilmedi/g)).toHaveLength(4);
  expect(html).toContain("Before &lt;script&gt;");
  expect(html).not.toContain("JavaScript ilk yanıttaki noindex");
});
it("distinguishes absent values from unknown ones and retains HTTP canonical fallback", () => {
  const html = render({ raw_noindex: false, noindex: false, raw_canonical: null, canonical: null, header_canonical: "/http-header", raw_meta_description: null });
  expect(html).not.toContain("Kaydedilmedi");
  expect(html).toContain("/http-header");
  expect(html).toContain("Aynı");
});
it("warns about incomplete DOM without asserting noindex removal", () => {
  const html = render({ render_truncated: true });
  expect(html).toContain("Alınan DOM kesildi");
  expect(html).not.toContain("JavaScript ilk yanıttaki noindex");
});
it("explains when this URL has no JavaScript comparison", () => {
  expect(renderToStaticMarkup(<RenderingComparison facts={{ columns: [], rows: [], total: 0 }} />)).toContain("JavaScript karşılaştırması kaydedilmemiş");
});
