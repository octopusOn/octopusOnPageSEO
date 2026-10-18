import type { SchemaReport, SchemaTestRequest } from "../api/types";

export const SCHEMA_RULE_VERSION="2026-10-18.2";

export function schemaSummary(report:SchemaReport) {
  return {
    error:report.items.filter(item=>item.status==="error").length,
    warning:report.items.filter(item=>item.status==="warning").length,
    checked:report.items.filter(item=>item.status==="checked").length,
    unchecked:report.items.filter(item=>item.status==="unchecked").length,
  };
}
export function schemaInputValid(input:string,mode:SchemaTestRequest["mode"]) {
  if(!input.trim() || new TextEncoder().encode(input).length>4*1024*1024)return false;
  if(mode!=="url")return true;
  try {
    const url=new URL(input.trim());
    return input.length<=8192 && !/[\u0000-\u0020\u007f]/.test(input.trim()) && ["http:","https:"].includes(url.protocol) && !!url.hostname && !url.username && !url.password;
  } catch {return false;}
}
export function richResultsUrl(url:string) {
  if(!schemaInputValid(url,"url"))return null;
  const target=new URL("https://search.google.com/test/rich-results");
  target.searchParams.set("url",url);
  return target.href;
}
