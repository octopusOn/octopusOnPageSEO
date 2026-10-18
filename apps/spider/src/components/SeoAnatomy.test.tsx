import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { categories } from "../catalog";
import { demoSeoScore } from "../lib/demoSeoScore";
import { seoSystems, scorePointText, scoreLossText } from "../lib/seoAnatomy";
import { SeoAnatomy } from "./SeoAnatomy";

it("covers every catalog branch once and preserves overall impact totals",()=>{
  const assigned=seoSystems.flatMap(system=>system.categories);
  expect(new Set(assigned).size).toBe(assigned.length);
  expect([...assigned].sort()).toEqual(categories.map(row=>row.id).sort());
  const score=demoSeoScore([{rules:["title-missing","anchor-empty"],analyzed:true,limited:false}]);
  expect(score.categories!.reduce((sum,row)=>sum+row.points,0)).toBeCloseTo(score.deductions.reduce((sum,row)=>sum+row.points,0));
});

it("renders all branches, system filters, exclusion accounting and unmeasured states",()=>{
  const score=demoSeoScore([{rules:["title-missing"],analyzed:true,limited:false}]);
  score.excluded_urls=1;score.excluded_links=751;score.excluded_findings=521;
  const html=renderToStaticMarkup(<SeoAnatomy score={score} onIssue={()=>{}}/>);
  expect(html).toContain("SEO Röntgeni");expect(html).toContain("Doğrulanan istisnalar puanı düşürmez");
  expect(html).toContain("751");expect(html).toContain("521");
  for(const category of categories)expect(html).toContain(renderToStaticMarkup(<>{category.tr}</>));
  expect(html).not.toMatch(/seo-body|seo-body-grid|viewBox="0 0 300 450"/);
  expect(html).toContain("Öncelikli düzeltmeler");expect(html).toContain("Çözüm önerisi");
  expect(html).toContain("Yetersiz veri");
  expect(html).toContain("Genel puana etkisi");expect(html).not.toMatch(/NaN|Infinity|−-?0(?: |<)/);
});

it("keeps small positive point impacts visible",()=>{expect(scorePointText(.01,"tr")).toBe("<0,1");expect(scorePointText(0,"tr")).toBe("0");expect(scoreLossText(-0,"tr")).toBe("0");expect(scoreLossText(.01,"tr")).toBe("−<0,1");});
