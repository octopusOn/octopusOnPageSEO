import { renderToStaticMarkup } from "react-dom/server";
import { expect,it } from "vitest";
import { defaultConfig, type CrawlMeta, type Job } from "../api/types";
import { Sessions } from "./Sessions";
const crawl:CrawlMeta={id:"saved",name:"Example <script>",start_urls:["https://example.com/"],created_at_ms:0,finished_at_ms:null,status:"running",progress:{crawled:7,queued:3,errors:0,elapsed_ms:1000,discovered:10,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:defaultConfig};
const render=(jobs:Job[],jobsStatus:"ready"|"loading"|"unavailable")=>renderToStaticMarkup(<Sessions crawls={[crawl]} jobs={jobs} jobsStatus={jobsStatus} onOpen={()=>{}} onRepeat={()=>{}} onDelete={async()=>{}}/>);
it("shows an interrupted record without rewriting it and offers a separate new crawl",()=>{
  const html=render([],"ready");
  expect(html).toContain("Bu oturumda etkin iş yok"); expect(html).toContain("Yeni tarama olarak tekrarla"); expect(html).toMatch(/kuyruk/i);
  expect(html).toContain("Example &lt;script&gt;"); expect(html).not.toContain("<script>");
  expect(html).toMatch(/disabled=""[^>]*aria-label="Sil"/);
  expect(crawl.status).toBe("running");
});
it("does not diagnose interruption while the jobs service is unknown",()=>{
  expect(render([],"loading")).toContain("Canlı durum doğrulanıyor");
  const html=render([],"unavailable");
  expect(html).toContain("Canlı durum alınamadı"); expect(html).not.toContain("Bu oturumda etkin iş yok");
  expect(html).toMatch(/session-repeat[^>]*disabled=""/);
});
it("keeps a finalizing crawl active and disables a second launch from its saved card",()=>{
  const job:Job={id:"active",crawl_id:crawl.id,url:crawl.start_urls[0]!,status:"running",phase:"finalizing",progress:crawl.progress};
  const html=render([job],"ready");
  expect(html).toContain("Analiz ediliyor ve kaydediliyor");
  expect(html).not.toContain("Bu oturumda etkin iş yok");
  expect(html).toMatch(/session-repeat[^>]*disabled=""/);
});

it("shows measured total and per-crawl file sizes while excluding deleted snapshot entries", () => {
  const html = renderToStaticMarkup(<Sessions crawls={[crawl]} jobs={[]} jobsStatus="ready" onOpen={()=>{}} onRepeat={()=>{}} onDelete={async()=>{}}
    storageStatus="ready" storage={[{id:crawl.id,bytes:1_250_000},{id:"deleted",bytes:9_000_000_000}]} onRefreshStorage={()=>{}} />);
  expect(html).toContain("Kayıtlı tarama dosyaları");
  expect(html.match(/1,3 MB/g)).toHaveLength(2);
  expect(html).not.toContain("9 GB");
  expect(html).toContain("Dosya boyutu");
  expect(html).toContain("En büyük önce");
  expect(html).toMatch(/disabled=""[^>]*aria-label="Sil"/);
});

it("labels partial measurements and does not reuse cached sizes after a measurement failure", () => {
  const props = {crawls:[crawl,{...crawl,id:"unknown"}],jobs:[],jobsStatus:"ready" as const,onOpen:()=>{},onRepeat:()=>{},onDelete:async()=>{},storage:[{id:crawl.id,bytes:1000} ]};
  const partial = renderToStaticMarkup(<Sessions {...props} storageStatus="ready" />);
  expect(partial).toContain("Bilinen dosya boyutu");
  expect(partial).toContain("1 taramanın boyutu alınamadı");
  expect(partial).not.toContain("0 B");
  const failed = renderToStaticMarkup(<Sessions {...props} storageStatus="unavailable" />);
  expect(failed).toContain("Dosya boyutları ölçülemedi");
  expect(failed).not.toContain("1 kB");
  expect(failed).not.toContain("0 B");
});

it("distinguishes an empty measured library from unavailable demo or pending sizes", () => {
  const props = {jobs:[],jobsStatus:"ready" as const,onOpen:()=>{},onRepeat:()=>{},onDelete:async()=>{}};
  expect(renderToStaticMarkup(<Sessions {...props} crawls={[]} storageStatus="ready" storage={[]}/>)).toContain("0 B");
  const unknown = renderToStaticMarkup(<Sessions {...props} crawls={[crawl]} storageStatus="ready" storage={[{id:crawl.id,bytes:null}]}/>);
  expect(unknown).not.toContain("0 B");
  expect(unknown).toContain("1 taramanın boyutu alınamadı");
  const pending = renderToStaticMarkup(<Sessions {...props} crawls={[crawl]}/>);
  expect(pending).toContain("Dosya boyutları ölçülüyor");
  expect(pending).not.toContain("0 B");
});
