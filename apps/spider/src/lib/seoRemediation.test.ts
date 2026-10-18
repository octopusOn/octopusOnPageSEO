import { expect, it } from "vitest";
import type { Cell, RemediationDetail, TablePage } from "../api/types";
import { issueById } from "../catalog";
import { solutionGuide } from "./seoRemediation";
const table=(values:Record<string,Cell>):TablePage=>({columns:Object.keys(values).map(name=>({name,kind:"text"})),rows:[Object.values(values)],total:1});
export const evidence=(facts:Record<string,Cell>,witness:Record<string,Cell>={}):RemediationDetail=>({urls:table({address:"https://example.com/tr/page"}),selected_url:"https://example.com/tr/page",facts:table({url:"https://example.com/tr/page",status:200,indexability:"indexable",...facts}),evidence:table(witness),evidence_kind:"facts"});

it("uses recorded hreflang witnesses and flags intentionally canonicalized source pages",()=>{
 const guide=solutionGuide(issueById.get("hreflang-missing-return")!,evidence({lang:"tr",indexability:"canonicalised",canonical:"https://example.com/tr/primary"},{lang:"en",target:"https://example.com/en/page",return_link:false}),"tr");
 expect(guide.before).toContain("https://example.com/en/page");expect(guide.before).toContain("Hayır");
 expect(guide.after).toBeNull();
 expect(guide.warning).toContain("canonicalised");expect(guide.warning).toContain("olduğu gibi eklemeyin");
 expect(guide.steps.join(" ")).toContain("tam kümeye");
});

it("explains distinct pagination versus a canonical duplicate without demanding translations",()=>{
 const guide=solutionGuide(issueById.get("canonicalised")!,evidence({canonical:"https://example.com/tr/services",pagination_candidate:true,canonical_content_equal:false}),"tr");
 expect(guide.observations.some(row=>row.label==="Canonical ile kayıtlı metin aynı"&&row.value==="Hayır")).toBe(true);
 expect(guide.steps.join(" ")).toContain("farklı öğeler");expect(guide.steps.join(" ")).toContain("aynı içerik kopyasında");
 expect(guide.warning).toContain("mevcut eşdeğer dil");expect(guide.sources[0]!.url).toContain("pagination");
 const hreflang=solutionGuide(issueById.get("hreflang-missing-return")!,evidence({lang:"tr"},{lang:"en",target:"https://example.com/en/page",return_link:false}),"tr");
 expect(hreflang.after).toContain('hreflang="tr"');expect(hreflang.steps.join(" ")).toContain("çeviri veya ilgisiz");
});
it("distinguishes observed text from illustrative replacement copy and escapes markup",()=>{
 const guide=solutionGuide(issueById.get("anchor-empty")!,evidence({}, {destination:'https://example.com/?q="<script>',text:"",destination_status:200}),"tr");
 expect(guide.observations.some(row=>row.label==="Bağlantı metni / etiketi"&&row.value==="Boş metin")).toBe(true);
 expect(guide.before).toContain("&quot;&lt;script&gt;");expect(guide.before).not.toContain("<a ");expect(guide.warning).toContain("SVG");expect(guide.after).toContain("konusunu açıklayan");
 const missing=solutionGuide(issueById.get("title-missing")!,evidence({title:null,title_count:0}),"en");
 expect(missing.before).toContain("unrecorded");expect(missing.after).toContain("Main page topic | Brand");
});
it("keeps missing compression data unknown and safely quotes measurement commands",()=>{
 const input=evidence({content_encoding:null,size_bytes:2468});input.selected_url="https://example.com/a'$(secret)`command`";
 const guide=solutionGuide(issueById.get("uncompressed")!,input,"en");
 expect(guide.before).toContain("Not recorded / empty");expect(guide.after).toContain("gzip on;");
 expect(guide.validation[0]).toContain("'\\''");expect(guide.validation[0]).not.toContain('"https://');
 expect(guide.steps.join(" ")).toContain("does not identify your server");
});
it("does not fabricate a preceding H0 tag or a fast response observation",()=>{
 const guide=solutionGuide(issueById.get("headings-non-sequential")!,evidence({}, {previous_level:0,previous_text:null,level:2,text:"First heading"}),"en");
 expect(guide.before).not.toContain("<h0>");expect(guide.before).toContain("No preceding heading");
 const slow=solutionGuide(issueById.get("slow-response")!,evidence({response_ms:1900}),"tr");
 expect(slow.before).toContain("1.900");expect(slow.after).toContain("time_starttransfer");
});

it("gives a concrete illustrative label to an observed WhatsApp target",()=>{
 const guide=solutionGuide(issueById.get("anchor-empty")!,evidence({}, {destination:"https://wa.me/123?text=",text:""}),"tr");
 expect(guide.after).toBe('<a href="https://wa.me/123?text=">WhatsApp ile iletişime geçin</a>');
});

it("preserves an observed SVG instead of reconstructing an empty anchor",()=>{
 const guide=solutionGuide(issueById.get("anchor-empty")!,evidence({}, {destination:"https://wa.me/905555555555",text:"",visible_text:"",name_source:"missing",has_svg:true,has_image:false}),"tr");
 expect(guide.before).toContain("SVG ikon mevcut");expect(guide.before).not.toContain("></a>");
 expect(guide.after).toContain('aria-label="WhatsApp ile iletişime geçin"');expect(guide.after).toContain('<svg aria-hidden="true">');
 expect(guide.steps.join(" ")).toContain("yalnızca görünür metni olmadığı");
});
