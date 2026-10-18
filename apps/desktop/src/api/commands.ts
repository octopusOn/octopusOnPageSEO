// Typed wrappers around the Tauri IPC commands (see src-tauri/src/commands).

import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  AccountStatus,
  AnalysisTable,
  AppInfo,
  ArchiveReport,
  BotInfo,
  CheckedQuery,
  ClientInfo,
  CommandError,
  CrawlOverview,
  CustomBot,
  ExportReport,
  FormatInfo,
  FormatInput,
  GeoStatus,
  GeoUpdateReport,
  ImportEvent,
  ImportReport,
  InspectReport,
  IpCheck,
  NormalizeOptions,
  PeriodReport,
  QueryAst,
  QueryPageResult,
  QuerySchema,
  QuerySpec,
  RangeSourceStatus,
  RangeUpdateReport,
  RobotsReport,
  SavedImportReport,
  SavedQueryInfo,
  SiteInfo,
  SiteOverview,
  SiteSettings,
  Table,
  TemplateInfo,
  UrlListReport,
  ValueSuggestion,
  VerifySiteReport,
} from "./types";

/** Rejections of `invoke` are `CommandError` objects; anything else is wrapped. */
export function asCommandError(error: unknown): CommandError {
  if (typeof error === "object" && error !== null && "kind" in error && "message" in error) {
    return error as CommandError;
  }
  return { kind: "error", message: String(error) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw asCommandError(error);
  }
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  accountStatus: () => call<AccountStatus>("account_status"),
  accountConnect: () => call<boolean>("account_connect"),
  accountCancel: () => call<void>("account_cancel"),
  accountLogout: () => call<void>("account_logout"),
  accountOpen: (page: "credits" | "devices") => call<void>("account_open", { page }),

  listClients: () => call<ClientInfo[]>("list_clients"),
  addClient: (name: string) => call<ClientInfo>("add_client", { name }),
  removeClient: (name: string) => call<void>("remove_client", { name }),
  listSites: () => call<SiteInfo[]>("list_sites"),
  addSite: (client: string, name: string, format: FormatInput) =>
    call<SiteInfo>("add_site", { client, name, format }),
  setSiteFormat: (name: string, format: FormatInput) =>
    call<SiteInfo>("set_site_format", { name, format }),
  removeSite: (name: string) => call<void>("remove_site", { name }),
  siteOverview: (name: string) => call<SiteOverview>("site_overview", { name }),
  exportSite: (site: string, path: string) => call<ArchiveReport>("export_site", { site, path }),
  importSite: (path: string, name: string | null, client: string | null) =>
    call<SiteInfo>("import_site", { path, name, client }),
  siteSettings: (site: string) => call<SiteSettings>("site_settings", { site }),
  setSiteSettings: (site: string, settings: SiteSettings) =>
    call<SiteSettings>("set_site_settings", { site, settings }),
  deleteImport: (site: string, importId: number) => call<void>("delete_import", { site, importId }),
  listFormats: () => call<FormatInfo[]>("list_formats"),

  inspectFiles: (paths: string[], format: FormatInput) =>
    call<InspectReport>("inspect_files", { paths, format }),
  importLogs: (
    importId: string,
    site: string,
    paths: string[],
    verifyDns: boolean,
    onEvent: (event: ImportEvent) => void,
  ) => {
    const channel = new Channel<ImportEvent>();
    channel.onmessage = onEvent;
    return call<ImportReport>("import_logs", { importId, site, paths, verifyDns, onEvent: channel });
  },
  cancelImport: (importId: string) => call<boolean>("cancel_import", { importId }),

  listTemplates: () => call<TemplateInfo[]>("list_templates"),
  checkQuery: (spec: QuerySpec) => call<CheckedQuery>("check_query", { spec }),
  parseQuery: (text: string) => call<QueryAst>("parse_query", { text }),
  querySchema: () => call<QuerySchema>("query_schema"),
  suggestValues: (site: string, field: string, search: string, filter: string | null, limit: number) =>
    call<ValueSuggestion[]>("suggest_values", { site, field, search, filter, limit }),
  runQuery: (site: string, spec: QuerySpec, offset: number, limit: number) =>
    call<QueryPageResult>("run_query", { site, spec, offset, limit }),
  exportQuery: (site: string, spec: QuerySpec, path: string) =>
    call<ExportReport>("export_query", { site, spec, path }),
  saveTable: (path: string, table: Table) => call<number>("save_table", { path, table }),

  listSavedQueries: () => call<SavedQueryInfo[]>("list_saved_queries"),
  saveQuery: (name: string, description: string, query: string, overwrite: boolean) =>
    call<SavedQueryInfo>("save_query", { name, description, query, overwrite }),
  removeSavedQuery: (name: string) => call<void>("remove_saved_query", { name }),
  exportSavedQueries: (path: string) => call<number>("export_saved_queries", { path }),
  importSavedQueries: (path: string, overwrite: boolean) =>
    call<SavedImportReport>("import_saved_queries", { path, overwrite }),

  listBots: (category?: string) => call<BotInfo[]>("list_bots", { category: category ?? null }),
  botRangeStatus: () => call<RangeSourceStatus[]>("bot_range_status"),
  geoStatus: () => call<GeoStatus>("geo_status"),
  updateGeoData: () => call<GeoUpdateReport>("update_geo_data"),
  listCustomBots: () => call<CustomBot[]>("list_custom_bots"),
  saveCustomBots: (bots: CustomBot[]) => call<CustomBot[]>("save_custom_bots", { bots }),
  exportCustomBots: (path: string) => call<number>("export_custom_bots", { path }),
  importCustomBots: (path: string) => call<CustomBot[]>("import_custom_bots", { path }),
  updateBotRanges: () => call<RangeUpdateReport>("update_bot_ranges"),
  verifySiteBots: (site: string, dns: boolean, all: boolean) =>
    call<VerifySiteReport>("verify_site_bots", { site, dns, all }),
  verifyIp: (ip: string, bot: string, dns: boolean) => call<IpCheck>("verify_ip", { ip, bot, dns }),

  crawlOverview: (site: string, period: string | null) =>
    call<CrawlOverview>("crawl_overview", { site, period }),
  robotsCompliance: (site: string, robots: string | null, filter: string | null) =>
    call<AnalysisTable<RobotsReport>>("robots_compliance", { site, robots, filter }),
  compareUrls: (
    site: string,
    sources: string[],
    filter: string | null,
    normalize: NormalizeOptions,
    host: string | null,
  ) => call<AnalysisTable<UrlListReport>>("compare_urls", { site, sources, filter, normalize, host }),
  comparePeriods: (site: string, before: string, after: string, group: string, filter: string | null) =>
    call<AnalysisTable<PeriodReport>>("compare_periods", { site, before, after, group, filter }),
};
