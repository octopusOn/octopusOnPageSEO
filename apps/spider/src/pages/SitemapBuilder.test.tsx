import {renderToStaticMarkup} from "react-dom/server";
import {expect,it,vi} from "vitest";
vi.mock("../api/client",()=>({api:{},errorText:String,isDemo:false}));
import {SitemapReport} from "./SitemapBuilder";
import type {SitemapPlan} from "../api/types";
const plan:SitemapPlan={origin:"https://example.com",origins:[],scanned:864,included:180,excluded:[{reason:"external",count:598},{reason:"rendering",count:72},{reason:"noindex",count:14}],sample:['https://example.com/?q=<script>&other="x"'],parts:1,xml_bytes:9123,warnings:[]};
it("shows honest included/excluded totals, exact reasons and a safely escaped bounded preview",()=>{
  const html=renderToStaticMarkup(<SitemapReport plan={plan}/>);
  expect(html).toContain("180");expect(html).toContain("684");expect(html).toContain("1 / 180 URL");
  expect(html).toContain("Harici URL");expect(html).toContain("Başarısız veya doğrulanmamış JS");
  expect(html).toContain("&lt;script&gt;");expect(html).not.toContain("<script>");
  expect(html).toContain("İlk 100 uygun URL");
});
it("explains incomplete and list-based coverage without claiming a full site inventory",()=>{
  const html=renderToStaticMarkup(<SitemapReport plan={{...plan,warnings:["partial","list-mode"]}}/>);
  expect(html).toContain("Keşfedilmeyen sayfalar");expect(html).toContain("sitenin tamamını kapsamayabilir");
});
it("handles a finished archive with no eligible URLs or recorded URLs",()=>{
  const html=renderToStaticMarkup(<SitemapReport plan={{...plan,scanned:0,included:0,parts:0,sample:[],excluded:[],xml_bytes:0}}/>);
  expect(html).toContain("Site haritası ölçütlerini karşılayan URL yok");expect(html).not.toMatch(/NaN|Infinity/);
});
