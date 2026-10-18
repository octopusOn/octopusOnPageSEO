import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { Coins, LogIn, LogOut, MonitorSmartphone } from "lucide-react";

import { isNative } from "../api/client";
import { number, useI18n } from "../i18n";
import { Button } from "./ui";

interface AccountStatus {
  signed_in: boolean;
  account: {
    email: string | null;
    available_micros: number;
    credit_unit_micros: number;
    low_balance: boolean;
  } | null;
  error: string | null;
}

const strings = {
  en: {
    title: "OctopusOn SEO account",
    intro:
      "The SEO Crawler has no paid sources. Your OctopusOn SEO account is shared by every OctopusOn SEO program on this computer: connect it once and SERP Checker, SEO Keyword Research and Index Checker can use your credits.",
    connect: "Connect account",
    waiting: "Finish signing in on the page that opened in your browser…",
    cancel: "Cancel",
    signOut: "Disconnect",
    notConnected: "Not connected",
    credits: "credits",
    low: "Your balance is low.",
    buy: "Buy credits",
    devices: "Devices",
  },
  tr: {
    title: "OctopusOn SEO hesabı",
    intro:
      "SEO Crawler'da ücretli kaynak yok. OctopusOn SEO hesabınız bu bilgisayardaki tüm OctopusOn SEO programlarınca paylaşılır: bir kez bağlayın, SERP Checker, SEO Keyword Research ve Index Checker kredinizi kullanabilsin.",
    connect: "Hesaba bağlan",
    waiting: "Tarayıcınızda açılan sayfada girişi tamamlayın…",
    cancel: "Vazgeç",
    signOut: "Bağlantıyı kes",
    notConnected: "Bağlı değil",
    credits: "kredi",
    low: "Bakiyeniz azaldı.",
    buy: "Kredi yükle",
    devices: "Cihazlar",
  },
};

/** The preview has no account. */
const offline: AccountStatus = { signed_in: false, account: null, error: null };

function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return isNative ? invoke<T>(command, args) : Promise.resolve((command === "account_status" ? offline : false) as T);
}

/** The OctopusOn SEO account every program on this computer shares: one click connects them all. */
export function AccountCard() {
  const { lang } = useI18n();
  const s = strings[lang === "tr" ? "tr" : "en"];
  const queryClient = useQueryClient();
  // Another OctopusOn SEO program may connect or disconnect the account meanwhile.
  const status = useQuery({ queryKey: ["account"], queryFn: () => call<AccountStatus>("account_status"), refetchInterval: 30_000 });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["account"] });
  const connect = useMutation({ mutationFn: () => call<boolean>("account_connect"), onSettled: refresh });
  const logout = useMutation({ mutationFn: () => call<void>("account_logout"), onSettled: refresh });
  const open = useMutation({ mutationFn: (page: "credits" | "devices") => call<void>("account_open", { page }) });
  const cloud = status.data;
  const account = cloud?.account;
  const failure = connect.error ?? logout.error ?? open.error;
  return (
    <section className="oc-panel settings-card">
      <h2>{s.title}</h2>
      <p className="setting-hint">{s.intro}</p>
      {cloud?.error && <div className="oc-banner warning">{cloud.error}</div>}
      {account?.low_balance && <div className="oc-banner warning">{s.low}</div>}
      <div className="row between wrap">
        <div>
          <strong>{cloud?.signed_in ? (account?.email ?? "") : s.notConnected}</strong>
          {account && (
            <div className="faint">
              {number(account.available_micros / account.credit_unit_micros, lang)} {s.credits}
            </div>
          )}
        </div>
        <div className="row">
          {cloud?.signed_in ? (
            <>
              <Button primary onClick={() => open.mutate("credits")}>
                <Coins size={15} />
                {s.buy}
              </Button>
              <Button onClick={() => open.mutate("devices")}>
                <MonitorSmartphone size={15} />
                {s.devices}
              </Button>
              <Button onClick={() => logout.mutate()}>
                <LogOut size={15} />
                {s.signOut}
              </Button>
            </>
          ) : connect.isPending ? (
            <>
              <span className="faint">{s.waiting}</span>
              <Button onClick={() => void call<void>("account_cancel")}>{s.cancel}</Button>
            </>
          ) : (
            <Button primary onClick={() => connect.mutate()}>
              <LogIn size={15} />
              {s.connect}
            </Button>
          )}
        </div>
      </div>
      {failure && <div className="oc-banner error">{String(failure)}</div>}
    </section>
  );
}
