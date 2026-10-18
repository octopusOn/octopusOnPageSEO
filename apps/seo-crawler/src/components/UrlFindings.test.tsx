import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { TablePage } from "../api/types";
import { issueById } from "../catalog";
import { UrlFindings } from "./UrlFindings";

function render(ids: string[] | undefined, { excluded = [], activeIssueId = "", recorded = true, total = ids?.length ?? 0 }: {excluded?:string[];activeIssueId?:string;recorded?:boolean;total?:number} = {}) {
  const findings:TablePage|undefined = ids && {columns:[{name:"issue_id",kind:"text"}],rows:ids.map((id)=>[id]),total};
  return renderToStaticMarkup(<UrlFindings findings={findings} recorded={recorded} excluded={excluded} activeIssueId={activeIssueId} onIssue={()=>{}}/>);
}

it("prioritizes URL findings and explains the problem and correction using the shared catalog",()=>{
  const high=issueById.get("render-error")!;
  const html=render(["title-too-wide","render-error"]);
  expect(html.indexOf(high.tr.title)).toBeLessThan(html.indexOf(issueById.get("title-too-wide")!.tr.title));
  expect(html).toContain(high.tr.description.replaceAll("'","&#x27;"));
  expect(html).toContain("Aynı bulguya sahip tüm URL’ler");
  expect(html).toContain("2 kayıtlı bulgu");
  expect(html).toContain("tüm kontrollerin çalıştığı");
});

it("preserves excluded findings while hiding them from the initial list",()=>{
  const excluded=["render-error"];
  const html=render(["render-error","title-missing"],{excluded});
  expect(html).not.toContain(issueById.get("render-error")!.tr.title);
  expect(html).toContain("2 kayıtlı bulgu");
  expect(html).toContain("Hariç tutulanları göster");
  expect(excluded).toEqual(["render-error"]);
  expect(render(["render-error"],{excluded})).toContain("Görüntülenen bulgular hariç tutulmuş");
});

it("reveals and opens an explicitly selected excluded finding",()=>{
  const html=render(["title-too-wide","render-error"],{excluded:["render-error"],activeIssueId:"render-error"});
  expect(html).toContain(issueById.get("render-error")!.tr.title);
  expect(html).toMatch(/type="checkbox"[^>]*checked=""/);
  expect(html).toContain('class="finding-excluded"');
  expect(html).toContain('<details open=""');
});

it("keeps missing or unknown URL results inconclusive",()=>{
  expect(render([])).toContain("bu URL için kaydedilmiş bulgu yok");
  expect(render([],{recorded:false})).toContain("kayıtlı yanıtı veya bulgusu yok");
  const old=render(undefined);
  expect(old).toContain("URL bulguları alınamadı");
  expect(old).not.toContain("kaydedilmiş bulgu yok");
});

it("retains unknown historical rules as escaped IDs and reports a truncated preview",()=>{
  const html=render(["legacy-<script>"],{total:1005});
  expect(html).toContain("legacy-&lt;script&gt;");
  expect(html).not.toContain("<script>");
  expect(html).toContain("güncel katalogda bulunmuyor");
  expect(html).toContain("Önizlenen bulgular:");
  expect(html).toContain("1 / 1.005");
  expect(html).not.toContain("Aynı bulguya sahip tüm URL’ler");
});
