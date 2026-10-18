import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import type { ContentMatch as Match } from "../api/types";
import { ContentMatch } from "./ContentMatch";

vi.mock("../api/client", () => ({ api: { contentMatch: vi.fn() }, errorText: String }));

function render(match: Match | null) {
  const client = new QueryClient();
  client.setQueryData(["content-match", "crawl", "/selected"], match);
  const html = renderToStaticMarkup(<QueryClientProvider client={client}><ContentMatch id="crawl" url="/selected" onInspect={() => {}} /></QueryClientProvider>);
  client.clear();
  return html;
}

it("shows an actionable observed example with bit distance instead of a similarity percentage", () => {
  const html = render({url:"/selected",match_url:"/example?q=<script>",match_title:"Example <script>",distance:3,words:200,match_words:250});
  for (const text of ["Benzer içerik örneği", "3 / 64", "200", "250", "Örnek URL’yi incele", "benzerlik yüzdesi değildir", "tek bir örnek"]) expect(html).toContain(text);
  expect(html).toContain("/example?q=&lt;script&gt;");
  expect(html).not.toContain("<script>");
  expect(html).not.toContain("95%");
});

it("keeps no-match results inconclusive and explains the eligibility criteria", () => {
  const html = render(null);
  expect(html).toContain("benzersiz olduğunu kanıtlamaz");
  expect(html).toContain("En az 200 görünür kelimesi");
  expect(html).toContain("Başarısız JS işlemleri ve kesilmiş içerikler");
  expect(html).not.toContain("Örnek URL’yi incele");
});
