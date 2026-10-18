import {describe,expect,it} from "vitest";
import {richResultsUrl,schemaInputValid,schemaSummary} from "./schema";
import type {SchemaReport} from "../api/types";

describe("explicit Schema input",()=>{
  it("accepts only explicit HTTP URLs without credentials, controls or oversized source",()=>{
    expect(schemaInputValid("https://example.com/page?q=2","url")).toBe(true);
    for(const input of ["example.com","javascript:alert(1)","file:///etc/passwd","https://user:password@example.com","https://exam\nple.com"])expect(schemaInputValid(input,"url")).toBe(false);
    expect(schemaInputValid("{}","jsonld")).toBe(true);
    expect(schemaInputValid(" ","html")).toBe(false);
    expect(schemaInputValid("ş".repeat(2*1024*1024+1),"html")).toBe(false);
  });
  it("encodes an official test link without changing or interpreting the tested URL",()=>{
    const input='https://example.com/?a=1&b=<script>';
    const link=new URL(richResultsUrl(input)!);
    expect(link.origin).toBe("https://search.google.com");
    expect(link.searchParams.get("url")).toBe(input);
    expect(richResultsUrl("javascript:alert(1)")).toBeNull();
  });
  it("counts item states once instead of presenting multiple recommendations as failed items",()=>{
    const base={block:1,format:"jsonld",path:"",types:["Product"],profile:"Product snippet",issues:[]};
    const report:SchemaReport={rule_version:"test",jsonld_blocks:1,limited:false,items:[{...base,status:"error"},{...base,status:"warning"},{...base,status:"checked"},{...base,status:"unchecked"}]};
    expect(schemaSummary(report)).toEqual({error:1,warning:1,checked:1,unchecked:1});
  });
});
