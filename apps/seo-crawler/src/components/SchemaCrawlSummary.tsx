import type {SeoScore} from "../api/types";
import {useI18n,number} from "../i18n";
import {scoreLossText} from "../lib/seoAnatomy";
import {Button} from "./ui";

export function SchemaCrawlSummary({score,onIssue}:{score:SeoScore;onIssue:(id:string)=>void}) {
 const {lang}=useI18n();const tr=lang==="tr";const coverage=score.schema_validation;
 if(!coverage)return null;
 const points=score.categories?.find(c=>c.id==="structured-data")?.points??0;
 return <section className="oc-panel schema-crawl-summary" aria-label={tr?"Tarama Schema değerlendirmesi":"Crawl Schema assessment"}><header className="row between"><h3>{tr?"Schema / zengin sonuç alan kontrolü":"Schema / rich-result property checks"}</h3><strong>{scoreLossText(points,lang)} {tr?"puan":"points"}</strong></header><p>{tr?"Bu taramada etkin · Zorunlu alan hataları SEO Röntgeni ve genel puana dahil edildi.":"Enabled for this crawl · Required-property errors contribute to the score and breakdown."}</p><div className="schema-coverage-grid">{[
 [tr?"İncelenen URL":"Inspected URLs",coverage.checked_urls],
 [tr?"Puanlanan URL":"Scored URLs",coverage.scored_urls],
 [tr?"Hatalı URL":"URLs with errors",coverage.error_urls],
 [tr?"Alan uyarısı / öneri bulunan URL":"URLs with warnings / advice",coverage.warning_urls],
 [tr?"Schema bulunmayan URL":"URLs without Schema",coverage.missing_urls],
 [tr?"Kısmi / doğrulanamayan URL":"Partial / unverified URLs",coverage.unverified_urls],
 ].map(([label,count])=><div key={String(label)}><strong>{number(Number(count),lang)}</strong><span>{label}</span></div>)}</div><p className="faint">{tr?"Schema yokluğu, önerilen alanlar ve WebPage / FAQPage yapı uyarıları puanı düşürmez. Destek dışı türler, eksik belgeler ve başarısız render geçerli sayılmaz. Bu yerel kontrol Google zengin sonuç gösterimi garantisi değildir.":"Absent Schema, recommended fields and WebPage / FAQPage structure warnings do not deduct points. Unsupported types, incomplete documents and failed rendering are not treated as valid. Local checks do not guarantee Google rich-result appearance."}</p><div className="row">{score.deductions.filter(d=>["schema-required-invalid","structured-data-invalid"].includes(d.id)).map(d=><Button small key={d.id} onClick={()=>onIssue(d.id)}>{d.id==="schema-required-invalid"?(tr?"Zorunlu alan hataları":"Required-property errors"):(tr?"JSON-LD sözdizimi hataları":"JSON-LD syntax errors")} · {number(d.count,lang)} URL · {scoreLossText(d.points,lang)}</Button>)}{coverage.unverified_urls>0&&<Button small onClick={()=>onIssue("schema-unverified")}>{tr?"Doğrulanamayanları göster":"Show unverified"}</Button>}</div></section>;
}
