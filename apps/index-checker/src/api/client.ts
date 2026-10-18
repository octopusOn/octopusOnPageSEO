import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { demoApi } from "./demo";
import { demoCloud } from "./demoCloud";
import type {
  AccountLogin,
  ApiConfig,
  CloudStatus,
  CreditSource,
  FinderJob,
  FinderPageRequest,
  FinderTablePage,
  RedirectCandidate,
  RedirectServerFormat,
  ScanComparison,
  ScanConfig,
  ScanMeta,
  ScanOverview,
  SourceMode,
} from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isDemo =
  typeof window === "undefined" || !("__TAURI_INTERNALS__" in window);

export function isDemoScanId(id: string): boolean {
  return isDemo || id.startsWith("finder-demo-");
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export const api = {
  async scans(): Promise<ScanMeta[]> {
    if (isDemo) return demoApi.listScans();
    const saved = await invoke<ScanMeta[]>("list_scans");
    return saved.length > 0 ? saved : demoApi.listScans();
  },
  async jobs(): Promise<FinderJob[]> {
    if (isDemo) return [];
    return invoke<FinderJob[]>("list_jobs");
  },
  async start(config: ScanConfig): Promise<string> {
    if (isDemo) {
      throw new Error(
        "Scans need the desktop app; the browser preview has sample data only.",
      );
    }
    return invoke<string>("start_scan", { config });
  },
  async control(
    id: string,
    action: "pause" | "resume" | "stop",
  ): Promise<void> {
    if (isDemo) return;
    return invoke("control_scan", { id, action });
  },
  async overview(id: string): Promise<ScanOverview> {
    if (isDemoScanId(id)) return demoApi.overview(id);
    return invoke<ScanOverview>("scan_overview", { id });
  },
  async page(id: string, request: FinderPageRequest): Promise<FinderTablePage> {
    if (isDemoScanId(id)) return demoApi.queryPage(id, request);
    return invoke<FinderTablePage>("scan_page", { id, request });
  },
  async exportScan(
    id: string,
    request: FinderPageRequest,
    path: string,
  ): Promise<number> {
    if (isDemoScanId(id)) return 0;
    return invoke<number>("export_scan", { id, request, path });
  },
  async redirectRules(
    id: string,
    server: RedirectServerFormat,
  ): Promise<string> {
    if (isDemoScanId(id)) return demoApi.redirectRules(id, server);
    return invoke<string>("redirect_rules", { id, server });
  },
  async reverifyScan(id: string, url?: string): Promise<ScanOverview> {
    if (isDemoScanId(id)) return demoApi.reverifyScan(id, url);
    return invoke<ScanOverview>("reverify_scan", { id, url: url ?? null });
  },
  async compareScans(
    baseId: string,
    targetId: string,
  ): Promise<ScanComparison> {
    if (isDemoScanId(baseId) || isDemoScanId(targetId)) {
      return demoApi.compareScans(baseId, targetId);
    }
    return invoke<ScanComparison>("compare_scans", {
      baseId,
      targetId,
    });
  },
  async redirectCandidates(
    id: string,
    url: string,
  ): Promise<RedirectCandidate[]> {
    if (isDemoScanId(id)) return demoApi.redirectCandidates(id, url);
    return invoke<RedirectCandidate[]>("redirect_candidates", { id, url });
  },
  async updateRedirectTarget(
    id: string,
    url: string,
    target: string,
  ): Promise<void> {
    if (isDemoScanId(id)) {
      demoApi.updateRedirectTarget(id, url, target);
      return;
    }
    return invoke("update_redirect_target", { id, url, target });
  },
  async deleteScan(id: string): Promise<void> {
    if (isDemoScanId(id)) {
      demoApi.deleteScan(id);
      return;
    }
    return invoke("delete_scan", { id });
  },
  async watch(onJob: (job: FinderJob) => void): Promise<() => void> {
    if (isDemo) return () => {};
    return listen<FinderJob>("finder-job", (event) => onJob(event.payload));
  },

  // OctopusOn SEO account: credits for Serper and SerpApi.
  async cloudStatus(): Promise<CloudStatus> {
    if (isDemo) return demoCloud.status();
    return invoke<CloudStatus>("cloud_status");
  },
  async creditSources(config: ApiConfig): Promise<CreditSource[]> {
    if (isDemo) return demoCloud.sources(config);
    return invoke<CreditSource[]>("credit_sources", { api: config });
  },
  async setSourceMode(source: string, mode: SourceMode): Promise<void> {
    if (isDemo) {
      demoCloud.setMode(source, mode);
      return;
    }
    return invoke("set_source_mode", { source, mode });
  },
  async accountLogin(): Promise<AccountLogin> {
    if (isDemo) return demoCloud.startLogin();
    return invoke<AccountLogin>("start_account_login");
  },
  async accountLoginState(): Promise<AccountLogin | null> {
    if (isDemo) return demoCloud.loginState();
    return invoke<AccountLogin | null>("account_login");
  },
  async cancelAccountLogin(): Promise<void> {
    if (isDemo) {
      demoCloud.cancelLogin();
      return;
    }
    return invoke("cancel_account_login");
  },
  async accountLogout(): Promise<void> {
    if (isDemo) {
      demoCloud.logout();
      return;
    }
    return invoke("account_logout");
  },
  async openAccountPage(page: "credits" | "devices" | "usage"): Promise<void> {
    if (isDemo) return;
    return invoke("open_account_page", { page });
  },
  async watchAccount(onLogin: (login: AccountLogin) => void): Promise<() => void> {
    if (isDemo) return demoCloud.onLogin(onLogin);
    return listen<AccountLogin>("finder-account", (event) => onLogin(event.payload));
  },
};
