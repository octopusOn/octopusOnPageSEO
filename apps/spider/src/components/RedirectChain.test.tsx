import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import { RedirectChain } from "./RedirectChain";
import type { RedirectTrace } from "../api/types";

vi.mock("../api/client", () => ({ api: { redirects: vi.fn() }, errorText: String }));

function render(trace: RedirectTrace) {
  const client = new QueryClient();
  client.setQueryData(["redirects", "crawl", "start"], trace);
  const html = renderToStaticMarkup(<QueryClientProvider client={client}><RedirectChain id="crawl" url="start" /></QueryClientProvider>);
  client.clear();
  return html;
}
const hop = (url: string, status: number | null, location: string | null = null) => ({ url, status, location, error: null, blocked: false, recorded: true });

it("shows ordered HTTP hops and the final error without executing stored content", () => {
  const html = render({ outcome: "http-error", hops: [hop("/a", 301, "/b"), hop("/b", 302, "/missing?<script>"), hop("/missing?<script>", 404)] });
  expect(html).toContain("Zincir bir HTTP hatasıyla sona eriyor.");
  expect(html).toContain("301");
  expect(html.indexOf("/a")).toBeLessThan(html.indexOf("/b"));
  expect(html).toContain("&lt;script&gt;");
  expect(html).not.toContain("<script>");
});

it("keeps unchecked and truncated traces inconclusive", () => {
  const unknown = render({ outcome: "unchecked", hops: [hop("/a", 301, "/unfetched"), { ...hop("/unfetched", null), recorded: false }] });
  expect(unknown).toContain("son sonuç bilinmiyor");
  expect(unknown).toContain("Bu URL taramada kaydedilmemiş.");
  expect(unknown).not.toContain("redirect-error");
  const limited = render({ outcome: "limit", hops: [hop("/99", 301, "/100")] });
  expect(limited).toContain("zincirin devamı bilinmiyor");
  expect(limited).toContain("/100");
  expect(limited).not.toContain("kaydedilmiş bir yanıtla sona eriyor");
});

it("explains cycles and request failures", () => {
  expect(render({ outcome: "loop", hops: [hop("/a", 301, "/a"), hop("/a", 301, "/a")] })).toContain("Yönlendirme döngüsü");
  expect(render({ outcome: "request-error", hops: [{ ...hop("/offline", null), error: "connect: refused" }] })).toContain("connect: refused");
});
