// Sample content work for the interface outside the program: two briefs of the first sample
// project's shoe shop (one with a brief an assistant wrote), a page and a draft scored against
// them, and the shop's pages worth checking. A draft typed in the editor is scored here by the
// share of the brief's terms it uses, its length and its headings, close enough to show how
// the score moves.

import { CallError } from "./transport";
import type { BriefSummary, BriefView, CheckerRow, ContentBrief, ContentScore, ContentScoreRow, ScoredContent, Writer } from "./types";

const ago = (days: number, hour = 9) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 11) + `${String(hour).padStart(2, "0")}:00:00Z`;

const PARTS: [string, number][] = [
  ["placement", 20],
  ["coverage", 25],
  ["depth", 15],
  ["structure", 10],
  ["questions", 10],
  ["media", 5],
  ["schema", 5],
  ["snippet", 5],
  ["freshness", 5],
];

const RUNNING: ContentBrief = {
  keyword: "koşu ayakkabısı",
  intent: "commercial",
  words: { low: 1150, median: 1640, high: 2300 },
  h2: { low: 5, median: 7, high: 10 },
  title: { median_chars: 56, median_px: 498, keyword_first: 0.6, with_year: 0.4, with_number: 0.3 },
  headings: ["Koşu ayakkabısı nasıl seçilir", "Taban ve yastıklama", "Ayak tipine göre seçim", "Numara ve kalıp", "En iyi modeller", "Fiyatlar"],
  outline: ["Koşu ayakkabısı nasıl seçilir", "Taban ve yastıklama", "Ayak tipine göre seçim", "Numara ve kalıp", "En iyi modeller", "Fiyatlar", "Koşu ayakkabısı kaç km dayanır?", "Koşu ayakkabısı ile yürünür mü?", "Asfalt ve patika ayakkabısı farkı nedir?"],
  questions: ["Koşu ayakkabısı kaç km dayanır?", "Koşu ayakkabısı ile yürünür mü?", "Asfalt ve patika ayakkabısı farkı nedir?", "Koşu ayakkabısı bir numara büyük mü alınır?"],
  ideas: ["koşu ayakkabısı erkek", "koşu ayakkabısı kadın", "patika koşu ayakkabısı", "en iyi koşu ayakkabısı 2026"],
  schema: ["Article", "FAQPage", "BreadcrumbList"],
  short_answer: true,
  terms: [
    ["taban", 10, 1, 6.2],
    ["yastıklama", 9, 0.9, 4.1],
    ["drop", 8, 0.8, 3.4],
    ["pronasyon", 8, 0.8, 2.9],
    ["numara", 8, 0.8, 3.6],
    ["asfalt", 7, 0.7, 2.4],
    ["patika", 7, 0.7, 2.8],
    ["hafif", 7, 0.7, 2.2],
    ["nefes alan", 6, 0.6, 1.6],
    ["kalıp", 6, 0.6, 1.9],
    ["topuk", 6, 0.6, 2.1],
    ["maraton", 5, 0.5, 1.8],
    ["km", 5, 0.5, 3.0],
    ["fiyat", 5, 0.5, 2.0],
    ["karbon plaka", 4, 0.4, 1.5],
  ].map(([term, pages, share, average]) => ({ term: String(term), pages: Number(pages), share: Number(share), average: Number(average) })),
  competitors: [
    [1, "https://www.kosurehberi.com/kosu-ayakkabisi-secimi", "Koşu Ayakkabısı Nasıl Seçilir? 2026 Rehberi", 2420, 11, 82.4],
    [2, "https://www.sporpazari.com/kosu-ayakkabisi", "Koşu Ayakkabısı Modelleri ve Fiyatları", 980, 6, 71.2],
    [3, "https://www.ornekmagaza.com/kosu-ayakkabisi", "Koşu Ayakkabısı | Örnek Mağaza", 640, 3, 54.0],
    [4, "https://www.atletdergisi.com/en-iyi-kosu-ayakkabilari", "En İyi 12 Koşu Ayakkabısı (Test Ettik)", 3150, 14, 79.8],
    [5, "https://www.rival.com/kosu", "Koşu Ayakkabıları", 410, 2, 48.6],
    [6, "https://www.saglikliyasam.com/kosu-ayakkabisi", "Doğru Koşu Ayakkabısı Sakatlığı Önler", 1640, 8, 73.1],
    [7, "https://www.youtube.com/watch?v=kosu", "Koşu ayakkabısı nasıl seçilir? (video)", 0, 0, null],
    [8, "https://www.kosucular.org/ayakkabi", "Ayakkabı seçimi — Koşucular Derneği", 1890, 9, 76.5],
  ].map(([position, url, title, words, h2, score]) => ({
    position: Number(position),
    url: String(url),
    domain: String(url).replace(/^https:\/\/(www\.)?/, "").split("/")[0]!,
    title: String(title),
    words: Number(words),
    h2: Number(h2),
    score: score === null ? null : Number(score),
    status: score === null ? 403 : 200,
  })),
  internal_links: [
    { url: "https://www.ornekmagaza.com/blog/kosu-ayakkabisi-secimi", title: "Koşu ayakkabısı seçerken 7 ipucu", relevance: 0.86 },
    { url: "https://www.ornekmagaza.com/erkek/kosu", title: "Erkek koşu ayakkabıları", relevance: 0.64 },
    { url: "https://www.ornekmagaza.com/kadin/kosu", title: "Kadın koşu ayakkabıları", relevance: 0.62 },
    { url: "https://www.ornekmagaza.com/numara-tablosu", title: "Ayakkabı numara tablosu", relevance: 0.41 },
  ],
};

const SNEAKERS: ContentBrief = {
  ...RUNNING,
  keyword: "spor ayakkabı",
  intent: "transactional",
  words: { low: 420, median: 760, high: 1300 },
  h2: { low: 2, median: 4, high: 6 },
  headings: ["Erkek spor ayakkabı", "Kadın spor ayakkabı", "Markalar", "İndirimli modeller"],
  outline: ["Erkek spor ayakkabı", "Kadın spor ayakkabı", "Markalar", "İndirimli modeller", "Spor ayakkabı nasıl temizlenir?"],
  questions: ["Spor ayakkabı nasıl temizlenir?", "Spor ayakkabı numarası nasıl seçilir?"],
  ideas: ["spor ayakkabı erkek", "beyaz spor ayakkabı", "spor ayakkabı indirim"],
  schema: ["Product", "BreadcrumbList", "ItemList"],
  short_answer: false,
  internal_links: RUNNING.internal_links.slice(1, 3),
};

const ASSISTANT_BRIEF = `# Koşu ayakkabısı — içerik brief'i

## 1. Arama niyeti ve açı
Arayanlar hem **nasıl seçileceğini** öğrenmek hem de **model karşılaştırmak** istiyor. İlk sıradaki rehberler uzun ama mağaza sayfaları kısa; seçim rehberini mağazanın ürün yelpazesiyle birleştiren bir sayfa ikisini de karşılar.

## 2. Başlık ve açıklama önerileri
- Koşu Ayakkabısı Nasıl Seçilir? 2026 Rehberi ve Modeller
- Koşu Ayakkabısı: Ayak Tipine Göre Seçim, Numara ve Fiyatlar
- En İyi Koşu Ayakkabıları: Asfalt ve Patika İçin Seçim Rehberi

Meta açıklama: Ayak tipinize, koşu zeminine ve mesafenize göre koşu ayakkabısı seçin; taban, drop ve numara rehberiyle modelleri karşılaştırın.

## 3. Taslak
- **H2 Koşu ayakkabısı nasıl seçilir** — kısa, doğrudan yanıtla açın (AI özeti var).
- **H2 Taban ve yastıklama** — yastıklama, drop, karbon plaka.
- **H2 Ayak tipine göre seçim** — pronasyon, nötr basış.
- **H2 Numara ve kalıp** — numara tablosuna link verin.
- **H2 Asfalt mı patika mı?**
- **H2 Öne çıkan modeller** — mağaza ürünleri, tablo halinde.
- **H2 Sık sorulan sorular** — kaç km dayanır, yürünür mü, bir numara büyük mü.

## 7. Uzunluk ve biçim
Yaklaşık **1.600 kelime**, 7–9 H2, bir karşılaştırma tablosu, FAQPage yapısal verisi.
`;

const SUGGESTIONS = `**En önemli üç değişiklik**

1. **Sayfayı rehberle açın.** Şu an yalnız ürün listesi var; ürünlerin üstüne 300–400 kelimelik bir "nasıl seçilir" bölümü ekleyin. Örnek H2: *Koşu ayakkabısı seçerken nelere bakmalı?*
2. **Eksik terimleri doğal biçimde kullanın:** drop, pronasyon, asfalt, patika. Örnek: "Asfalt koşuları için yastıklaması yüksek, düşük drop'lu modeller rahat eder."
3. **Soruları yanıtlayın.** "Koşu ayakkabısı kaç km dayanır?" sorusuna 2–3 cümlelik bir yanıt ve FAQPage yapısal verisi ekleyin.

Başlık önerisi: *Koşu Ayakkabısı Modelleri ve Seçim Rehberi | Örnek Mağaza* (56 karakter).
`;

function breakdown(values: Record<string, number | null>): ContentScore["breakdown"] {
  const components = PARTS.map(([id, weight]) => {
    const normalized = values[id] ?? null;
    return { id, weight, value: normalized, normalized, contribution: (normalized ?? 0) * weight, reason: "", params: {} };
  });
  const measured = components.filter((part) => part.normalized !== null);
  const weight = measured.reduce((sum, part) => sum + part.weight, 0) || 1;
  const score = Math.round((measured.reduce((sum, part) => sum + part.contribution, 0) / weight) * 1000) / 10;
  return { score, model: "content-1", confidence: weight / 100, components };
}

const PAGE_SCORE: ContentScore = {
  breakdown: breakdown({ placement: 0.8, coverage: 0.33, depth: 0.39, structure: 0.4, questions: 0, media: 0.7, schema: 0.33, snippet: 0.9, freshness: 0.8 }),
  words: 640,
  h2: 3,
  title: "Koşu Ayakkabısı | Örnek Mağaza",
  title_chars: 30,
  description_chars: 118,
  keyword_in_title: true,
  readability: 61,
  terms: RUNNING.terms.filter((_, index) => [0, 4, 7, 13, 11].includes(index)).map((term, index) => ({ term: term.term, count: [4, 2, 1, 3, 1][index]!, average: term.average, share: term.share })),
  missing: RUNNING.terms.filter((_, index) => ![0, 4, 7, 13, 11].includes(index)).map((term) => ({ term: term.term, count: 0, average: term.average, share: term.share })),
  unanswered: RUNNING.questions.slice(0, 3),
  status: 200,
};

interface State {
  briefs: BriefView[];
}

const states: Record<number, State> = {
  1: {
    briefs: [
      {
        id: 2,
        project_id: 1,
        keyword: "koşu ayakkabısı",
        country: "tr",
        language: "tr",
        mobile: false,
        status: "done",
        brief: RUNNING,
        ai_text: ASSISTANT_BRIEF,
        ai_model: "claude:claude-opus-5-5",
        cost_usd: 0.071,
        error: null,
        created_at: ago(1, 10),
        updated_at: ago(1, 11),
        scores: [
          { id: 21, brief_id: 2, project_id: 1, kind: "url", url: "https://www.ornekmagaza.com/kosu-ayakkabisi", title: null, description: null, body: null, score: PAGE_SCORE.breakdown.score, result: PAGE_SCORE, suggestions: SUGGESTIONS, scored_at: ago(1, 10) },
        ],
      },
      {
        id: 1,
        project_id: 1,
        keyword: "spor ayakkabı",
        country: "tr",
        language: "tr",
        mobile: true,
        status: "done",
        brief: SNEAKERS,
        ai_text: null,
        ai_model: null,
        cost_usd: 0,
        error: null,
        created_at: ago(6, 15),
        updated_at: ago(6, 15),
        scores: [],
      },
    ],
  },
};

function stateOf(projectId: number): State {
  return (states[projectId] ??= { briefs: [] });
}

function briefOf(briefId: number): BriefView {
  for (const state of Object.values(states)) {
    const found = state.briefs.find((brief) => brief.id === briefId);
    if (found) return found;
  }
  throw new CallError({ code: "not_found", message: `content brief ${briefId} not found` });
}

const fold = (text: string) => text.toLocaleLowerCase("tr");

/** A draft scored the sample way: terms used, length and headings against the brief. */
function scoreDraft(brief: ContentBrief, title: string, description: string, body: string): ContentScore {
  const text = fold(`${title}\n${body}`);
  const words = body.split(/\s+/).filter((word) => /[\p{L}\p{N}]/u.test(word.replace(/[#*|-]/g, ""))).length;
  const h2 = body.split("\n").filter((line) => /^##\s/.test(line.trim())).length;
  const count = (term: string) => text.split(fold(term)).length - 1;
  const terms = brief.terms.map((term) => ({ term: term.term, count: count(term.term), average: term.average, share: term.share }));
  const used = terms.filter((term) => term.count > 0);
  const median = brief.words?.median ?? 1000;
  const answered = brief.questions.filter((question) => fold(question).split(/\s+/).filter((word) => word.length > 3).every((word) => text.includes(word.replace(/[?.,]/g, ""))));
  const keyword = fold(brief.keyword);
  const values = {
    placement: (fold(title).includes(keyword) ? 0.5 : 0) + (text.slice(0, 600).includes(keyword) ? 0.3 : 0) + (fold(description).includes(keyword) ? 0.2 : 0),
    coverage: terms.length ? used.length / terms.length : null,
    depth: Math.min(1, words / median),
    structure: Math.min(1, h2 / Math.max(1, brief.h2?.median ?? 4)),
    questions: brief.questions.length ? answered.length / brief.questions.length : null,
    media: 0.5,
    schema: null,
    snippet: (title.length > 0 && title.length <= 60 ? 0.5 : 0) + (description.length >= 70 && description.length <= 160 ? 0.5 : 0),
    freshness: null,
  };
  return {
    breakdown: breakdown(values),
    words,
    h2,
    title: title || null,
    title_chars: [...title].length,
    description_chars: [...description].length,
    keyword_in_title: fold(title).includes(keyword),
    readability: words > 30 ? 58 : null,
    terms: used,
    missing: terms.filter((term) => term.count === 0),
    unanswered: brief.questions.filter((question) => !answered.includes(question)),
    status: 200,
  };
}

const PAGES: CheckerRow[] = [
  { url: "https://www.ornekmagaza.com/kosu-ayakkabisi", keyword: "koşu ayakkabısı", source: "rank", position: 3, clicks: null, volume: 14_800, score: PAGE_SCORE.breakdown.score, scored_for: "koşu ayakkabısı", scored_at: ago(1, 10) },
  { url: "https://www.ornekmagaza.com/spor-ayakkabi", keyword: "spor ayakkabı", source: "rank", position: 6, clicks: null, volume: 90_500, score: null, scored_for: null, scored_at: null },
  { url: "https://www.ornekmagaza.com/kadin", keyword: "kadın ayakkabı", source: "rank", position: 11, clicks: null, volume: 33_100, score: null, scored_for: null, scored_at: null },
  { url: "https://www.ornekmagaza.com/blog/kosu-ayakkabisi-secimi", keyword: "koşu ayakkabısı nasıl seçilir", source: "gsc", position: 8.4, clicks: 212, volume: null, score: null, scored_for: null, scored_at: null },
  { url: "https://www.ornekmagaza.com/erkek/bot", keyword: "erkek bot", source: "gsc", position: 14.2, clicks: 96, volume: null, score: null, scored_for: null, scored_at: null },
];

const WRITERS: Writer[] = [
  { engine: "claude", model: "claude-opus-5-5", ready: true, usd: 0.12 },
  { engine: "chatgpt", model: "gpt-5.5", ready: false, usd: 0.08 },
  { engine: "gemini", model: "gemini-3.8-flash", ready: false, usd: 0.01 },
];

const summary = (brief: BriefView): BriefSummary => ({
  id: brief.id,
  keyword: brief.keyword,
  country: brief.country,
  language: brief.language,
  mobile: brief.mobile,
  status: brief.status,
  error: brief.error,
  created_at: brief.created_at,
  updated_at: brief.updated_at,
  words: brief.brief?.words?.median ?? null,
  pages: brief.brief?.competitors.filter((page) => page.score !== null).length ?? 0,
  written: brief.ai_text !== null,
  cost_usd: brief.cost_usd,
});

export function contentDemo(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case "content_brief_estimate": {
      const kept = String(args.keyword ?? "").trim().toLocaleLowerCase("tr") === "koşu ayakkabısı";
      return { usd: 0.002, credits: 2.7, via: "credit", kept: kept ? { fetched_at: ago(1, 10), fresh: true } : null, blocked: null };
    }
    case "start_content_brief":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to make a brief." });
    case "content_briefs":
      return stateOf(Number(args.project_id)).briefs.map(summary);
    case "content_brief":
      return briefOf(Number(args.brief_id));
    case "delete_content_brief": {
      const state = stateOf(1);
      state.briefs = state.briefs.filter((brief) => brief.id !== Number(args.brief_id));
      return null;
    }
    case "score_content": {
      const view = briefOf(Number(args.brief_id));
      if (!view.brief) throw new CallError({ code: "invalid", message: "the brief is not made yet" });
      if (typeof args.url === "string" && args.url.trim()) throw new CallError({ code: "unsupported", message: "Sample data: open the program to read a page." });
      const body = String(args.body ?? "");
      if (!body.trim()) throw new CallError({ code: "invalid", message: "write or paste the draft, or give a page's address" });
      const title = String(args.title ?? "");
      const description = String(args.description ?? "");
      const score = scoreDraft(view.brief, title, description, body);
      let id: number | null = null;
      if (args.save === true) {
        const row: ContentScoreRow = { id: view.id * 100 + 1, brief_id: view.id, project_id: view.project_id, kind: "draft", url: null, title, description, body, score: score.breakdown.score, result: score, suggestions: null, scored_at: new Date().toISOString() };
        view.scores = [row, ...view.scores.filter((entry) => entry.kind !== "draft")];
        id = row.id;
      }
      return { ...score, id } satisfies ScoredContent;
    }
    case "content_pages":
      return Number(args.project_id) === 1 ? PAGES : [];
    case "content_writing_estimate":
      return { writers: WRITERS, material_chars: 9_800 };
    case "reply_estimate":
      return WRITERS;
    case "start_content_writing":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to write with your own key." });
    case "draft_review_reply":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to draft with your own key." });
    default:
      return undefined;
  }
}
