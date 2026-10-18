import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { defaultConfig } from "../api/types";
import { RenderingSettings } from "./RenderingSettings";
import { RepeatSettings } from "./RepeatSettings";

it("keeps viewport controls out of the HTML-only flow", () => {
  const html = renderToStaticMarkup(<RenderingSettings config={defaultConfig} onChange={()=>{}}/>);
  expect(html).not.toContain("Rendering görünüm alanı");
});

it("offers automatic and explicit presets in JavaScript mode with a user-agent explanation", () => {
  const config = { ...defaultConfig, rendering: { ...defaultConfig.rendering, mode: "javascript" as const } };
  const html = renderToStaticMarkup(<RenderingSettings config={config} onChange={()=>{}}/>);
  expect(html).toContain('value="auto" selected=""');
  expect(html).toContain("1280 × 800");
  expect(html).toContain("390 × 844");
  expect(html).toContain("kullanıcı aracısını seçili bot belirler");
  expect(html).toContain('role="switch" checked=""');
  expect(html).toContain("64 MiB");
});

it("retains an explicit disabled cache choice in rendering settings and saved-plan review",()=>{
  const config={...defaultConfig,rendering:{...defaultConfig.rendering,mode:"javascript" as const,cache_resources:false}};
  const html=renderToStaticMarkup(<RenderingSettings config={config} onChange={()=>{}}/>);
  expect(html).not.toContain('role="switch" checked=""');
  expect(renderToStaticMarkup(<RepeatSettings config={config}/>)).toContain("Sabit JavaScript ve CSS dosyalarını yeniden kullan</dt><dd>Hayır");
});

it("shows bounded custom fields and preserves their exact layout in the repeat-plan review", () => {
  const config = { ...defaultConfig, rendering: { ...defaultConfig.rendering, mode: "javascript" as const, viewport: { width: 768, height: 1024, mobile: true } } };
  const html = renderToStaticMarkup(<RenderingSettings config={config} onChange={()=>{}}/>);
  expect(html).toContain('value="custom" selected=""');
  expect(html).toContain('min="240" max="3840" step="1" value="768"');
  expect(html).toContain('value="1024"');
  expect(html).toContain('type="checkbox" checked=""');
  expect(renderToStaticMarkup(<RepeatSettings config={config}/>)).toContain("768 × 1024 CSS px · Mobil");
});
