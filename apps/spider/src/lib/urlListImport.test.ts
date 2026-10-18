import {expect,it,vi} from "vitest";
import {mergeImportedUrls,parseUrlList,previewUrlList,UrlListError,URL_LIST_MAX_BYTES,URL_LIST_MAX_RECORDS} from "./urlListImport";
const error=(text:string,name:string,code:string)=>{try{parseUrlList(text,name);throw new Error("expected import error");}catch(e){expect(e).toBeInstanceOf(UrlListError);expect((e as UrlListError).code).toBe(code);}};
it("normalizes TXT URLs, removes fragments and counts invalid/duplicate rows without losing their physical positions",()=>{
  const result=parseUrlList("\uFEFF# comments\r\n\r\n HTTPS://EXAMPLE.COM:443/a#one \r\nhttps://example.com/a#two\r\nftp://example.com/\r\nhttps://user:pass@example.com/\r\nhttps://example.com/b?x=1&y=2\r\n", "urls.TXT");
  expect(result).toEqual({urls:["https://example.com/a","https://example.com/b?x=1&y=2"],records:5,duplicates:1,invalid:2,samples:[{line:5,reason:"url"},{line:6,reason:"credentials"}]});
});
it("reads a selected CSV header column with quoted delimiters, escaped quotes and multiline metadata",()=>{
  const result=parseUrlList('Title,Address,Status\r\n"A, B","https://example.com/a,b",200\r\n"escaped ""quote""\nnext line","https://example.com/b",200\r\nmissing,,404\r\n',"crawl.csv");
  expect(result.urls).toEqual(["https://example.com/a,b","https://example.com/b"]);expect(result.records).toBe(3);expect(result.invalid).toBe(1);expect(result.samples).toEqual([{line:5,reason:"url"}]);
});
it("detects the delimiter after a quoted multiline header and retains physical error lines",()=>{
  const preview=parseUrlList('\r\n"Title,\r\ncontinued";Address;Status\r\n"Title, metadata";"https://example.com/a?x=1;y=2";200\r\nMissing;/relative;404\r\n',"multiline.csv");
  expect(preview).toEqual({urls:["https://example.com/a?x=1;y=2"],records:2,duplicates:0,invalid:1,samples:[{line:5,reason:"url"}]});
});
it("uses a recognized URL header rather than punctuation frequency to detect the delimiter",()=>{
  const preview=parseUrlList('Title, meta, description;URL\nOne, two, three;https://example.com/a?x=1&y=2\n',"punctuation.csv");
  expect(preview.urls).toEqual(["https://example.com/a?x=1&y=2"]);expect(preview.records).toBe(1);expect(preview.invalid).toBe(0);
});
it("rejects competing URL columns under different delimiters instead of importing the wrong addresses",()=>{
  const body='URL,Title;Address\nhttps://example.com/wrong,metadata;https://example.com/right\n';
  error(body,"ambiguous.csv","separator");
  expect(parseUrlList('sep=;\r\n'+body,"explicit.csv").urls).toEqual(["https://example.com/right"]);
});
it("supports semicolon CSV, Excel separator declarations, TSV and header-only exports",()=>{
  expect(parseUrlList('sep=;\r\nStatus;URL\r\n200;https://example.com/a\r\n',"input.csv").urls).toEqual(["https://example.com/a"]);
  expect(parseUrlList('Status;Adres\n200;"https://example.com/b?x=1;y=2"\n',"input.csv").urls).toEqual(["https://example.com/b?x=1;y=2"]);
  expect(parseUrlList('Page\tTitle\nhttps://example.com/a\t"comma, semicolon; tab\tvalue"\n',"input.tsv").urls).toEqual(["https://example.com/a"]);
  expect(parseUrlList('Address,Status\n',"empty.csv").records).toBe(0);
});
it("accepts headerless single-column CSV and skips blank records without guessing a URL column",()=>{
  expect(parseUrlList('\n"https://example.com/a,b"\n\n"https://example.com/c"\n',"single.csv").urls).toEqual(["https://example.com/a,b","https://example.com/c"]);
  error('Title,Destination\nName,https://example.com/\n',"input.csv","column");
  error('URL,Address\nhttps://example.com/a,https://example.com/b',"input.csv","ambiguous");
});
it("rejects malformed quoting, including a valid prefix followed by bad data",()=>{
  for(const text of ['Address,Title\nhttps://example.com/a,"unterminated','Address,Title\nhttps://example.com/a,"ok"trailer','Address,Title\nhttps://example.com/a,b"c','Address,Title\nhttps://example.com/a,One,Extra','URL;Status\nhttps://example.com/a?x=1;y=2;200'])error(text,"input.csv","csv");
});
it("rejects unsupported formats, UTF-16 NULs and oversized files instead of silently truncating",async()=>{
  error('<urlset/>',"input.xml","format");error('h\u0000t\u0000t\u0000p',"input.txt","encoding");
  error("x".repeat(URL_LIST_MAX_BYTES+1),"input.txt","size");
  const read=vi.fn();await expect(previewUrlList({name:"input.txt",size:URL_LIST_MAX_BYTES+1,arrayBuffer:read})).rejects.toMatchObject({code:"size"});expect(read).not.toHaveBeenCalled();
  await expect(previewUrlList({name:"input.txt",size:2,arrayBuffer:async()=>new Uint8Array([255,255]).buffer})).rejects.toMatchObject({code:"encoding"});
  await expect(previewUrlList({name:"input.txt",size:1,arrayBuffer:async()=>{throw new Error("no access");}})).rejects.toMatchObject({code:"read"});
});
it("enforces record and column limits without discarding overflow rows",()=>{
  const line="http://e.test/\n";
  const preview=parseUrlList(line.repeat(URL_LIST_MAX_RECORDS),"input.txt");expect(preview.records).toBe(URL_LIST_MAX_RECORDS);expect(preview.duplicates).toBe(URL_LIST_MAX_RECORDS-1);
  error(line.repeat(URL_LIST_MAX_RECORDS+1),"input.txt","records");
  error("Address\n"+line.repeat(URL_LIST_MAX_RECORDS+1),"input.csv","records");
  error("x,".repeat(512)+"x","input.csv","columns");
});
it("bounds error examples and rejects relative, scheme-less, malformed-escape and whitespace addresses",()=>{
  const invalid=["/relative","example.com","javascript:alert(1)","https://example.com/a%zz","https://example.com/a b","https://","data:text/html,hello","https://user@example.com/"].join("\n");
  const result=parseUrlList(invalid,"input.txt");expect(result.urls).toEqual([]);expect(result.invalid).toBe(8);expect(result.samples).toHaveLength(5);
});
it("decodes valid UTF-8 and preserves Unicode and query data while deduplicating canonical URLs",async()=>{
  const text="https://example.com/çalışma?x=1&x=2#one\nhttps://example.com/%C3%A7al%C4%B1%C5%9Fma?x=1&x=2#two";
  const bytes=new TextEncoder().encode(text);
  const preview=await previewUrlList({name:"input.txt",size:bytes.byteLength,arrayBuffer:async()=>bytes.buffer});
  expect(preview.urls).toEqual(["https://example.com/%C3%A7al%C4%B1%C5%9Fma?x=1&x=2"]);expect(preview.duplicates).toBe(1);
});
it("applies replace or append explicitly, preserving ordering and refusing an invalid existing draft",()=>{
  const imported=["https://example.com/a","https://example.com/b"];
  expect(mergeImportedUrls("https://example.com/a#old\nexample.com/c",imported,true)).toEqual(["https://example.com/a","https://example.com/c","https://example.com/b"]);
  expect(mergeImportedUrls("ftp://bad.example/",imported,false)).toEqual(imported);
  expect(()=>mergeImportedUrls("ftp://bad.example/",imported,true)).toThrow(UrlListError);expect(imported).toHaveLength(2);
});
