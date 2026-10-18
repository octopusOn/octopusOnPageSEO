import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { ColumnSettings } from "./ColumnSettings";

const schema=["address","status_code","title","word_count","meta_description"];
const defaults=["address","status_code","title"];
function render(saved?:string[],required=["address"],columns=schema) {
  return renderToStaticMarkup(<ColumnSettings tabLabel="Dahili URL" schema={columns} defaults={defaults} saved={saved} required={required} onApply={()=>{}} onClose={()=>{}}/>);
}

it("provides reviewable field selection, search, order controls, reset, apply and cancel",()=>{
  const html=render();
  for(const text of ["Dahili URL · Sütunlar","Görünen sütunlar","Diğer alanlar","Alanlarda ara…","Sütunları uygula","Varsayılanlara dön","Tümünü seç","İsteğe bağlı alanları temizle","Vazgeç"]) expect(html).toContain(text);
  expect(html).toContain('aria-label="Sütunu yukarı taşı: Sayfa başlıkları"');
  expect(html).toMatch(/type="checkbox" disabled="" checked=""/);
  expect(html).toContain("filtreli dışa aktarma aynı sütunları");
});

it("reflects a custom column order without changing the caller's saved choice",()=>{
  const saved=["word_count","title"];
  const html=render(saved);
  const shown=html.slice(html.indexOf('aria-label="Görünen sütunlar"'),html.indexOf('aria-label="Diğer alanlar"'));
  expect(shown.indexOf("URL")).toBeLessThan(shown.indexOf("Kelime"));
  expect(shown.indexOf("Kelime")).toBeLessThan(shown.indexOf("Sayfa başlıkları"));
  expect(saved).toEqual(["word_count","title"]);
});

it("locks temporary comparison fields and keeps them distinct from optional choices",()=>{
  const html=render(["word_count"],["address","meta_description"]);
  expect((html.match(/type="checkbox" disabled="" checked=""/g)??[]).length).toBe(2);
  expect(html).toContain("siz seçmediyseniz geçicidir");
});

it("escapes historical column names and keeps endpoint-only layouts usable",()=>{
  const html=render([], ["address"], [...schema,"legacy<script>"]);
  expect(html).toContain("legacy&lt;script&gt;");
  expect(html).not.toContain("<script>");
  expect(html).toContain("Sütunları uygula");
});
