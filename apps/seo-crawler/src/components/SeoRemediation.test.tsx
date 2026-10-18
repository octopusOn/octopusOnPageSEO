import { renderToStaticMarkup } from "react-dom/server";
import { expect,it } from "vitest";
import { issueById } from "../catalog";
import { SeoRemediationContent } from "./SeoRemediation";
import type { RemediationDetail } from "../api/types";

it("presents the exact URL, recorded state, steps, examples and verification without executing page markup",()=>{
 const empty={columns:[],rows:[],total:0};
 const detail:RemediationDetail={urls:empty,selected_url:"https://example.com/tr/page",facts:{columns:[{name:"title",kind:"text"},{name:"title_count",kind:"number"}],rows:[['<script>alert("page")</script>',1]],total:1},evidence:empty,evidence_kind:"facts"};
 const html=renderToStaticMarkup(<SeoRemediationContent rule={issueById.get("title-too-long")!} detail={detail}/>);
 for(const label of ["https://example.com/tr/page","Mevcut durum ve hata","Nasıl düzeltilmeli?","Bu URL için örnekler","Düzeltmeyi doğrulayın","orijinal HTML değildir"])expect(html).toContain(label);
 expect(html).toContain("&lt;script&gt;");expect(html).not.toContain("<script>");
 expect(html).toContain("kayıtlı metinden");
});
