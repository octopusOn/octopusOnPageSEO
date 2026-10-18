import {useEffect,useRef,useState} from "react";
import {FileUp} from "lucide-react";
import {useI18n,number,type Key} from "../i18n";
import {Button} from "./ui";
import {previewUrlList,UrlListError,type UrlListImportError,type UrlListPreview} from "../lib/urlListImport";

const errors:Record<UrlListImportError,Key>={size:"urlImportSize",records:"urlImportRecords",format:"urlImportFormat",csv:"urlImportCsv",column:"urlImportColumn",ambiguous:"urlImportAmbiguous",separator:"urlImportSeparator",encoding:"urlImportEncoding",read:"urlImportRead",existing:"urlImportExisting",columns:"urlImportColumns"};
export function UrlListImporter({disabled,onPending,onApply}:{disabled:boolean;onPending:(pending:boolean)=>void;onApply:(urls:string[],append:boolean)=>void}) {
  const {t}=useI18n();
  const input=useRef<HTMLInputElement>(null),revision=useRef(0);
  const [preview,setPreview]=useState<{name:string;data:UrlListPreview}|null>(null);
  const [reading,setReading]=useState(false),[error,setError]=useState<UrlListImportError|null>(null);
  useEffect(()=>()=>{revision.current++;},[]);
  function clear() {revision.current++;setPreview(null);setReading(false);setError(null);onPending(false);}
  async function select(file:File) {
    const request=++revision.current;
    setReading(true);setError(null);setPreview(null);onPending(true);
    try {
      const data=await previewUrlList(file);
      if(request===revision.current)setPreview({name:file.name,data});
    } catch(error) {
      if(request===revision.current)setError(error instanceof UrlListError?error.code:"read");
    } finally {if(request===revision.current)setReading(false);}
  }
  function apply(append:boolean) {
    if(!preview||reading||disabled||!preview.data.urls.length)return;
    try {onApply(preview.data.urls,append);clear();} catch {setError("existing");}
  }
  return <section className="url-list-import">
    <input ref={input} type="file" hidden accept=".txt,.csv,.tsv,text/plain,text/csv,text/tab-separated-values" aria-label={t("urlImportFile")} onChange={event=>{const file=event.target.files?.[0];event.target.value="";if(file)void select(file);}}/>
    <div className="row between"><Button small disabled={disabled||reading} onClick={()=>input.current?.click()}><FileUp size={15}/>{t("urlImportFile")}</Button><small>{t("urlImportFormats")}</small></div>
    {reading&&<p role="status">{t("urlImportReading")}</p>}
    {preview&&<UrlImportPreview name={preview.name} data={preview.data}/>}
    {error&&<div className="oc-banner error" role="alert">{t(errors[error])}</div>}
    {(preview||error||reading)&&<div className="row url-import-actions"><Button small disabled={disabled||reading||!preview?.data.urls.length} onClick={()=>apply(false)}>{t("urlImportReplace")}</Button><Button small disabled={disabled||reading||!preview?.data.urls.length} onClick={()=>apply(true)}>{t("urlImportAppend")}</Button><Button small disabled={disabled} onClick={clear}>{t("cancel")}</Button></div>}
  </section>;
}

export function UrlImportPreview({name,data}:{name:string;data:UrlListPreview}) {
  const {t,lang}=useI18n();
  const stats:[[Key,number],[Key,number],[Key,number]]=[["urlImportValid",data.urls.length],["urlImportDuplicates",data.duplicates],["urlImportInvalid",data.invalid]];
  return <div className="url-import-preview">
    <strong className="url-import-filename">{name}</strong><p>{t("urlImportPreviewNote")}</p>
    <dl className="url-import-stats">{stats.map(([key,value])=><div key={key}><dt>{t(key)}</dt><dd>{number(value,lang)}</dd></div>)}</dl>
    <small>{t("urlImportRows")}: {number(data.records,lang)}</small>
    {!!data.samples.length&&<ul className="url-import-errors">{data.samples.map(sample=><li key={sample.line}>{t("urlImportLine")} {number(sample.line,lang)}: {t(sample.reason==="credentials"?"urlImportCredential":"urlImportInvalidAddress")}</li>)}</ul>}
    <div className="url-import-sample"><b>{t("urlImportSample")}</b>{data.urls.length?<ol>{data.urls.slice(0,10).map(url=><li key={url}>{url}</li>)}</ol>:<p>{t("urlImportEmpty")}</p>}</div>
  </div>;
}
