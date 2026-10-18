// Browser-only fixtures. Native builds always use the Rust crawler over IPC.
import type {
  Cell,
  CrawlConfig,
  CrawlMeta,
  Job,
  Overview,
  PageRequest,
  TablePage,
  TableExportRequest,
  UrlActivity,
  RedirectTrace,
  DuplicateKind,
  DuplicateGroup,
  ComparisonRequest,
} from "./types";
import { defaultConfig } from "./types";
import { responseGroup, responseGroups } from "../lib/responses";
import { sitemapXml } from "../lib/sitemap";
import { issueById } from "../catalog";
import { isLinkEvidenceIssue } from "../lib/links";
import { demoSeoScore } from "../lib/demoSeoScore";

const columns = [
  "address",
  "content",
  "status_code",
  "error",
  "indexability",
  "title",
  "title_length",
  "title_count",
  "meta_description",
  "meta_description_length",
  "h1",
  "h1_count",
  "canonical",
  "crawl_depth",
  "inlinks",
  "outlinks",
  "word_count",
  "size_bytes",
  "response_time_ms",
  "lang",
];
const paths = [
  "/",
  "/collections",
  "/collections/outdoor",
  "/collections/lighting",
  "/products/arc-chair",
  "/products/orbit-lamp",
  "/products/linen-sofa",
  "/journal",
  "/journal/materials",
  "/about",
  "/contact",
  "/shipping",
  "/returns",
  "/old-collection",
  "/products/discontinued",
  "/search?q=chair",
];
function records(host: string, count = 96): Cell[][] {
  return Array.from({ length: count }, (_, i) => {
    const path =
      paths[i] ??
      `/${i % 3 === 0 ? "products" : i % 3 === 1 ? "journal" : "collections"}/${["natural-wood", "woven-linen", "sculpted-light", "everyday-design", "living-spaces"][i % 5]}-${i}`;
    const status = i === 13 ? 301 : i % 19 === 14 ? 404 : i === 46 ? 500 : 200;
    const title =
      i % 13 === 4
        ? ""
        : i % 9 === 3
          ? "Koleksiyon | Atelier"
          : `${path === "/" ? "Atelier — Modern yaşam için özenle tasarlandı" : path.split("/").at(-1)?.replaceAll("-", " ")} | Atelier`;
    const description =
      i % 7 === 2
        ? ""
        : "Zamansız tasarımlar ve doğal malzemelerle yaşam alanınızı yeniden keşfedin. Atelier koleksiyonunu inceleyin.";
    return [
      `https://${host}${path}`,
      "html",
      status,
      null,
      status === 301
        ? "redirected"
        : status >= 500
          ? "server-error"
          : status >= 400
            ? "client-error"
            : i % 17 === 9
              ? "noindex"
              : "indexable",
      title,
      title.length,
      1,
      description,
      description.length,
      title ? (title.split("|")[0] ?? "") : null,
      i % 8 === 5 ? 0 : 1,
      `https://${host}${path}`,
      Math.min(6, Math.floor(i / 20)),
      (i % 12) + 1,
      (i % 16) + 2,
      150 + i * 23,
      12400 + i * 417,
      64 + ((i * 37) % 1280),
      "tr",
    ];
  });
}
const byCrawl = new Map<string, Cell[][]>();
const fixture: CrawlMeta = {
  id: "demo-atelier",
  name: "atelier.example",
  start_urls: ["https://atelier.example/"],
  created_at_ms: Date.now() - 900000,
  finished_at_ms: Date.now() - 540000,
  status: "completed",
  progress: {
    crawled: 96,
    queued: 0,
    errors: 6,
    elapsed_ms: 360000,
    discovered: 96,
    in_flight: 0,
    sitemaps: 1,
    paused: false,
  },
  limit_reached: false,
  error: null,
  config: defaultConfig,
};
const previousFixture:CrawlMeta={...structuredClone(fixture),id:"demo-atelier-previous",created_at_ms:fixture.created_at_ms-86400000,finished_at_ms:fixture.finished_at_ms!-86400000};
let crawls: CrawlMeta[] = [fixture,previousFixture];
byCrawl.set(fixture.id, records(fixture.name));
const previousRecords=records(fixture.name).slice(0,-3);
previousRecords[0]![5]="Atelier — Previous page title";
previousRecords[0]![6]=String(previousRecords[0]![5]).length;
previousRecords[1]![2]=404;
previousRecords[1]![4]="client-error";
previousRecords[2]![4]="noindex";
const previousOnly=[...previousRecords[0]!];
previousOnly[0]=previousOnly[12]="https://atelier.example/previous-only";
previousOnly[5]=previousOnly[10]="Previous collection";
previousOnly[6]=String(previousOnly[5]).length;
previousRecords.push(previousOnly);
previousFixture.progress={...previousFixture.progress,crawled:previousRecords.length,discovered:previousRecords.length,errors:previousRecords.filter(row=>Number(row[2])>=400).length};
byCrawl.set(previousFixture.id,previousRecords);
const jobs = new Map<string, Job>();
function table(
  names: string[],
  rows: Cell[][],
  total = rows.length,
): TablePage {
  return {
    columns: names.map((name) => ({ name, kind: "text" })),
    rows,
    total,
  };
}
function findings(row: Cell[]) {
  const ids: string[] = [];
  const status = Number(row[2]);
  if (status >= 400 && status < 500) ids.push("internal-client-error");
  if (status >= 500 && status < 600) ids.push("internal-server-error");
  if (status === 301) ids.push("internal-redirect");
  if (status === 200) {
    if (!row[5]) ids.push("title-missing");
    if (Number(row[6]) > 0 && Number(row[6]) < 30) ids.push("title-too-short");
    if (row[5] === "Koleksiyon | Atelier") ids.push("title-duplicate");
    if (!row[8]) ids.push("description-missing");
    if (row[11] === 0) ids.push("h1-missing");
    if (row[4] === "noindex") ids.push("noindex");
    if (Number(row[13]) >= 4) ids.push("deep-page");
    if (Number(row[18]) > 1000) ids.push("slow-response");
    if (Number(row[16]) < 200) ids.push("content-low");
  }
  return ids.filter(
    (id) =>
      row[4] === "indexable" ||
      ![
        "title-missing",
        "title-duplicate",
        "description-missing",
        "h1-missing",
        "deep-page",
        "content-low",
      ].includes(id),
  );
}
function data(id: string, tab: string): TablePage {
  const rows = byCrawl.get(id) ?? [];
  if (tab === "content-matches") return table(["address", "match_address", "hamming_distance", "word_count", "match_word_count", "title", "match_title"], []);
  if (tab === "rendering")
    return table(
      ["address", "rendered", "render_error", "raw_title", "rendered_title"],
      [],
    );
  if (tab === "external")
    return table(["address", "content", "status_code", "inlinks"], []);
  if (tab === "issues")
    return table(
      ["issue_id", "address"],
      rows.flatMap((row) =>
        findings(row).map((rule) => [rule, row[0] ?? null]),
      ),
    );
  if (tab === "response-codes")
    return table(
      ["address", "content", "status_code", "indexability", "response_time_ms", "error", "blocked_by_robots"],
      rows.map((r) => [
        r[0] ?? null,
        r[1] ?? null,
        r[2] ?? null,
        r[4] ?? null,
        r[18] ?? null,
        r[3] ?? null,
        false,
      ]),
    );
  if (tab === "links")
    return table(
      [
        "source",
        "destination",
        "kind",
        "text",
        "nofollow",
        "internal",
        "destination_status",
        "source_internal",
        "destination_internal",
        "destination_error",
      ],
      rows
        .slice(1)
        .map((r, i) => [
          rows[Math.floor(i / 8)]?.[0] ?? "",
          r[0] ?? "",
          "hyperlink",
          r[5] ?? "",
          false,
          true,
          r[2] ?? null,
          true,
          true,
          r[3] ?? null,
        ]),
    );
  if (tab === "images")
    return table(
      [
        "image",
        "page",
        "alt",
        "missing_alt",
        "has_dimensions",
        "lazy",
        "status_code",
        "size_bytes",
      ],
      rows
        .filter((_, i) => i % 3 === 0)
        .map((r, i) => [
          `https://atelier.example/images/product-${i}.webp`,
          r[0] ?? "",
          i % 5 === 0 ? null : "Atelier collection",
          i % 5 === 0,
          true,
          true,
          200,
          32000 + i * 2100,
        ]),
    );
  if (tab === "headings")
    return table(
      ["page", "level", "text", "length"],
      rows
        .filter((r) => r[11] !== 0)
        .map((r) => [r[0] ?? "", 1, r[10] ?? "", String(r[10] ?? "").length]),
    );
  if (tab === "structured-data")
    return table(
      ["page", "format", "types", "valid"],
      rows
        .filter((_, i) => i % 2 === 0)
        .map((r) => [r[0] ?? "", "json-ld", "Product", true]),
    );
  if (tab === "sitemaps")
    return table(
      ["url", "status_code", "kind", "error", "entries", "children", "redirect_url", "redirect_location", "redirect_outcome", "final_url", "final_status", "redirect_hops"],
      [
        [
          `https://${crawls.find((c) => c.id === id)?.name}/sitemap.xml`,
          200,
          "urlset",
          null,
          rows.length,
          0,
          null,null,null,null,null,null,
        ],
      ],
    );
  if (tab === "sitemap-urls") return table(
    ["url","crawl_url","sitemap","lastmod","status_code","indexability","crawl_depth"],
    rows.map(row=>[`${String(row[0])}#sitemap`,row[0]??null,`${new URL(String(row[0])).origin}/sitemap.xml`,null,row[2]??null,row[4]??null,row[12]??null]),
  );
  if (tab === "robots")
    return table(
      ["url", "status", "http_status", "sitemaps", "text"],
      [
        [
          `https://atelier.example/robots.txt`,
          "available",
          200,
          "https://atelier.example/sitemap.xml",
          "User-agent: *\nDisallow: /search\nSitemap: https://atelier.example/sitemap.xml",
        ],
      ],
    );
  if (tab === "hreflang")
    return table(["page", "lang", "target", "target_status", "source_canonical", "return_link", "canonical_context", "pagination_candidate", "canonical_content_equal"], []);
  const links = data(id, "links");
  return table([...columns, "internal_inlinks", "external_inlinks", "internal_outlinks", "external_outlinks"], rows.map(row => {
    const incoming = links.rows.filter(link => link[1] === row[0]);
    const outgoing = links.rows.filter(link => link[0] === row[0]);
    return [...row, incoming.filter(link => link[7] === true).length, incoming.filter(link => link[7] === false).length, outgoing.filter(link => link[8] === true).length, outgoing.filter(link => link[8] === false).length];
  }));
}

const demoDuplicateColumns: Partial<Record<DuplicateKind, number>> = {title:5,description:8,h1:10};
function duplicateRows(id: string, url: string, kind: DuplicateKind) {
  const column = demoDuplicateColumns[kind];
  if (column === undefined) return [];
  const eligible = (row: Cell[]) => row[1]==="html" && row[2]===200 && row[4]==="indexable" && String(row[column]??"").trim()!=="";
  const rows = byCrawl.get(id) ?? [];
  const source = rows.find((row) => row[0]===url && eligible(row));
  return source ? rows.filter((row) => eligible(row) && row[column]===source[column]) : [];
}
function overview(id: string): Overview {
  const rows = byCrawl.get(id) ?? [];
  const counts = new Map<string, number>();
  rows.forEach((row) =>
    findings(row).forEach((rule) =>
      counts.set(rule, (counts.get(rule) ?? 0) + 1),
    ),
  );
  const links = data(id, "links");
  const scored = new Map(rows.map(row=>[String(row[0]),{rules:findings(row),analyzed:Number(row[2])===200,limited:row[2]===null}]));
  for (const issue of ["links-to-broken", "links-to-broken-external", "links-to-redirects"]) {
    const affected = new Set(links.rows.filter(row => linkEvidenceMatches(links, row, issue)).map(row => row[0]));
    if (affected.size) counts.set(issue, affected.size);
    for(const source of affected)scored.get(String(source))?.rules.push(issue);
  }
  return {
    seo_score: demoSeoScore([...scored.values()]),
    internal: rows.length,
    external: 0,
    indexable: rows.filter((r) => r[4] === "indexable").length,
    html: rows.length,
    blocked: 0,
    errors: rows.filter((r) => Number(r[2]) >= 400).length,
    redirects: rows.filter((r) => r[2] === 301).length,
    responses: responseGroups.map(({group}) => ({group, count: rows.filter((row) => responseGroup(row[2] ?? null, row[3] ?? null, false) === group).length})),
    average_response_ms:
      rows.reduce((s, r) => s + Number(r[18]), 0) / (rows.length || 1),
    issues: [...counts].map(([id, count]) => ({ id, count })),
  };
}
function linkEvidenceMatches(table: TablePage, row: Cell[], issue: string) {
  const value = (name: string) => row[table.columns.findIndex(column => column.name === name)];
  if (value("kind") !== "hyperlink") return false;
  const status = Number(value("destination_status"));
  if (issue === "links-to-redirects") return [301, 302, 303, 307, 308].includes(status);
  return value("destination_internal") === (issue === "links-to-broken") && (status >= 400 || value("destination_error") != null);
}
const emit = (job: Job) => {
  job.revision = (job.revision ?? 0) + 1;
  window.dispatchEvent(new CustomEvent("spider-demo-progress", { detail: structuredClone(job) }));
};

function demoActivity(rows: Cell[][], completed: number, threads: number) {
  const entry = (row: Cell[], index: number, active = false): UrlActivity => ({
    id: index, url: String(row[0]), kind: "page",
    state: active ? "processing" : Number(row[2]) >= 400 ? "error" : "completed",
    started_ms: index * 125, elapsed_ms: active ? 180 : Number(row[18]),
    status: active ? null : Number(row[2]), title: active ? null : String(row[5]),
    error: null, rendered: false, render_error: null,
    response_ms: active ? null : Number(row[18]), size_bytes: active ? null : Number(row[17]),
  });
  return {
    active: rows.slice(completed, completed + threads).map((row, i) => entry(row, completed + i, true)),
    recent: rows.slice(Math.max(0, completed - 100), completed).map((row, i) => entry(row, Math.max(0, completed - 100) + i)).reverse(),
    rendered: 0, render_errors: 0, blocked: 0,
    request_errors: rows.slice(0, completed).filter((row) => Number(row[2]) >= 400).length,
  };
}
export async function demo(
  command: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  if(command==="schema_test")throw new Error("Schema validation is available in the desktop app; no preview result is fabricated.");
  if(command==="sitemap_plan")return demoSitemap(String(args.id),args.selection as import("./types").SitemapSelection).plan;
  if(command.startsWith("finding_comparison_"))return demoFindingComparison(command,args);
  if(command.startsWith("comparison_")) return demoComparison(command,args);
  const id = String(args.id ?? "");
  if (command === "list_crawls") return [...crawls];
  // Browser samples have no saved database files; never invent a disk measurement.
  if (command === "crawl_storage") return crawls.map(crawl => ({ id: crawl.id, bytes: null }));
  if (command === "list_jobs") return structuredClone([...jobs.values()]);
  if (command === "crawl_overview") return overview(id);
  if (command === "score_remediation") {
    const request=args.request as import("./types").RemediationRequest;
    if(!issueById.has(request.issue_id))throw new Error("Unknown remediation check");
    const raw=byCrawl.get(id)??[], links=data(id,"links");
    const affected=new Set(raw.filter(row=>findings(row).includes(request.issue_id)).map(row=>String(row[0])));
    if(isLinkEvidenceIssue(request.issue_id)) for(const row of links.rows)if(linkEvidenceMatches(links,row,request.issue_id))affected.add(String(row[0]));
    const eligible=[...affected].filter(url=>url.toLowerCase().includes(request.search.toLowerCase())).sort();
    const addresses=eligible.slice(request.offset,request.offset+Math.min(50,Math.max(1,request.limit)));
    if(request.url&&!addresses.includes(request.url))throw new Error("Selected URL is not on this scored page");
    const selected=request.url||addresses[0]||null, source=raw.find(row=>row[0]===selected);
    const aliases:Record<string,string>={address:"url",content:"content_kind",status_code:"status",response_time_ms:"response_ms",crawl_depth:"depth"};
    const facts=table(columns.map(name=>aliases[name]??name),source?[source]:[]);
    let evidence=table([],[]),kind="facts";
    if(selected&&isLinkEvidenceIssue(request.issue_id)) {kind="links";evidence={...links,rows:links.rows.filter(row=>row[0]===selected&&linkEvidenceMatches(links,row,request.issue_id))};evidence.total=evidence.rows.length;}
    if(selected&&request.issue_id==="title-duplicate") {kind="duplicates";evidence=table(["address","matched_value"],raw.filter(row=>row[0]!==selected&&row[5]===source?.[5]).map(row=>[row[0]??null,row[5]??null]));}
    evidence={...evidence,rows:evidence.rows.slice(request.evidence_offset,request.evidence_offset+50)};
    return {urls:table(["address"],addresses.map(url=>[url]),eligible.length),selected_url:selected,facts,evidence,evidence_kind:kind};
  }
  if (command === "crawl_page") {
    const request = args.request as PageRequest;
    const result = data(id, request.tab);
    let rows = result.rows;
    const index = (name: string) =>
      result.columns.findIndex((c) => c.name === name);
    const address = Math.max(0, ["address", "url", "page", "source", "image"].map(index).find((i) => i >= 0) ?? 0);
    const scope = request.link_scope;
    if (request.response_group && (request.tab !== "response-codes" || !responseGroups.some(({group}) => group === request.response_group))) throw new Error("A valid response group requires the response-codes tab.");
    const duplicateScope = request.duplicate_scope;
    if (duplicateScope && (request.tab !== "internal" || !duplicateScope.url || !["title","description","h1","h2","content"].includes(duplicateScope.kind))) throw new Error("A duplicate scope requires the internal tab, a nonempty URL and a valid kind.");
    const duplicateUrls = duplicateScope ? new Set(duplicateRows(id, duplicateScope.url, duplicateScope.kind).map((row) => row[0])) : null;
    if (scope && (request.tab !== "links" || !scope.url || !["incoming", "outgoing"].includes(scope.direction)))
      throw new Error("A link scope requires the links tab, a nonempty URL and a valid direction.");
    if (request.link_class && (request.tab !== "links" || !["internal", "external"].includes(request.link_class))) throw new Error("A link classification requires the links tab.");
    const source = index("source"), destination = index("destination");
    const searchColumns = request.tab === "links"
      ? scope ? [scope.direction === "incoming" ? source : destination] : [source, destination]
      : request.tab === "sitemap-urls" ? [index("url"),index("crawl_url"),index("sitemap")] : request.tab === "sitemaps" ? [index("url"),index("redirect_url"),index("redirect_location"),index("final_url")] : [address];
    const status = ["status_code", "destination_status"].map(index).find((i) => i >= 0) ?? -1;
    const indexability = index("indexability");
    rows = rows.filter(
      (r) =>
        (!request.response_group || responseGroup(r[status] ?? null, r[index("error")] ?? null, r[index("blocked_by_robots")] ?? null) === request.response_group) &&
        (!duplicateUrls || duplicateUrls.has(r[address])) &&
        (!scope || r[scope.direction === "incoming" ? destination : source] === scope.url) &&
        (!request.link_class || r[index(scope?.direction === "incoming" ? "source_internal" : "destination_internal")] === (request.link_class === "internal")) &&
        (!(request.tab === "links" && isLinkEvidenceIssue(request.issue_id)) || linkEvidenceMatches(result, r, request.issue_id)) &&
        (!request.search ||
          searchColumns.some((i) => String(r[i] ?? "")
            .toLowerCase()
            .includes(request.search.toLowerCase()))) &&
        (status < 0 || request.status_class < 2 || request.status_class > 5 ||
          Math.floor(Number(r[status]) / 100) === request.status_class) &&
        (indexability < 0 || !request.indexability || r[indexability] === request.indexability) &&
        (!request.issue_id || (request.tab === "links" && isLinkEvidenceIssue(request.issue_id)) ||
          findings(
            (byCrawl.get(id) ?? []).find((row) => row[0] === r[request.tab === "sitemap-urls" ? index("crawl_url") : address]) ?? [],
          ).includes(request.issue_id)),
    );
    const sort = index(request.sort);
    if (sort >= 0)
      rows = [...rows].sort((a, b) => {
        const x = a[sort],
          y = b[sort];
        return (
          (typeof x === "number" && typeof y === "number"
            ? x - y
            : String(x ?? "").localeCompare(String(y ?? ""))) *
          (request.descending ? -1 : 1)
        );
      });
    return {
      ...result,
      total: rows.length,
      rows: rows.slice(request.offset, request.offset + request.limit),
      ...(request.tab === "links" ? { link_summary: { sources: new Set(rows.map(row => row[source])).size, destinations: new Set(rows.map(row => row[destination])).size } } : {}),
    };
  }
  if (command === "url_detail") {
    const row = (byCrawl.get(id) ?? []).find((r) => r[0] === args.url);
    const links = data(id, "links");
    const incoming = links.rows.filter((r) => r[1] === args.url),
      outgoing = links.rows.filter((r) => r[0] === args.url);
    return {
      facts: table(columns, row ? [row] : []),
      findings: table(["issue_id"], row ? findings(row).sort().map((id) => [id]) : []),
      inlinks: { ...links, rows: incoming, total: incoming.length },
      outlinks: { ...links, rows: outgoing, total: outgoing.length },
      link_counts: {
        incoming: { internal: incoming.filter(row => row[7] === true).length, external: incoming.filter(row => row[7] === false).length },
        outgoing: { internal: outgoing.filter(row => row[8] === true).length, external: outgoing.filter(row => row[8] === false).length },
      },
      headers: table(
        ["name", "value"],
        [
          ["content-type", "text/html; charset=utf-8"],
          ["cache-control", "public, max-age=3600"],
          ["server", "nginx"],
        ],
      ),
      html: null,
      raw_html: null,
    };
  }
  if (command === "url_redirects") {
    const url = String(args.url ?? "");
    if (!url) throw new Error("Empty redirect URL.");
    const rows = byCrawl.get(id) ?? [];
    const row = rows.find((row) => row[0] === url);
    const target = row?.[2] === 301 ? rows[0] : undefined;
    const hop = (url: string, row?: Cell[]) => ({ url, status: row ? Number(row[2]) : null, location: null as string | null, error: null, blocked: false, recorded: !!row });
    const first = hop(url, row);
    const trace: RedirectTrace = { hops: [first], outcome: !row ? "unchecked" : Number(row[2]) >= 400 ? "http-error" : "complete" };
    if (target) {
      first.location = String(target[0]);
      trace.hops.push(hop(first.location, target));
    }
    return trace;
  }
  if (command === "url_content_match") return null;
  if (command === "url_duplicate_groups") {
    if (!args.url) throw new Error("Empty duplicate group URL.");
    const groups: DuplicateGroup[] = [];
    for (const kind of Object.keys(demoDuplicateColumns) as DuplicateKind[]) {
      const rows = duplicateRows(id,String(args.url??""),kind);
      if (rows.length>1) {
        const value = String(rows[0]![demoDuplicateColumns[kind]!]??"");
        const chars = Array.from(value);
        groups.push({kind,value:chars.slice(0,512).join(""),value_truncated:chars.length>512,count:rows.length});
      }
    }
    return groups;
  }
  if (command === "delete_crawl") {
    crawls = crawls.filter((c) => c.id !== id);
    byCrawl.delete(id);
    return;
  }
  if (command === "control_crawl") {
    const job = jobs.get(String(args.jobId));
    if (job) {
      job.status =
        args.action === "pause"
          ? "paused"
          : args.action === "resume"
            ? "running"
            : "stopping";
      job.progress.paused = job.status === "paused";
      emit(job);
    }
    return;
  }
  if (command === "start_crawl") {
    const config = args.config as CrawlConfig;
    const id = `demo-${Date.now()}`;
    const host = new URL(config.start_urls[0] ?? "https://atelier.example")
      .host;
    const total = config.limits.max_urls
      ? Math.min(96, config.limits.max_urls)
      : 96;
    const job: Job = {
      id,
      url: config.start_urls[0] ?? "",
      rendering: config.rendering.mode,
      status: "running",
      crawl_id: id,
      phase: "crawling",
      revision: 0,
      activity: demoActivity(records(host, total), 0, config.speed.threads),
      progress: {
        ...fixture.progress,
        crawled: 0,
        queued: total,
        discovered: total,
        in_flight: Math.min(total, config.speed.threads),
        elapsed_ms: 0,
        errors: 0,
        paused: false,
      },
    };
    jobs.set(id, job);
    job.progress.queued -= job.progress.in_flight;
    emit(job);
    const timer = window.setInterval(() => {
      if (job.status === "paused") return;
      job.progress = {
        ...job.progress,
        crawled: Math.min(total, job.progress.crawled + 4),
        elapsed_ms: job.progress.elapsed_ms + 500,
      };
      job.activity = demoActivity(records(host, total), job.progress.crawled, config.speed.threads);
      job.progress.errors = job.activity.request_errors;
      job.progress.in_flight = job.activity.active.length;
      job.progress.queued = total - job.progress.crawled - job.progress.in_flight;
      if (job.progress.crawled === total || job.status === "stopping") {
        clearInterval(timer);
        job.phase = "finalizing";
        job.activity.active = [];
        job.progress.in_flight = 0;
        const meta: CrawlMeta = {
          ...fixture,
          id,
          name: host,
          config,
          start_urls: config.start_urls,
          created_at_ms: Date.now() - job.progress.elapsed_ms,
          finished_at_ms: Date.now(),
          status: job.status === "stopping" ? "stopped" : "completed",
          progress: job.progress,
        };
        byCrawl.set(id, records(host, job.progress.crawled));
        meta.progress.errors = overview(id).errors;
        crawls = [meta, ...crawls];
        job.meta = meta;
        job.status = meta.status;
      }
      emit(job);
    }, 500);
    return id;
  }
  throw new Error(`Unsupported demo command: ${command}`);
}
export async function exportDemoTable(id: string, tab: string, selection?: TableExportRequest): Promise<TablePage> {
  if (!selection) return data(id, tab);
  if (selection.request.tab !== tab) throw new Error("Export selection does not match the requested tab.");
  const result = await demo("crawl_page", { id, request: { ...selection.request, offset: 0, limit: Number.MAX_SAFE_INTEGER } }) as TablePage;
  if (!selection.columns.length) return result;
  const indexes = selection.columns.map((name) => result.columns.findIndex((column) => column.name === name));
  if (indexes.some((index) => index < 0) || new Set(indexes).size !== indexes.length) throw new Error("Unknown or duplicate export column.");
  return { ...result, columns: indexes.map((index) => result.columns[index]!), rows: result.rows.map((row) => indexes.map((index) => row[index] ?? null)) };
}
export async function exportDemo(id: string, tab: string, format: string, selection?: TableExportRequest) {
  const result = await exportDemoTable(id, tab, selection);
  return downloadDemo(result,tab,format);
}

function downloadDemo(result:TablePage,tab:string,format:string) {
  if (format === "xlsx")
    throw new Error("XLSX export is available in the desktop app.");
  const cell = (v: Cell) =>
    `"${String(v ?? "")
      .replaceAll('"', '""')
      .replace(/^[=+@-]/, "'$&")}"`;
  const content =
    format === "json"
      ? JSON.stringify(
          result.rows.map((row) =>
            Object.fromEntries(result.columns.map((c, i) => [c.name, row[i]])),
          ),
          null,
          2,
        )
      : [
          result.columns.map((c) => cell(c.name)).join(","),
          ...result.rows.map((r) => r.map(cell).join(",")),
        ].join("\r\n");
  const url = URL.createObjectURL(
    new Blob([content], {
      type: format === "json" ? "application/json" : "text/csv;charset=utf-8",
    }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${tab}.${format}`;
  anchor.click();
  URL.revokeObjectURL(url);
  return result.rows.length;
}

// The browser comparison uses only fields actually present in its sample records.
const demoFields=["content","status_code","error","indexability","title","title_count","meta_description","h1","h1_count","canonical","crawl_depth","inlinks","outlinks","word_count"];
function demoComparison(command:string,args:Record<string,unknown>):unknown {
  const baseline=String(args.baseline),current=String(args.current);
  if(baseline===current) throw new Error("Choose two different crawls.");
  for(const id of [baseline,current]) if(!crawls.some(c=>c.id===id&&c.status!=="running")) throw new Error("Choose two finished crawls.");
  const before=new Map((byCrawl.get(baseline)??[]).map(row=>[String(row[0]),row]));
  const after=new Map((byCrawl.get(current)??[]).map(row=>[String(row[0]),row]));
  const value=(row:Cell[]|undefined,field:string)=>{const v=row?.[columns.indexOf(field)];return v==null?null:String(v);};
  const rows=[...new Set([...before.keys(),...after.keys()])].sort().map(url=>{
    const b=before.get(url),a=after.get(url),changed=demoFields.filter(f=>value(b,f)!==value(a,f));
    return {url,b,a,changed:b&&a?changed:[],kind:!b?"new":!a?"missing":changed.length?"changed":"unchanged"};
  });
  if(command==="comparison_summary") {
    const request=args.request as ComparisonRequest|undefined;
    if(request?.field&&!demoFields.includes(request.field))throw new Error("Unknown comparison field.");
    const matching=rows.filter(row=>(!request?.field||row.changed.includes(request.field))&&row.url.toLowerCase().includes((request?.search??"").toLowerCase()));
    return {new:matching.filter(r=>r.kind==="new").length,missing:matching.filter(r=>r.kind==="missing").length,changed:matching.filter(r=>r.kind==="changed").length,unchanged:matching.filter(r=>r.kind==="unchanged").length,fields:demoFields.map(field=>({field,count:matching.filter(r=>r.changed.includes(field)).length})),warnings:[]};
  }
  if(command==="comparison_detail") {
    const row=rows.find(row=>row.url===String(args.url));
    return table(["field","before_value","after_value","changed","in_baseline","in_current"],row?demoFields.map(f=>[f,value(row.b,f),value(row.a,f),row.changed.includes(f),!!row.b,!!row.a]):[]);
  }
  const request=args.request as ComparisonRequest;
  if(request.field&&!demoFields.includes(request.field)) throw new Error("Unknown comparison field.");
  const matching=rows.filter(r=>(request.filter==="all"||request.filter===r.kind)&&(!request.field||r.changed.includes(request.field))&&r.url.toLowerCase().includes(request.search.toLowerCase()));
  const names=["address","change","changed_fields","before_status_code","after_status_code","before_indexability","after_indexability","before_title","after_title"];
  if(command==="comparison_page") return table(names,matching.slice(request.offset,request.offset+request.limit).map(r=>[r.url,r.kind,r.changed.join(", "),value(r.b,"status_code"),value(r.a,"status_code"),value(r.b,"indexability"),value(r.a,"indexability"),value(r.b,"title"),value(r.a,"title")]),matching.length);
  return table(["address","in_baseline","in_current","change","changed_fields",...demoFields.flatMap(f=>[`before_${f}`,`after_${f}`])],matching.map(r=>[r.url,!!r.b,!!r.a,r.kind,r.changed.join(", "),...demoFields.flatMap(f=>[value(r.b,f),value(r.a,f)])]));
}
export function exportDemoComparison(baseline:string,current:string,request:ComparisonRequest,format:string) {
  return downloadDemo(demoComparison("comparison_export",{baseline,current,request}) as TablePage,"crawl-comparison",format);
}

function demoSitemap(id:string,selection:import("./types").SitemapSelection) {
  const meta=crawls.find(c=>c.id===id);
  if(!meta||meta.status==="running")throw new Error("Choose a finished crawl.");
  const origins=new Map<string,number>();
  const records=(byCrawl.get(id)??[]).map(row=>{
    const address=String(row[0]??"");let reason:import("./types").SitemapReason|undefined;
    let origin="";
    if(Number(row[2])!==200)reason="response";
    else if(row[1]!=="html")reason="non-html";
    else if(row[4]==="noindex")reason="noindex";
    else if(meta.config.rendering?.mode==="javascript")reason="rendering";
    else if(row[12]&&String(row[12])!==address)reason="canonical";
    else if(row[4]!=="indexable")reason="non-indexable";
    try {const url=new URL(address);origin=url.origin;if(!["http:","https:"].includes(url.protocol)||url.username||url.password||url.hash||url.href!==address||address.length>=2048)reason??="invalid-url";}catch{reason??="invalid-url";}
    if(!reason)origins.set(origin,(origins.get(origin)??0)+1);
    return {address,origin,reason};
  });
  if(selection.origin) {const parsed=new URL(selection.origin+"/");if(parsed.origin!==selection.origin||!["http:","https:"].includes(parsed.protocol)||parsed.username||parsed.password)throw new Error("Choose a valid sitemap origin.");}
  const origin=selection.origin||[...origins].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]))[0]?.[0]||"";
  if(origin&&!origins.has(origin))origins.set(origin,0);
  const excluded=new Map<import("./types").SitemapReason,number>();
  const urls:string[]=[];
  for(const record of records) {
    const reason=record.reason||(record.origin!==origin?"other-origin":undefined);
    if(reason)excluded.set(reason,(excluded.get(reason)??0)+1);else urls.push(record.address);
  }
  urls.sort();
  const xml=sitemapXml(urls);
  const plan:import("./types").SitemapPlan={origin,origins:[...origins].sort(([a],[b])=>a.localeCompare(b)).map(([origin,count])=>({origin,count})),scanned:records.length,included:urls.length,excluded:[...excluded].map(([reason,count])=>({reason,count})),sample:urls.slice(0,100),parts:urls.length?1:0,xml_bytes:urls.length?new TextEncoder().encode(xml).length:0,warnings:[...(meta.status!=="completed"||meta.limit_reached?["partial"]:[]),...(meta.config.mode==="list"?["list-mode"]:[])]};
  return {plan,xml};
}
export function exportDemoSitemap(id:string,selection:import("./types").SitemapSelection,format:"xml"|"zip") {
  if(format!=="xml")throw new Error("Sitemap ZIP packages are available in the desktop app.");
  const {plan,xml}=demoSitemap(id,selection);
  if(!plan.included)throw new Error("No eligible sitemap URLs.");
  const url=URL.createObjectURL(new Blob([xml],{type:"application/xml;charset=utf-8"}));
  const anchor=document.createElement("a");anchor.href=url;anchor.download="sitemap.xml";anchor.click();URL.revokeObjectURL(url);
  return {urls:plan.included,parts:1};
}

function demoFindingComparison(command:string,args:Record<string,unknown>):unknown {
  const baseline=crawls.find(c=>c.id===args.baseline),current=crawls.find(c=>c.id===args.current);
  if(!baseline||!current||baseline.id===current.id||baseline.status==="running"||current.status==="running")throw new Error("Choose two different finished crawls.");
  const before=new Map((byCrawl.get(baseline.id)??[]).map(row=>[String(row[0]),row]));
  const after=new Map((byCrawl.get(current.id)??[]).map(row=>[String(row[0]),row]));
  const pairs=new Map<string,{id:string;url:string;before:boolean;after:boolean}>();
  for(const [records,side] of [[before,"before"],[after,"after"]] as const) for(const [url,row] of records) for(const id of findings(row)) {
    const key=JSON.stringify([id,url]),pair=pairs.get(key)??{id,url,before:false,after:false};pair[side]=true;pairs.set(key,pair);
  }
  const compatible=baseline.status==="completed"&&current.status==="completed"&&!baseline.limit_reached&&!current.limit_reached&&baseline.config.mode!=="list"&&current.config.mode!=="list"&&JSON.stringify(baseline.config)===JSON.stringify(current.config)&&JSON.stringify(baseline.start_urls)===JSON.stringify(current.start_urls);
  const http=new Set(["internal-no-response","external-no-response","internal-client-error","external-client-error","internal-server-error","external-server-error","internal-redirect","internal-redirect-temporary","internal-blocked-robots"]);
  const columns=["address","issue_id","finding_change","comparison_reason","priority","category","before_finding","after_finding","before_status_code","after_status_code","before_indexability","after_indexability","before_error","after_error","before_render_error","after_render_error"];
  const rows:Cell[][]=[...pairs.values()].map(pair=>{
    const b=before.get(pair.url),a=after.get(pair.url),rule=issueById.get(pair.id);
    let reason="";let state:import("./types").FindingChange="persisting";
    if(!(pair.before&&pair.after)) {
      if(!b&&a&&pair.after){reason="url-only-current";state="new-url";}
      else if(!a&&b&&pair.before){reason="url-only-baseline";state="missing-url";}
      else {
        const observation=pair.after?b:a,side=pair.after?"before":"after",config=pair.after?baseline.config:current.config;
        if(!a||!b)reason="no-url-observation";
        else if(!rule)reason="unknown-rule";
        else if(!compatible)reason="scope-not-comparable";
        else if(observation?.[2]==null)reason=side+"-http-unobserved";
        else if(!http.has(pair.id)) {
          if(observation?.[2]!==200||observation?.[1]!=="html")reason=side+"-not-html-200";
          else if(observation?.[4]!=="indexable")reason=side+"-not-indexable";
          else if(config.rendering?.mode==="javascript")reason=side+"-rendering-unverified";
        }
        state=reason?"unverified":pair.after?"introduced":"not-detected";
      }
    }
    return [pair.url,pair.id,state,reason,rule?.priority??"unknown",rule?.category??"unknown",pair.before,pair.after,b?.[2]==null?null:String(b[2]),a?.[2]==null?null:String(a[2]),b?.[4]??null,a?.[4]??null,null,null,null,null];
  });
  const priorities=["high","medium","low","unknown"],states=["introduced","not-detected","unverified","new-url","missing-url","persisting"];
  rows.sort((a,b)=>priorities.indexOf(String(a[4]))-priorities.indexOf(String(b[4]))||states.indexOf(String(a[2]))-states.indexOf(String(b[2]))||String(a[1]).localeCompare(String(b[1]))||String(a[0]).localeCompare(String(b[0])));
  const counts=(rows:Cell[][],id="",priority="",category=""):import("./types").FindingRuleCount=>({id,priority,category,total:rows.length,introduced:rows.filter(r=>r[2]==="introduced").length,not_detected:rows.filter(r=>r[2]==="not-detected").length,persisting:rows.filter(r=>r[2]==="persisting").length,new_url:rows.filter(r=>r[2]==="new-url").length,missing_url:rows.filter(r=>r[2]==="missing-url").length,unverified:rows.filter(r=>r[2]==="unverified").length});
  const request=args.request as import("./types").FindingComparisonRequest|undefined;
  if(request&&!["",...priorities].includes(request.priority))throw new Error("Unknown finding priority.");
  const filtered=rows.filter(row=>(!request?.issue_id||row[1]===request.issue_id)&&(!request?.priority||row[4]===request.priority)&&String(row[0]).toLowerCase().includes((request?.search??"").toLowerCase()));
  if(command==="finding_comparison_summary") {
    const ids=[...new Set(rows.map(r=>String(r[1])))].sort((a,b)=>priorities.indexOf(issueById.get(a)?.priority??"unknown")-priorities.indexOf(issueById.get(b)?.priority??"unknown")||a.localeCompare(b));
    return {counts:counts(filtered),baseline:filtered.filter(r=>r[6]===true).length,current:filtered.filter(r=>r[7]===true).length,rules:ids.slice(0,500).map(id=>counts(rows.filter(r=>r[1]===id),id,issueById.get(id)?.priority??"unknown",issueById.get(id)?.category??"unknown")),rule_total:ids.length,warnings:["finding-observations",...(!compatible?["settings-differ"]:[])]};
  }
  if(!request)throw new Error("Missing finding request.");
  const matching=filtered.filter(row=>request.change==="all"||row[2]===request.change);
  return table(columns,command==="finding_comparison_page"?matching.slice(request.offset,request.offset+request.limit):matching,matching.length);
}
export function exportDemoFindingComparison(baseline:string,current:string,request:import("./types").FindingComparisonRequest,format:string) {
  return downloadDemo(demoFindingComparison("finding_comparison_export",{baseline,current,request}) as TablePage,"finding-comparison",format);
}
