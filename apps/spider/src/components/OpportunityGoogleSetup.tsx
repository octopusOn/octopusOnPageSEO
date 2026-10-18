import {useState} from "react";
import {FileUp} from "lucide-react";
import {opportunitiesApi} from "../api/opportunities";
import {errorText} from "../api/client";
import {useI18n} from "../i18n";
import {Button,Modal} from "./ui";

export function OpportunityGoogleSetup({onSaved,onClose}:{onSaved:()=>Promise<void>;onClose:()=>void}){
 const {lang}=useI18n();const tr=lang==="tr";
 const [path,setPath]=useState(""),[busy,setBusy]=useState(false),[error,setError]=useState("");
 async function choose(){setError("");try{const selected=await opportunitiesApi.chooseGoogleClient();if(selected)setPath(selected);}catch(error){setError(errorText(error));}}
 async function save(){setBusy(true);setError("");try{await opportunitiesApi.importGoogleClient(path);await onSaved();onClose();}catch(error){setError(errorText(error));}finally{setBusy(false);}}
 return <Modal title={tr?"Google OAuth kurulumu":"Google OAuth setup"} onClose={onClose} footer={<><Button disabled={busy} onClick={onClose}>{tr?"Vazgeç":"Cancel"}</Button><Button primary busy={busy} disabled={!path} onClick={()=>void save()}>{tr?"Parola kasasına kaydet":"Save in credential store"}</Button></>}>
  <p>{tr?"Google Cloud projenizde Search Console API, Google Analytics Data API ve Google Analytics Admin API etkin olmalı.":"Enable Search Console API, Google Analytics Data API and Google Analytics Admin API in your Google Cloud project."}</p>
  <ol>
   <li>{tr?"Google Auth Platform’da uygulamayı yapılandırın; test modundaysa giriş yapacağınız hesabı test kullanıcılarına ekleyin.":"Configure your app in Google Auth Platform. In testing mode, add your account as a test user."}</li>
   <li>{tr?"Clients bölümünde Desktop app türünde istemci oluşturun ve JSON dosyasını indirin.":"Create a Desktop app client under Clients and download its JSON file."}</li>
   <li>{tr?"Dosyayı burada seçip kaydedin; ardından Google hesabıyla bağlan düğmesini kullanın.":"Select and save the file here, then use Connect Google account."}</li>
  </ol>
  <p className="faint">{tr?"İstemci bilgileri işletim sisteminin parola kasasında saklanır. Kaynak dosya fırsat deposuna kopyalanmaz. Google izinleri yalnızca Search Console ve Analytics okuma erişimidir. Test modunda Google bağlantısının süresi dolabilir; yeniden giriş yapabilirsiniz.":"Client configuration is kept in the operating-system credential store. The source file is not copied into the opportunity store. Google permissions are limited to Search Console and Analytics read access. Testing-mode connections can expire and require reconnecting."}</p>
  <Button disabled={busy} onClick={()=>void choose()}><FileUp size={16}/>{tr?"Masaüstü istemci JSON dosyasını seç":"Choose Desktop client JSON"}</Button>
  {path&&<p className="mono">{path.split(/[\\/]/).pop()}</p>}
  {error&&<div className="oc-banner error" role="alert">{error}</div>}
 </Modal>;
}
