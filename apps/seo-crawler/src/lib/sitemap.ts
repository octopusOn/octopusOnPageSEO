import type { Key } from "../i18n";
import type { CrawlMeta, SitemapPlan, SitemapReason } from "../api/types";
import type { JobsStatus } from "./sessions";

export const sitemapReasonKeys:Record<SitemapReason,Key>={external:"sitemapExternal",blocked:"sitemapBlocked",response:"sitemapResponse", "non-html":"sitemapNonHtml",noindex:"sitemapNoindex",rendering:"sitemapRendering",truncated:"sitemapTruncated",canonical:"sitemapCanonical","non-indexable":"sitemapNonIndexable","invalid-url":"sitemapInvalidUrl",duplicate:"sitemapDuplicate","other-origin":"sitemapOtherOrigin"};
export function canPlanSitemap(meta:CrawlMeta,active:boolean,jobsStatus:JobsStatus) {
  return meta.status!=="running"&&!active&&jobsStatus==="ready";
}
export function sitemapExportReady(plan:SitemapPlan|undefined,fetching:boolean,error:boolean) {
  return !!plan&&plan.included>0&&!!plan.origin&&!fetching&&!error;
}

export function sitemapXml(urls:string[]) {
  const escape=(url:string)=>url.replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll(">","&gt;").replaceAll('"',"&quot;").replaceAll("'","&apos;");
  return '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'+urls.map(url=>`  <url><loc>${escape(url)}</loc></url>\n`).join("")+'</urlset>\n';
}
