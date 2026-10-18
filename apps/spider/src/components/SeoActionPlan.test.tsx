import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { issueById } from "../catalog";
import { seoActionPlan } from "../lib/seoActionPlan";
import { demoSeoScore } from "../lib/demoSeoScore";
import { SeoActionPlan } from "./SeoActionPlan";

it("shows the first three fixes with rationale, current loss and evidence access", () => {
  const score = demoSeoScore([{rules:["hreflang-missing-return","anchor-empty","uncompressed","slow-response"],analyzed:true,limited:false}]);
  const actions = seoActionPlan(score);
  const html = renderToStaticMarkup(<SeoActionPlan actions={actions} available onIssue={()=>{}}/>);
  expect(html).toContain("Öncelikli düzeltmeler");
  expect(html).toContain("Buradan başlayın");
  expect(html).toContain("yüksek → orta → düşük");
  for (const action of actions.slice(0,3)) {
    expect(html).toContain(renderToStaticMarkup(<>{action.rule.tr.fix}</>));
    expect(html).toContain(renderToStaticMarkup(<>{action.rule.tr.description}</>));
    expect(html).toContain(`aria-label="Önerilen sıra ${action.rank}"`);
  }
  expect(html).not.toContain(renderToStaticMarkup(<>{actions[3]!.rule.tr.fix}</>));
  expect(html).toContain("Tüm düzeltmeleri göster");
  expect(html).toContain("Etkilenen URL’leri gör");
  expect(html).toContain("yeniden tarayarak doğrulayın");
  expect(html.indexOf(issueById.get("hreflang-missing-return")!.tr.title)).toBeLessThan(html.indexOf(issueById.get("anchor-empty")!.tr.title));
});

it("distinguishes no scored problems from an unavailable assessment", () => {
  const empty = renderToStaticMarkup(<SeoActionPlan actions={[]} available onIssue={()=>{}}/>);
  expect(empty).toContain("Bu puanı düşüren kayıtlı bir bulgu yok");
  expect(empty).not.toContain("Buradan başlayın");
  const missing = renderToStaticMarkup(<SeoActionPlan actions={[]} available={false} onIssue={()=>{}}/>);
  expect(missing).toContain("hesaplanmış bir puan gerekli");
  expect(missing).not.toContain("Bu puanı düşüren kayıtlı bir bulgu yok");
});
