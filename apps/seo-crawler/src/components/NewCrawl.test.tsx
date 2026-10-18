import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import { defaultConfig } from "../api/types";
import { NewCrawl } from "./NewCrawl";
import { RepeatSettings } from "./RepeatSettings";

vi.mock("../api/client", () => ({ isDemo: false }));

it("previews a rendering retry as a URL list with scope and new-record explanations", () => {
  const html = renderToStaticMarkup(<NewCrawl
    config={{ ...defaultConfig, mode: "list", rendering: { ...defaultConfig.rendering, mode: "javascript" } }}
    initialUrl={"https://example.com/a\nhttps://example.com/b?x=<script>"}
    retryRendering onClose={() => {}} onStart={async () => {}}
  />);
  expect(html).toContain("JS hatalarını yeniden tara");
  expect(html).toContain("<textarea");
  expect(html).toContain("yeni bir tarama olarak kaydedilecek");
  expect(html).toContain("hreflang ilişkileri eksik kalabilir");
  expect(html).toContain("&lt;script&gt;");
  expect(html).not.toContain("<script>");
});

it("keeps the normal new-crawl flow in spider mode", () => {
  const html = renderToStaticMarkup(<NewCrawl config={defaultConfig} initialUrl="https://example.com/"
    onClose={() => {}} onStart={async () => {}} />);
  expect(html).not.toContain("<textarea");
  expect(html).not.toContain("Kaynak taramanın ayarları");
});
it("shows a single initial SEO Crawler address instead of concatenating a multiline seed draft", () => {
  const html = renderToStaticMarkup(<NewCrawl config={defaultConfig}
    initialUrl={"\r\nhttps://example.com/a\rhttps://example.com/b"}
    onClose={() => {}} onStart={async () => {}} />);
  expect(html).toContain('value="https://example.com/a"');
  expect(html).not.toContain("https://example.com/b");
});
it("preserves saved list mode, browser settings and a reviewable source description",()=>{
  const config={...defaultConfig,mode:"list" as const,start_urls:["https://example.com/a","https://example.com/b"],rendering:{...defaultConfig.rendering,mode:"javascript" as const,browser_path:"/Custom <script> Browser"},limits:{max_urls:50,max_depth:0,timeout_seconds:15}};
  const html=renderToStaticMarkup(<NewCrawl config={config} initialUrl={config.start_urls.join("\n")} repeatSource={{name:"Source <script>",created_at_ms:0}} onClose={()=>{}} onStart={async()=>{}}/>);
  expect(html).toContain("<textarea"); expect(html).toContain('value="javascript" selected=""');
  expect(html).toContain("önceki kuyruk sürdürülmez"); expect(html).toContain("Aktarılan ayarları incele");
  expect(html).toContain("Source &lt;script&gt;"); expect(html).toContain("/Custom &lt;script&gt; Browser");
  expect(html).not.toContain("<script>");
});
it("retains multiple saved SEO Crawler seeds instead of collapsing them to one URL",()=>{
  const config={...defaultConfig,start_urls:["https://example.com/a","https://example.com/b"]};
  const html=renderToStaticMarkup(<NewCrawl config={config} initialUrl={config.start_urls.join("\n")} repeatSource={{name:"Source",created_at_ms:0}} onClose={()=>{}} onStart={async()=>{}}/>);
  expect(html).toContain("Başlangıç URL’leri"); expect(html).toContain("<textarea"); expect(html).toContain("https://example.com/b");
});
it("distinguishes unlimited limits from a zero-depth seed-only crawl",()=>{
  const unlimited=renderToStaticMarkup(<RepeatSettings config={{...defaultConfig,speed:{...defaultConfig.speed,max_requests_per_second:0}}}/>);
  expect(unlimited).toContain("<dt>Saniyede istek</dt><dd>Sınırsız</dd>");
  expect(unlimited).toContain("<dt>Maksimum derinlik</dt><dd>Sınırsız</dd>");
  const seedOnly=renderToStaticMarkup(<RepeatSettings config={{...defaultConfig,limits:{...defaultConfig.limits,max_depth:0}}}/>);
  expect(seedOnly).toContain("<dt>Maksimum derinlik</dt><dd>0</dd>");
});

it("lets a repeated JavaScript crawl review its saved custom viewport before starting", () => {
  const config={...defaultConfig,rendering:{...defaultConfig.rendering,mode:"javascript" as const,viewport:{width:768,height:1024,mobile:true}}};
  const html=renderToStaticMarkup(<NewCrawl config={config} initialUrl="https://example.com/" repeatSource={{name:"Source",created_at_ms:0}} onClose={()=>{}} onStart={async()=>{}}/>);
  expect(html).toContain('value="custom" selected=""');
  expect(html).toContain('value="768"');
  expect(html).toContain('value="1024"');
  expect(html).toContain("768 × 1024 CSS px · Mobil");
});
