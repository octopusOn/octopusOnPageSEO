import {renderToStaticMarkup} from "react-dom/server";
import {expect,it} from "vitest";
import {UrlListImporter,UrlImportPreview} from "./UrlListImporter";
import {parseUrlList} from "../lib/urlListImport";
it("renders a local file picker and its explicit limits without starting a crawl",()=>{
  const html=renderToStaticMarkup(<UrlListImporter disabled={false} onPending={()=>{}} onApply={()=>{throw new Error("must not apply automatically");}}/>);
  expect(html).toContain('type="file"');expect(html).toContain("URL dosyası içe aktar");expect(html).toContain("10 MiB / 100.000 kayıt");expect(html).not.toContain("URL taslağını değiştir");
});
it("renders accurate preview counts and line errors without exposing rejected credentials or HTML",()=>{
  const data=parseUrlList('URL,Title\nhttps://example.com/a,Title\nhttps://example.com/a#two,Other\nhttps://user:secret@example.com/,Private\ninvalid,Error\n',"input.csv");
  const html=renderToStaticMarkup(<UrlImportPreview name="<script>.csv" data={data}/>);
  expect(html).toContain("&lt;script&gt;.csv");expect(html).not.toContain("<script>");expect(html).not.toContain("secret");
  expect(html).toMatch(/Benzersiz geçerli URL<\/dt><dd>1<\/dd>/);expect(html).toMatch(/Tekrarlanan kayıt<\/dt><dd>1<\/dd>/);expect(html).toMatch(/Geçersiz kayıt<\/dt><dd>2<\/dd>/);expect(html).toContain("Satır 4");expect(html).toContain("Bu bir önizlemedir");
});
it("previews at most ten valid URLs while showing complete counts",()=>{
  const data=parseUrlList(Array.from({length:30},(_,i)=>`https://example.com/${i}`).join("\n"),"input.txt");
  const html=renderToStaticMarkup(<UrlImportPreview name="input.txt" data={data}/>);
  expect(html).toMatch(/Benzersiz geçerli URL<\/dt><dd>30<\/dd>/);expect(html.match(/<li>/g)).toHaveLength(10);expect(html).not.toContain("https://example.com/29");
});
