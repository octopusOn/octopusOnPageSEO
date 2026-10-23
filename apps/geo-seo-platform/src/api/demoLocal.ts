// Sample local SEO for the interface outside the program: the first sample project's shoe shop
// in Kadıköy, its Maps grid for three keywords over several weeks, the shops around it and its
// reviews.

import { CallError } from "./transport";
import type {
  AroundBusiness,
  Business,
  BusinessProfile,
  Estimate,
  Fetched,
  GridCell,
  GridEstimate,
  GridPoint,
  GridSummary,
  LocalCompetitors,
  LocalGrid,
  LocalReview,
  LocalRun,
  LocalSchedule,
  LocalSetup,
  LocalTrendPoint,
  MapPlace,
  RatingDay,
  ReviewsView,
} from "./types";

const ago = (days: number, hour = 8) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 11) + `${String(hour).padStart(2, "0")}:00:00Z`;

const SHOP: Business = {
  title: "Örnek Mağaza Kadıköy",
  cid: "222222222222222222",
  place_id: "ChIJornek",
  category: "Ayakkabı mağazası",
  address: "Bahariye Cd. 20, 34714 Kadıköy/İstanbul",
  domain: "ornekmagaza.com",
  url: "https://ornekmagaza.com/magazalar/kadikoy",
  phone: "+90 216 000 00 02",
  lat: 40.9878,
  lng: 29.0286,
  rating: 4.6,
  votes: 812,
  is_claimed: true,
};

interface State {
  business: Business | null;
  keywords: string[];
  grid: number;
  radius_km: number;
  schedule: LocalSchedule;
  last_run_at: string | null;
}

const states: Record<number, State> = {
  1: { business: SHOP, keywords: ["ayakkabı mağazası", "spor ayakkabı", "koşu ayakkabısı"], grid: 7, radius_km: 3, schedule: "weekly", last_run_at: ago(2) },
};

function stateOf(projectId: number): State {
  return (states[projectId] ??= { business: null, keywords: [], grid: 5, radius_km: 2, schedule: "manual", last_run_at: null });
}

/** The points of a grid, as the program lays them out. */
function gridOf(lat: number, lng: number, size: number, radius: number): GridPoint[] {
  const half = (size - 1) / 2;
  const step = radius / half;
  const points: GridPoint[] = [];
  for (let row = 0; row < size; row += 1) {
    for (let col = 0; col < size; col += 1) {
      const north = (half - row) * step;
      const east = (col - half) * step;
      points.push({ row, col, lat: lat + north / 111.32, lng: lng + east / (111.32 * Math.cos((lat * Math.PI) / 180)) });
    }
  }
  return points;
}

const zoomFor = (radius: number) => (radius <= 1 ? 16 : radius <= 3 ? 15 : radius <= 7 ? 14 : radius <= 15 ? 13 : 12);

function setupOf(projectId: number): LocalSetup {
  const state = stateOf(projectId);
  return {
    business: state.business,
    keywords: state.keywords,
    grid: state.grid,
    radius_km: state.radius_km,
    schedule: state.schedule,
    last_run_at: state.last_run_at,
    points: state.business ? gridOf(state.business.lat, state.business.lng, state.grid, state.radius_km) : [],
    zoom: zoomFor(state.radius_km),
    next_run_at: state.schedule === "manual" || !state.business ? null : ago(-5),
    waiting_reviews: 0,
  };
}

const RIVALS: MapPlace[] = [
  { position: 0, title: "Sporium Kadıköy", cid: "111", rating: 4.4, votes: 2310 },
  { position: 0, title: "Adım Ayakkabı", cid: "333", rating: 4.7, votes: 640 },
  { position: 0, title: "Koşu Dünyası Moda", cid: "444", rating: 4.5, votes: 980 },
  { position: 0, title: "Büyük Pazar Ayakkabı", cid: "555", rating: 4.1, votes: 3920 },
];

/** The shop's place at a point: good near it, weaker and then missing further away; the run's age
 * and the keyword move it a little. */
function positionAt(point: GridPoint, size: number, keyword: number, age: number): number | null {
  const half = (size - 1) / 2;
  const distance = Math.hypot(point.row - half, point.col - half) / Math.max(1, half);
  const westward = point.col < half ? 0.6 : 0;
  const raw = 1 + distance * (5 + keyword * 4) + westward * keyword + age * 0.35 + ((point.row * 3 + point.col + keyword) % 3) * 0.5;
  const position = Math.round(raw);
  return position > 20 ? null : position;
}

function cellsOf(points: GridPoint[], size: number, keyword: number, age: number): GridCell[] {
  return points.map((point) => {
    const position = positionAt(point, size, keyword, age);
    const others = RIVALS.filter((_, index) => (index + point.row + point.col + keyword) % 4 !== 3);
    const top: MapPlace[] = [];
    let next = 0;
    for (let place = 1; place <= 3; place += 1) {
      if (place === position) top.push({ position: place, title: SHOP.title, cid: SHOP.cid, rating: SHOP.rating, votes: SHOP.votes });
      else {
        const rival = others[next % others.length]!;
        next += 1;
        top.push({ ...rival, position: place });
      }
    }
    return { ...point, position, top, error: null };
  });
}

function summarize(positions: (number | null)[]): GridSummary {
  const found = positions.filter((value): value is number => value !== null);
  const top3 = found.filter((value) => value <= 3).length;
  const round = (value: number) => Math.round(value * 10) / 10;
  return {
    points: positions.length,
    found: found.length,
    top3,
    arp: found.length ? round(found.reduce((sum, value) => sum + value, 0) / found.length) : null,
    atrp: positions.length ? round(positions.reduce<number>((sum, value) => sum + (value ?? 21), 0) / positions.length) : null,
    solv: positions.length ? round((top3 / positions.length) * 100) : 0,
  };
}

/** Weekly runs, the newest first: the shop climbs over the weeks. */
const RUN_AGES = [2, 9, 16, 23, 30];

function runsOf(projectId: number): LocalRun[] {
  const state = stateOf(projectId);
  if (!state.business || projectId !== 1) return [];
  const total = state.keywords.length * state.grid * state.grid;
  return RUN_AGES.map((days, index) => ({
    id: 40 - index,
    project_id: projectId,
    scheduled: index !== 3,
    status: "done",
    total,
    checked: total,
    failed: 0,
    cost_usd: Math.round(total * (index === 3 ? 0.002 : 0.0006) * 10_000) / 10_000,
    error: null,
    started_at: ago(days),
    finished_at: ago(days, 9),
    grid: state.grid,
    radius_km: state.radius_km,
    lat: state.business!.lat,
    lng: state.business!.lng,
    waiting: 0,
  }));
}

function gridReport(projectId: number, runId: number | null): LocalGrid {
  const state = stateOf(projectId);
  const runs = runsOf(projectId);
  const run = runs.find((entry) => entry.id === runId) ?? runs[0] ?? null;
  if (!run || !state.business) return { run: null, keywords: [], summary: summarize([]), rivals: [], trend: [], runs };
  const points = gridOf(run.lat, run.lng, run.grid, run.radius_km);
  const age = (entry: LocalRun) => runs.indexOf(entry);
  const keywords = state.keywords.map((keyword, index) => {
    const cells = cellsOf(points, run.grid, index, age(run));
    return { keyword, summary: summarize(cells.map((cell) => cell.position)), cells };
  });
  const tally = new Map<string, { place: MapPlace; count: number; sum: number }>();
  for (const cell of keywords.flatMap((entry) => entry.cells)) {
    for (const place of cell.top) {
      if (place.cid === SHOP.cid) continue;
      const entry = tally.get(place.title) ?? { place, count: 0, sum: 0 };
      entry.count += 1;
      entry.sum += place.position;
      tally.set(place.title, entry);
    }
  }
  const trend: LocalTrendPoint[] = [...runs].reverse().flatMap((entry) =>
    state.keywords.map((keyword, index) => {
      const figures = summarize(cellsOf(points, entry.grid, index, age(entry)).map((cell) => cell.position));
      return { run_id: entry.id, started_at: entry.started_at, keyword, arp: figures.arp, atrp: figures.atrp, solv: figures.solv };
    }),
  );
  return {
    run,
    keywords,
    summary: summarize(keywords.flatMap((entry) => entry.cells.map((cell) => cell.position))),
    rivals: [...tally.values()]
      .map(({ place, count, sum }) => ({ title: place.title, cid: place.cid, top3: count, average: Math.round((sum / count) * 10) / 10, rating: place.rating, votes: place.votes }))
      .sort((left, right) => right.top3 - left.top3),
    trend,
    runs,
  };
}

const PROFILE: BusinessProfile = {
  business: SHOP,
  description: "Kadıköy'ün merkezinde spor, koşu ve günlük ayakkabılar; ücretsiz ayak analizi.",
  additional_categories: ["Spor malzemeleri mağazası", "Koşu mağazası"],
  rating_distribution: { "1": 24, "2": 18, "3": 41, "4": 146, "5": 583 },
  total_photos: 214,
  main_image: null,
  price_level: "₺₺",
  hours: [
    { day: "monday", hours: ["10:00–21:00"] },
    { day: "tuesday", hours: ["10:00–21:00"] },
    { day: "wednesday", hours: ["10:00–21:00"] },
    { day: "thursday", hours: ["10:00–21:00"] },
    { day: "friday", hours: ["10:00–22:00"] },
    { day: "saturday", hours: ["10:00–22:00"] },
    { day: "sunday", hours: ["12:00–20:00"] },
  ],
  status: "opened",
  attributes: ["has_in_store_pickup", "has_delivery", "has_wheelchair_accessible_entrance", "pay_credit_card"],
  topics: [
    ["ayak analizi", 64],
    ["koşu ayakkabısı", 51],
    ["ilgili personel", 38],
    ["fiyat", 22],
    ["iade", 9],
  ],
  check_url: null,
};

const AROUND: AroundBusiness[] = [
  { ...SHOP, title: "Büyük Pazar Ayakkabı", cid: "555", place_id: null, domain: "buyukpazar.com", rating: 4.1, votes: 3920, address: "Söğütlüçeşme Cd. 70, Kadıköy", own: false, distance_km: 0.9 },
  { ...SHOP, title: "Sporium Kadıköy", cid: "111", place_id: null, domain: "sporium.com.tr", rating: 4.4, votes: 2310, address: "Moda Cd. 5, Kadıköy", own: false, distance_km: 1.2 },
  { ...SHOP, title: "Koşu Dünyası Moda", cid: "444", place_id: null, domain: null, rating: 4.5, votes: 980, address: "Moda Cd. 112, Kadıköy", own: false, distance_km: 1.6, is_claimed: false },
  { ...SHOP, own: true, distance_km: 0 },
  { ...SHOP, title: "Adım Ayakkabı", cid: "333", place_id: null, domain: "adimayakkabi.com", rating: 4.7, votes: 640, address: "Yoğurtçu Parkı Cd. 8, Kadıköy", own: false, distance_km: 0.6 },
  { ...SHOP, title: "Feneryolu Ayakkabıcısı", cid: "666", place_id: null, domain: null, rating: 3.9, votes: 118, address: "Bağdat Cd. 140, Kadıköy", own: false, distance_km: 2.8, is_claimed: false },
];

const REVIEWS: LocalReview[] = [
  ["r1", 5, "Ayak analizi yaptılar, koşu ayakkabımı sabırla birlikte seçtik. Çok memnun kaldım.", "Ayşe Kaya", 1, null, true, 2],
  ["r2", 2, "Beden değişimi için iki kez gelmem gerekti, kasada uzun sıra vardı.", "Mehmet T.", 3, null, false, 0],
  ["r3", 5, "Personel çok ilgili, fiyatlar internetle aynı.", "Zeynep", 6, "Güzel yorumunuz için teşekkür ederiz Zeynep Hanım!", false, 0],
  ["r4", 4, "Geniş model seçeneği var, hafta sonu biraz kalabalık.", "Can Ö.", 9, null, true, 1],
  ["r5", 1, "Aldığım ayakkabının tabanı bir ayda ayrıldı, iade sürecinde zorlandım.", "Burak D.", 12, "Yaşadığınız sorun için özür dileriz; mağaza müdürümüz sizinle iletişime geçti.", false, 0],
  ["r6", 5, null, "Elif", 15, null, false, 0],
  ["r7", 3, "Ürünler güzel ama denemek için yeterli oturma yeri yok.", "Kerem A.", 21, null, false, 0],
  ["r8", 5, "Koşu grubu etkinlikleri harika, herkese tavsiye ederim.", "Deniz", 34, "Bizi tercih ettiğiniz için teşekkürler!", true, 3],
].map(([id, rating, text, author, days, answer, guide, photos]) => ({
  review_id: String(id),
  rating: Number(rating),
  text: text === null ? null : String(text),
  author: String(author),
  author_url: null,
  url: "https://www.google.com/maps",
  published_at: ago(Number(days), 14),
  owner_answer: answer === null ? null : String(answer),
  answered_at: answer === null ? null : ago(Number(days) - 1, 10),
  photos: Number(photos),
  local_guide: Boolean(guide),
  first_seen: ago(Number(days), 15),
}));

const RATINGS: RatingDay[] = [
  [58, 4.4, 702],
  [44, 4.5, 731],
  [30, 4.5, 757],
  [16, 4.5, 779],
  [2, 4.6, 812],
].map(([days, rating, votes]) => ({ day: ago(Number(days)).slice(0, 10), rating: Number(rating), votes: Number(votes) }));

function reviewsOf(projectId: number): ReviewsView {
  if (projectId !== 1) return { reviews: [], summary: { count: 0, average: null, unanswered: 0, unanswered_negative: 0, recent: 0, stars: [0, 0, 0, 0, 0] }, ratings: [], waiting: 0 };
  const stars: [number, number, number, number, number] = [0, 0, 0, 0, 0];
  for (const review of REVIEWS) if (review.rating) stars[review.rating - 1]! += 1;
  const rated = REVIEWS.filter((review) => review.rating !== null);
  return {
    reviews: REVIEWS,
    summary: {
      count: REVIEWS.length,
      average: Math.round((rated.reduce((sum, review) => sum + review.rating!, 0) / rated.length) * 10) / 10,
      unanswered: REVIEWS.filter((review) => !review.owner_answer).length,
      unanswered_negative: REVIEWS.filter((review) => !review.owner_answer && (review.rating ?? 5) <= 3).length,
      recent: REVIEWS.filter((review) => Date.parse(review.published_at!) > Date.now() - 30 * 86_400_000).length,
      stars,
    },
    ratings: RATINGS,
    waiting: 0,
  };
}

const kept = <T>(data: T): Fetched<T> => ({ data, fetched_at: ago(1, 10), fresh: true, cached: true, spent_usd: 0, via: "credit", saved_usd: 0 });

function paidData(projectId: number, args: Record<string, unknown>): unknown {
  switch (args.report) {
    case "find":
      return { ...PROFILE, business: { ...SHOP, title: String(args.query ?? SHOP.title) } };
    case "competitors":
      return { businesses: AROUND, total: 214 } satisfies LocalCompetitors;
    default:
      return stateOf(projectId).business ? PROFILE : null;
  }
}

/** Answers a local SEO command with sample data; `undefined` for other commands. */
export function localDemo(command: string, args: Record<string, unknown>): unknown {
  const projectId = Number(args.project_id);
  switch (command) {
    case "local_setup":
      return setupOf(projectId);
    case "save_local_settings": {
      const state = stateOf(projectId);
      Object.assign(state, { keywords: args.keywords as string[], grid: Number(args.grid), radius_km: Number(args.radius_km), schedule: args.schedule as LocalSchedule });
      return setupOf(projectId);
    }
    case "choose_business":
      stateOf(projectId).business = args.business as Business;
      return setupOf(projectId);
    case "local_estimate": {
      const usd = args.report === "competitors" ? 0.025 : 0.0054;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: ago(1, 10), fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_local":
    case "fetch_local":
      return kept(paidData(projectId, args));
    case "local_grid_estimate": {
      const state = stateOf(projectId);
      const points = state.business ? state.grid * state.grid : 0;
      const checks = points * state.keywords.length;
      const runs = state.schedule === "weekly" ? 4 : state.schedule === "monthly" ? 1 : 0;
      const scheduled = checks * 0.0006 + 0.0015;
      return {
        points,
        checks,
        run: { usd: checks * 0.002, credits: checks * 0.002 * 1350, via: "credit", kept: null, blocked: null },
        queued: true,
        scheduled_usd: scheduled,
        runs_per_month: runs,
        month_usd: scheduled * runs,
        month_credits: scheduled * runs * 1350,
      } satisfies GridEstimate;
    }
    case "start_local_grid":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to check the grid." });
    case "local_runs":
      return runsOf(projectId);
    case "local_grid":
      return gridReport(projectId, typeof args.run_id === "number" ? args.run_id : null);
    case "local_reviews_estimate": {
      const usd = 0.00075 * Math.ceil(Number(args.depth ?? 100) / 10);
      return { usd, credits: usd * 1350, via: "credit", kept: null, blocked: null } satisfies Estimate;
    }
    case "fetch_local_reviews":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to read reviews." });
    case "local_reviews":
      return reviewsOf(projectId);
    default:
      return undefined;
  }
}
