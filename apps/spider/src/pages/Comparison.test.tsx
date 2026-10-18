import {renderToStaticMarkup} from "react-dom/server";
import {expect,it,vi} from "vitest";
vi.mock("../api/client",()=>({api:{},errorText:String,isDemo:false}));
import {ComparisonDetails} from "./Comparison";
import type {TablePage} from "../api/types";
const table=(rows:TablePage["rows"]):TablePage=>({columns:["field","before_value","after_value","changed","in_baseline","in_current"].map(name=>({name,kind:"text"})),rows,total:rows.length});
it("renders changed values escaped and distinguishes NULL from empty text",()=>{
  const html=renderToStaticMarkup(<ComparisonDetails data={table([["title","<script>before</script>","After",true,true,true],["meta_description",null,"",true,true,true],["h1","Same","Same",false,true,true]])}/>);
  expect(html).toContain("&lt;script&gt;before&lt;/script&gt;"); expect(html).not.toContain("<script>");
  expect(html).toContain("Kayıtlı değer yok (NULL)"); expect(html).toContain("Boş metin"); expect(html).not.toContain(">Same<");
});
it("shows one-sided URLs without claiming changed fields or overwriting absence with NULL",()=>{
  const html=renderToStaticMarkup(<ComparisonDetails data={table([["title","Old title",null,false,true,false]])}/>);
  expect(html).toContain("Old title"); expect(html).toContain("URL bu kapsamda yok"); expect(html).toContain("Eksik taraf için alan değişikliği varsayılmaz"); expect(html).not.toContain("compare-change-dot");
});
it("explains unchanged tracked fields and handles unknown URLs",()=>{
  expect(renderToStaticMarkup(<ComparisonDetails data={table([["title","A","A",false,true,true]])}/>)).toContain("İzlenen alanlar aynı");
  expect(renderToStaticMarkup(<ComparisonDetails data={table([])}/>)).toContain("Eşleşen sonuç yok");
});
