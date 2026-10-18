# Chromatophore — the octopusSEO design language

Chromatophores are the cells an octopus uses to change color. Every octopusSEO program fills its window with its own deep color, the **product ground** (the Log Analyser is deep violet; the SEO Spider will be deep teal), and lays the same translucent glass, white type and white action buttons on top of it.

The look is a premium Mac app: one continuous ground behind sidebar and content, frosted-glass cards, large confident titles, generous spacing, and white primary buttons. Color is rare and deliberate: small glossy icon tiles in the sidebar, status marks and charts. Everything else is white glass on the ground.

## Content fundamentals

- Write for SEO specialists at agencies. Use their terms: *verified Googlebot*, *crawl budget*, *orphan pages*, *status class*, *robots.txt*. Never explain them in the UI.
- Address the user as *you*; buttons are verbs that say what happens: *Import logs*, *Add site*, *Export to Excel*. A toast confirms in past tense: *Imported 4.8M lines*.
- Sentence case everywhere except `type-label` labels, which the style uppercases. No exclamation marks, no emoji.
- Numbers carry their unit and use the user's locale: *1.204.388 istek*, *182 ms*, *%9,7*. Dates in tables are ISO (`2026-10-11`); prose uses the locale's long form.
- Errors say what went wrong and how to fix it: *The site is busy importing; try again when the import finishes.*
- Every UI string exists in English and Turkish; the website adds German and Spanish.

## Visual foundations

### Color

- One theme, **Deep Sea**. The window ground is a gradient from `ground-1` (top left) through `ground-2` to `ground-3` (bottom right), with a faint white light in the top-left corner (`oc-ground`). It fills sidebar and content as one field.
- Glass layers are translucent white: `surface-1` (6%: cards, panels, stat tiles, tables), `surface-2` (10%: inputs, secondary buttons, hover), `surface-3` (16%: selected and pressed). Floating layers use `surface-raised` with a 30px backdrop blur.
- Text is white (`ink`) and lavender (`ink-muted` for subtitles and descriptions, `ink-faint` for labels and captions). All pass 4.9:1 on the ground, `surface-1` and `surface-2` anywhere along the gradient.
- The action color is white: primary buttons fill with `accent-fill` and carry `on-accent` text, like a light on the deep ground. `accent` (pale violet) marks links and selected values. One white button per card or view.
- Icon tiles (`tile-*`) are the only multicolored elements: each module has its own glossy tile. Keep them small (30px in the sidebar, 48px in cards).
- `pop-fill` is for the rare "New" badge only. `product` marks the product badge.
- Status colors (`good`, `warning`, `critical`, `info`, each with a `-soft` ground) mean state only and come with an icon or a word. Stat tiles keep their value white and show state with a dot.
- Charts take `chart-1` … `chart-8` in that fixed order (teal, orange, cyan, rose, blue, green, pink, amber), validated on the glass for color-blind separation and at least 4.2:1 contrast. Status classes use the status colors: 2xx `good`, 3xx `info`, 4xx `warning`, 5xx `critical`.
- Never add colored backgrounds to cards or gradients to text; the ground is the only large color.

### Type

- The system face first: SF Pro on macOS, Segoe UI on Windows, the bundled **Geist** elsewhere. **Geist Mono** (or SF Mono) for queries, URLs, IPs and code.
- The page title is large and confident: `type-display-xl` 48/56, 600, −0.025em, white, with a `type-lead` subtitle (19/27) in `ink-muted`. Card titles use `type-card-title` (20/26, 600).
- UI text is `type-body` 15/22; tables and meta lines `type-body-sm` 13/19. Labels use `type-label`: 11px, 600, 0.06em tracking, uppercase, `ink-faint`.
- Numbers that line up use tabular figures (`oc-num`, `type-metric`, numeric table cells).

### Space, shape, depth

- 4px grid: `space-1` (4) to `space-12` (48). Pages pad `space-10` (40) at the sides; cards pad `space-6` (24); cards are `space-4` (16) apart.
- Radii are soft: `radius-sm` (10) buttons and inputs, `radius-md` (14) sidebar items, menus and banners, `radius-lg` (22) cards and stat tiles, `radius-xl` (28) modals.
- Cards have a hairline `line` border and `shadow-card`; the white button has `shadow-button`; icon tiles have `shadow-tile` (a glossy top edge).
- Borders that define a control use `line-control` (3:1 on the glass).

### Window

- On macOS the title bar is transparent and the window controls float over the sidebar; the top 44px of sidebar and content (`oc-titlebar`, `data-tauri-drag-region`) drag the window.
- The sidebar is a list of modules (`oc-nav-item` with an `oc-tile`), with the current site or project in an `oc-switcher` above it and Settings at the bottom.

### Motion

- Durations: `--dur-fast` 120ms (hover), `--dur` 200ms (focus, toggles), `--dur-slow` 360ms (entering layers). Pressed buttons scale to 0.97. Menus, modals and toasts rise 8px as they appear.
- One signature motion: the white sonar line turning in `oc-radar scanning` while a page loads or an import scans. Everything respects `prefers-reduced-motion`.

### States

- Hover adds a glass layer (`surface-1` → `surface-2`); the selected item is `surface-3` with a `line-strong` border.
- Focus: 2px solid white outline, 2px offset; text fields get a white border and a 3px `focus-ring`.
- Disabled: 45% opacity, no hover.

## Components

All classes are prefixed `oc-`; modifiers are bare words used together with them (`oc-btn primary small`).

| Component | Classes |
|---|---|
| Window | `oc-ground`, `oc-shell`, `oc-work`, `oc-titlebar` |
| Sidebar | `oc-rail`, `oc-brand` (`-mark`, `-name`, `-product`, `-actions`), `oc-switcher` (`-text`), `oc-nav`, `oc-nav-item` (`active`), `oc-rail-foot` |
| Icon tile | `oc-tile` (`violet`, `blue`, `teal`, `orange`, `pink`, `cyan`, `amber`, `slate`; `large`) |
| Page header | `oc-pagehead`, `oc-pagehead-text`, `oc-pagehead-title`, `oc-pagehead-lead`, `oc-pagehead-actions`, `oc-search`, `oc-tabs`, `oc-tab` (`active`) |
| Card, panel | `oc-cards`, `oc-card` (`interactive`), `oc-card-title`, `oc-card-foot`, `oc-panel`, `oc-panel-head` |
| Stat tile | `oc-stats`, `oc-stat` (`good`, `critical`), `oc-stat-label`, `oc-stat-value`, `oc-stat-hint` |
| Button | `oc-btn` (`primary` = the white action, `ghost`, `danger`, `danger solid`, `small`, `large`, `icon`) |
| Inputs | `oc-input`, `oc-select`, `oc-textarea`, `oc-field`, `oc-field-hint`, `oc-check`, `oc-segmented` |
| Badge, chip | `oc-badge` (`good`, `warning`, `critical`, `info`, `accent`, `product`, `pop`), `oc-dot`, `oc-chips`, `oc-chip` |
| Banner | `oc-banner` (`error`, `warning`, `info`, `success`) |
| Table | `oc-table-wrap`, `oc-table` (`num` cells, `selected` rows) |
| Loaders | `oc-progress` (`indeterminate`), `oc-spinner`, `oc-radar` (`scanning`) |
| Empty state, drop zone | `oc-empty`, `oc-dropzone` (`over`) |
| Overlays | `oc-scrim`, `oc-modal`, `oc-menu` (`oc-menu-item`, `oc-menu-heading`, `oc-menu-sep`), `oc-toasts`, `oc-toast` (`error`, `success`) |
| Inline code | `oc-code` |

## Iconography

- Icons come from [Lucide](https://lucide.dev) (`lucide-react` in React apps): 2px strokes, 16px in controls and tiles, 18px in banners and toasts, 24px in large tiles and the radar emblem.
- Module icons sit on an `oc-tile` in their module's hue, white glyph on a glossy gradient tile. Other icons take the text color of their context; status icons take a status color.
- The octopus mark is the only illustration: the sidebar brand block, the app icon and the favicon. Never redraw or recolor it.

## Using the package

```css
@import "@octopusseo/design";            /* tokens, foundations and components */
@import "@octopusseo/design/tokens.css"; /* tokens and fonts only (e.g. a marketing site) */
```

- Put `oc-ground` on the app's root element (or the body) and set `data-product="<program>"` on `<html>` (`log-analyser`, `spider`) for the product ground.
- Charts read `--chart-1` … `--chart-8`, `--chart-grid`, `--chart-axis` and `--ink-faint` from the root at runtime (canvas renderers cannot use CSS variables directly). A new product ground needs the chart palette validated against it.
- `tokens.json` is the single source of every value. Change it and run `pnpm --filter @octopusseo/design build`; CI fails when `tokens.css` is stale.
- Fonts: Geist and Geist Mono (the fallback for systems without SF Pro or Segoe UI) under the SIL Open Font License (`fonts/OFL.txt`).

## Brand mark

The octopusSEO mark is three white glass blocks turning around a centre. Its files are in `brand/` (`@octopusseo/design/brand/*`):

| File | Use |
|---|---|
| `octopus-mark.svg` | Vector mark for dark grounds (white glass); the programs' `BrandMark`, the website's `LogoMark` and the favicons use the same three paths |
| `octopus-mark-dark.svg` | The same mark in deep violet for light grounds (documents, print) |
| `octopus-mark.png` | The original glossy artwork, 512 px on transparency, for large displays |
| `app-icon.png` | The 1024 px app icon (mark on the Deep Sea tile); `apps/log-analyser/src-tauri/icons/source.png` is a copy, regenerate the icon set with `pnpm tauri icon src-tauri/icons/source.png -o src-tauri/icons` in `apps/log-analyser` and remove the `android`/`ios` folders it adds |

Keep the mark white on the product grounds; never recolour single blocks.
