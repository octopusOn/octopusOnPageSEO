import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import { Inspector } from "./Inspector";

vi.mock("../api/client", () => ({ api: { detail: vi.fn(), page: vi.fn() }, errorText: String }));

it("offers full incoming and outgoing link exploration for a selected URL", () => {
  const url = "https://example.com/?q=<script>";
  const client = new QueryClient();
  const empty = { columns: [], rows: [], total: 0 };
  client.setQueryData(["detail", "crawl", url], { facts: empty, inlinks: { ...empty, total: 1505 }, outlinks: empty, headers: empty, html: null, raw_html: null });
  const html = renderToStaticMarkup(<QueryClientProvider client={client}><Inspector id="crawl" url={url} onClose={() => {}} onLinks={() => {}} onInspect={() => {}} onDuplicates={() => {}} onIssue={() => {}} /></QueryClientProvider>);
  expect(html).toContain("Gelen bağlantıları listele");
  expect(html).toContain("Giden bağlantıları listele");
  expect(html).toContain("&lt;script&gt;");
  expect(html).not.toContain("<script>");
  client.clear();
});

it("shows four complete counterpart counts and leaves missing older-backend counts unknown", () => {
  const client = new QueryClient();
  const empty = { columns: [], rows: [], total: 0 };
  const props = { id: "crawl", url: "https://example.com/", onClose: () => {}, onLinks: () => {}, onInspect: () => {}, onDuplicates: () => {}, onIssue: () => {} };
  client.setQueryData(["detail", props.id, props.url], { facts: empty, inlinks: empty, outlinks: empty, headers: empty, html: null, raw_html: null, link_counts: { incoming: { internal: 1505, external: 2 }, outgoing: { internal: 3, external: 4 } } });
  const html = renderToStaticMarkup(<QueryClientProvider client={client}><Inspector {...props} /></QueryClientProvider>);
  expect(html).toContain("İç gelen bağlantı<small>1.505</small>");
  expect(html).toContain("Dış gelen bağlantı<small>2</small>");
  expect(html).toContain("İç giden bağlantı<small>3</small>");
  expect(html).toContain("Dış giden bağlantı<small>4</small>");
  client.setQueryData(["detail", props.id, props.url], { facts: empty, inlinks: empty, outlinks: empty, headers: empty, html: null, raw_html: null });
  const legacy = renderToStaticMarkup(<QueryClientProvider client={client}><Inspector {...props} initialTab="externalInlinks" /></QueryClientProvider>);
  expect(legacy).toContain("Dış gelen bağlantı<small>—</small>");
  expect(legacy).toContain("güncel masaüstü arka ucunu gerektirir");
  client.clear();
});

it("opens URL findings with its exact stored count and selected issue explanation",()=>{
  const client=new QueryClient();
  const empty={columns:[],rows:[],total:0};
  client.setQueryData(["detail","crawl","https://example.com/"],{facts:{...empty,total:1},inlinks:empty,outlinks:empty,headers:empty,html:null,raw_html:null,findings:{columns:[{name:"issue_id",kind:"text"}],rows:[["title-missing"]],total:1}});
  const html=renderToStaticMarkup(<QueryClientProvider client={client}><Inspector id="crawl" url="https://example.com/" initialTab="urlFindings" activeIssueId="title-missing" onClose={()=>{}} onLinks={()=>{}} onInspect={()=>{}} onDuplicates={()=>{}} onIssue={()=>{}}/></QueryClientProvider>);
  expect(html).toContain("URL bulguları<small>1</small>");
  expect(html).toContain("Başlık (title) yok");
  expect(html).toContain("Aynı bulguya sahip tüm URL’ler");
  expect(html).toContain("findings-open");
  client.clear();
});
