import type {
  AccountLogin,
  ApiConfig,
  CloudStatus,
  CreditSource,
  IndexEngine,
  SourceMode,
  Via,
} from "./types";

/** The OctopusOn SEO account in the browser preview: the one-click connection takes a moment, then Serper and SerpApi are on offer. */

const PORTAL = "https://my.octopuson.com";

const cloud: CloudStatus = {
  signed_in: false,
  account: null,
  error: null,
  portal_url: PORTAL,
  api: "https://api.octopuson.com",
};
const modes: Record<string, SourceMode> = {};
let login: AccountLogin | null = null;
let loginTimer: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<(login: AccountLogin) => void>();

/** How a source is paid, as `octopuson_cloud::choose` decides it. */
export function choose(mode: SourceMode, own: boolean, credit: boolean): Via | null {
  if ((mode === "own" || mode === "auto") && own) return "own";
  if ((mode === "credit" || mode === "auto") && credit) return "credit";
  return null;
}

const CREDITABLE: { id: string; name: string; engine: IndexEngine; own: (api: ApiConfig) => boolean }[] = [
  { id: "serper", name: "Serper.dev", engine: "serper_api", own: (api) => api.serper_api_key.trim() !== "" },
  { id: "serpapi", name: "SerpApi", engine: "serpapi", own: (api) => api.serpapi_key.trim() !== "" },
];

const OWN_KEY_ONLY: { id: string; name: string; engine: IndexEngine; own: (api: ApiConfig) => boolean }[] = [
  {
    id: "google-cse",
    name: "Google Custom Search",
    engine: "google_api",
    own: (api) => api.google_api_key.trim() !== "" && api.google_cx.trim() !== "",
  },
  { id: "yandex-xml", name: "Yandex XML", engine: "yandex_api", own: (api) => api.yandex_api_key.trim() !== "" },
];

function finishLogin(status: AccountLogin["status"]) {
  if (!login) return;
  login = { ...login, status };
  for (const listener of listeners) listener(login);
}

export const demoCloud = {
  status(): CloudStatus {
    return { ...cloud };
  },
  sources(api: ApiConfig): CreditSource[] {
    const offered = cloud.signed_in;
    return [
      ...CREDITABLE.map((source): CreditSource => {
        const mode = modes[source.id] ?? "auto";
        const own = source.own(api);
        return {
          id: source.id,
          name: source.name,
          engine: source.engine,
          creditable: true,
          mode,
          own_key: own,
          via: choose(mode, own, offered),
          credit: offered ? { connector: source.id, price_multiplier: 1.35 } : null,
        };
      }),
      ...OWN_KEY_ONLY.map((source): CreditSource => {
        const own = source.own(api);
        return {
          id: source.id,
          name: source.name,
          engine: source.engine,
          creditable: false,
          mode: "own",
          own_key: own,
          via: own ? "own" : null,
          credit: null,
        };
      }),
    ];
  },
  setMode(source: string, mode: SourceMode) {
    if (!CREDITABLE.some((known) => known.id === source)) {
      throw new Error(`${source} cannot run on OctopusOn SEO credits`);
    }
    if (mode === "auto") delete modes[source];
    else modes[source] = mode;
  },
  startLogin(): AccountLogin {
    if (login?.status === "waiting") return login;
    login = { status: "waiting", error: null };
    clearTimeout(loginTimer);
    loginTimer = setTimeout(() => {
      cloud.signed_in = true;
      cloud.account = {
        email: "ornek@ajans.com.tr",
        name: "Örnek Ajans",
        balance_micros: 12_400_000,
        held_micros: 0,
        available_micros: 12_400_000,
        credit_unit_micros: 1000,
        low_balance: false,
        portal_url: PORTAL,
      };
      finishLogin("approved");
    }, 1500);
    return login;
  },
  loginState(): AccountLogin | null {
    return login;
  },
  cancelLogin() {
    clearTimeout(loginTimer);
    if (login?.status === "waiting") finishLogin("cancelled");
  },
  logout() {
    cloud.signed_in = false;
    cloud.account = null;
  },
  onLogin(listener: (login: AccountLogin) => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};
