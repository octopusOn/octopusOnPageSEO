import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import type { DuplicateGroup } from "../api/types";
import { DuplicateGroups } from "./DuplicateGroups";

vi.mock("../api/client", () => ({api:{duplicates:vi.fn()},errorText:String}));
function render(groups: DuplicateGroup[]) {
  const client=new QueryClient();
  client.setQueryData(["duplicates","crawl","/selected"],groups);
  const html=renderToStaticMarkup(<QueryClientProvider client={client}><DuplicateGroups id="crawl" url="/selected" onOpen={()=>{}} /></QueryClientProvider>);
  client.clear();
  return html;
}
it("shows complete group counts, field identity and a safe bounded preview",()=>{
  const html=render([{kind:"title",value:"Stored <script>",value_truncated:true,count:1505},{kind:"description",value:"Shared description",value_truncated:false,count:2}]);
  for(const text of ["Aynı başlık","Aynı açıklama","1.505","Bu grubu listele","Sayılara seçili URL dahildir","Önizleme kısaltıldı"])expect(html).toContain(text);
  expect(html).toContain("Stored &lt;script&gt;");
  expect(html).not.toContain("<script>");
});
it("describes normalized text without exposing a numeric fingerprint as content",()=>{
  const html=render([{kind:"content",value:null,value_truncated:false,count:10}]);
  expect(html).toContain("Aynı normalize metin");
  expect(html).toContain("Menü ve alt bilgi dahil");
  expect(html).toContain("Benzer içerik ayrı bir kontroldür");
});
it("keeps no-match results limited to the stored eligible pages",()=>{
  const html=render([]);
  expect(html).toContain("başka uygun sayfa bulunmadı");
  expect(html).not.toContain("Bu grubu listele");
});
