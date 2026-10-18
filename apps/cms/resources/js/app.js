// Behaviour of the admin pages: Alpine components for the block editor, the media picker,
// slugs and SEO counters, plus confirmations for destructive buttons.

import Alpine from "alpinejs";

import { ICONS } from "./icons";

/** SVG markup of a Lucide icon, for `x-html`. */
export function icon(name, size = 18) {
  const node = ICONS[name];
  if (!node) return "";
  const inner = node
    .map(([tag, attrs]) => `<${tag} ${Object.entries(attrs).map(([k, v]) => `${k}="${v}"`).join(" ")}/>`)
    .join("");
  return `<svg class="icon" xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;
}

const LETTERS = { ç: "c", ğ: "g", ı: "i", ö: "o", ş: "s", ü: "u", ß: "ss", æ: "ae", ø: "o", å: "a", œ: "oe", ñ: "n" };
const GERMAN = { ä: "ae", ö: "oe", ü: "ue" };

/** URL slug in the language's transliteration, matching the server's `Str::slug`. */
export function slugify(text, locale = "en") {
  const map = locale.startsWith("de") ? { ...LETTERS, ...GERMAN } : LETTERS;
  return text
    .toLocaleLowerCase(locale)
    .replace(/[^\u0000-~]/g, (ch) => map[ch] ?? ch)
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

const uid = () => `b${Math.random().toString(36).slice(2, 12)}`;

function blankValue(field) {
  switch (field.type) {
    case "toggle":
      return false;
    case "strings":
    case "items":
      return [];
    case "media":
    case "number":
      return null;
    case "select":
      return Object.keys(field.options)[0];
    default:
      return "";
  }
}

function fill(fields, data) {
  const out = data && typeof data === "object" && !Array.isArray(data) ? data : {};
  for (const field of fields) {
    if (!(field.name in out) || (out[field.name] === null && field.type !== "media" && field.type !== "number")) out[field.name] = blankValue(field);
    if (field.type === "items") out[field.name] = (out[field.name] || []).map((item) => fill(field.fields, item));
  }
  return out;
}

const csrf = () => document.querySelector('meta[name="csrf-token"]')?.content ?? "";

document.addEventListener("alpine:init", () => {
  /** Uploaded files known on this page (id → {url, filename}), filled by the server and the picker. */
  Alpine.store("media", {
    files: {},
    remember(list) {
      for (const file of list) this.files[file.id] = file;
    },
    url(id) {
      return id ? this.files[id]?.url ?? "" : "";
    },
    name(id) {
      return id ? this.files[id]?.filename ?? `#${id}` : "";
    },
  });

  /** The media picker modal; `choose(callback)` opens it and hands the chosen file's id back. */
  Alpine.store("picker", {
    open: false,
    callback: null,
    choose(callback) {
      this.callback = callback;
      this.open = true;
      window.dispatchEvent(new CustomEvent("picker-open"));
    },
    pick(file) {
      Alpine.store("media").remember([file]);
      this.callback?.(file.id);
      this.open = false;
    },
  });

  Alpine.data("mediaPicker", (endpoint) => ({
    items: [],
    query: "",
    page: 1,
    lastPage: 1,
    loading: false,
    uploading: false,
    error: "",
    async load(page = 1) {
      this.loading = true;
      this.error = "";
      try {
        const url = `${endpoint}?${new URLSearchParams({ q: this.query, page: String(page) })}`;
        const response = await fetch(url, { headers: { Accept: "application/json" } });
        const json = await response.json();
        this.items = json.data;
        this.page = json.current_page;
        this.lastPage = json.last_page;
        Alpine.store("media").remember(json.data);
      } catch {
        this.error = "Dosyalar yüklenemedi.";
      } finally {
        this.loading = false;
      }
    },
    async upload(event) {
      const files = [...event.target.files];
      if (!files.length) return;
      this.uploading = true;
      this.error = "";
      try {
        for (const file of files) {
          const body = new FormData();
          body.append("files[]", file);
          const response = await fetch(endpoint, {
            method: "POST",
            body,
            headers: { Accept: "application/json", "X-CSRF-TOKEN": csrf() },
          });
          const json = await response.json();
          if (!response.ok) throw new Error(json.message || "Yükleme başarısız.");
          Alpine.store("media").remember(json.data);
        }
        await this.load(1);
      } catch (error) {
        this.error = error.message;
      } finally {
        this.uploading = false;
        event.target.value = "";
      }
    },
  }));

  /** Ordered content blocks of a translation; serialised into the form's `blocks` field. */
  Alpine.data("blockEditor", (config) => ({
    blocks: [],
    defs: config.definitions,
    icons: config.icons,
    products: config.products,
    open: {},
    adding: false,
    addAt: null,
    init() {
      Alpine.store("media").remember(config.media || []);
      this.blocks = (config.blocks || [])
        .filter((block) => this.defs[block.type])
        .map((block) => ({ id: block.id || uid(), type: block.type, data: fill(this.defs[block.type].fields, block.data) }));
      if (this.blocks.length <= 3) for (const block of this.blocks) this.open[block.id] = true;
    },
    get json() {
      return JSON.stringify(this.blocks);
    },
    def(block) {
      return this.defs[block.type];
    },
    showAdd(index = null) {
      this.addAt = index;
      this.adding = true;
    },
    add(type) {
      const block = { id: uid(), type, data: fill(this.defs[type].fields, {}) };
      const at = this.addAt === null ? this.blocks.length : this.addAt;
      this.blocks.splice(at, 0, block);
      this.open[block.id] = true;
      this.adding = false;
      this.$dispatch("input");
      this.$nextTick(() => document.getElementById(`block-${block.id}`)?.scrollIntoView({ behavior: "smooth", block: "center" }));
    },
    remove(index) {
      if (!window.confirm("Bu blok silinsin mi?")) return;
      this.blocks.splice(index, 1);
      this.$dispatch("input");
    },
    move(list, index, step) {
      const to = index + step;
      if (to < 0 || to >= list.length) return;
      const [item] = list.splice(index, 1);
      list.splice(to, 0, item);
      this.$dispatch("input");
    },
    duplicate(index) {
      const copy = JSON.parse(JSON.stringify(this.blocks[index]));
      copy.id = uid();
      this.blocks.splice(index + 1, 0, copy);
      this.open[copy.id] = true;
      this.$dispatch("input");
    },
    toggle(block) {
      this.open[block.id] = !this.open[block.id];
    },
    expandAll(value) {
      for (const block of this.blocks) this.open[block.id] = value;
    },
    addItem(list, fields) {
      list.push(fill(fields, {}));
      this.$dispatch("input");
    },
    removeItem(list, index) {
      list.splice(index, 1);
      this.$dispatch("input");
    },
    /** One line telling collapsed blocks apart. */
    summary(block) {
      const data = block.data;
      for (const key of ["title", "badge", "body", "code"]) {
        if (typeof data[key] === "string" && data[key].trim()) return data[key].trim().split("\n")[0].slice(0, 90);
      }
      const items = Object.values(data).find((value) => Array.isArray(value) && value.length);
      if (items) return `${items.length} öğe`;
      return "";
    },
    itemTitle(item, index, label) {
      const text = item.title || item.question || item.label || item.query || item.value || "";
      return `${label} ${index + 1}${text ? ` · ${String(text).slice(0, 60)}` : ""}`;
    },
    icon,
  }));

  /** Title, slug and SEO fields of a translation, with live counters and a result preview. */
  Alpine.data("translationForm", (config) => ({
    isHome: config.isHome,
    parentUrl: config.parentUrl,
    title: config.title,
    slug: config.slug,
    slugTouched: config.exists,
    seoTitle: config.seoTitle,
    seoDescription: config.seoDescription,
    excerpt: config.excerpt,
    imageId: config.imageId,
    dirty: false,
    init() {
      Alpine.store("media").remember(config.media || []);
      window.addEventListener("beforeunload", (event) => {
        if (this.dirty) event.preventDefault();
      });
    },
    titleChanged() {
      if (!this.slugTouched && !config.isHome) this.slug = slugify(this.title, config.locale);
    },
    slugChanged() {
      this.slugTouched = true;
      this.slug = slugify(this.slug, config.locale);
    },
    get url() {
      return this.isHome ? this.parentUrl : `${this.parentUrl}/${this.slug || "…"}`;
    },
    get previewTitle() {
      const title = this.seoTitle || this.title;
      return config.isHome ? title : `${title}${config.suffix}`;
    },
    get previewDescription() {
      return this.seoDescription || this.excerpt || "";
    },
    count(value, low, high) {
      const length = (value || "").length;
      return { length, cls: length === 0 ? "" : length < low ? "warn" : length > high ? "bad" : "good" };
    },
    chooseImage() {
      Alpine.store("picker").choose((id) => {
        this.imageId = id;
        this.dirty = true;
      });
    },
    icon,
  }));

  Alpine.data("slugField", (config) => ({
    title: config.title ?? "",
    slug: config.slug ?? "",
    touched: Boolean(config.slug),
    locale: config.locale ?? "en",
    titleChanged() {
      if (!this.touched) this.slug = slugify(this.title, this.locale);
    },
    slugChanged() {
      this.touched = true;
      this.slug = slugify(this.slug, this.locale);
    },
  }));
});

// <form data-confirm="…"> asks before submitting (deletes).
document.addEventListener("submit", (event) => {
  const message = event.target.dataset?.confirm;
  if (message && !window.confirm(message)) event.preventDefault();
});

// Copy buttons: <button data-copy="text">.
document.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-copy]");
  if (!button) return;
  await navigator.clipboard.writeText(button.dataset.copy);
  const label = button.querySelector("span");
  if (label) {
    const before = label.textContent;
    label.textContent = "Kopyalandı";
    setTimeout(() => (label.textContent = before), 1400);
  }
});

window.Alpine = Alpine;
window.cmsIcon = icon;
Alpine.start();
