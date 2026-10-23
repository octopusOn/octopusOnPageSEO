//! A report as one self-contained HTML page in one of four designs: a cover (the agency's brand,
//! the report's name, the project and period, highlights and contents), then every part,
//! numbered, with its summary lines, tiles, an SVG chart, bars and tables. The program's font
//! (Geist) is embedded, styles are inline, and the page's own Content-Security-Policy forbids
//! every request and script, so it shows the same in an app preview, behind a share link and in
//! the PDF printer. On A4 the cover fills the first page edge to edge.

use std::fmt::Write as _;
use std::sync::OnceLock;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_platform::DEFAULT_DESIGN;

use super::texts::{date, text};
use super::{Bar, Cell, Chart, GridImage, ReportView, Table, Tile, Widget};
use crate::snapshot::summary::Sentence;

/// The accent colour when the agency has none, by design.
const ACCENTS: &[(&str, &str)] = &[
    ("clean", "#1e6fd9"),
    ("ocean", "#3354d1"),
    ("accent", "#1e6fd9"),
    ("elegant", "#9a7b4f"),
];

/// Highlights on the cover at most.
const HIGHLIGHTS: usize = 4;

/// Text for HTML (element content and quoted attributes).
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// The report's design, a known one.
fn design_of(view: &ReportView) -> &str {
    ACCENTS
        .iter()
        .map(|(design, _)| *design)
        .find(|design| *design == view.design)
        .unwrap_or(DEFAULT_DESIGN)
}

/// Ink on dark colours.
const WHITE: &str = "#ffffff";

/// Ink on light colours.
const INK: &str = "#0f1726";

/// Whether `value` is a `#rrggbb` colour.
fn is_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The report's own accent, else the agency's, else the design's.
fn accent<'a>(view: &'a ReportView, design: &str) -> &'a str {
    view.colors
        .accent
        .as_deref()
        .or(view.branding.accent.as_deref())
        .filter(|accent| is_color(accent))
        .unwrap_or_else(|| {
            ACCENTS
                .iter()
                .find(|(name, _)| *name == design)
                .map_or("#1e6fd9", |(_, accent)| accent)
        })
}

/// Ink that reads on `#rrggbb`: white on dark colours, near-black on light ones.
fn ink_on(color: &str) -> &'static str {
    let channel = |at: usize| {
        let value = f64::from(u8::from_str_radix(&color[at..at + 2], 16).unwrap_or(0)) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
    if luminance > 0.42 { INK } else { WHITE }
}

/// The embedded Geist font (Latin and Latin Extended, variable weight).
pub(super) fn fonts() -> &'static str {
    static FONTS: OnceLock<String> = OnceLock::new();
    FONTS.get_or_init(|| {
        let face = |bytes: &[u8], range: &str| {
            format!(
                "@font-face{{font-family:\"Geist\";src:url(data:font/woff2;base64,{}) format(\"woff2\");font-weight:100 900;font-style:normal;font-display:block;unicode-range:{range};}}",
                STANDARD.encode(bytes)
            )
        };
        face(
            include_bytes!("fonts/geist-latin-wght-normal.woff2"),
            "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD",
        ) + &face(
            include_bytes!("fonts/geist-latin-ext-wght-normal.woff2"),
            "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF",
        )
    })
}

/// What every design shares.
const STYLE: &str = r#"
@page { size: A4; margin: 16mm 14mm 18mm; }
@page :first { margin: 0; }
* { box-sizing: border-box; }
html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
body { margin: 0; background: #e8ecf3; color: #344054; font: 13px/1.55 "Geist", -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; -webkit-font-smoothing: antialiased; }
.report { --font: "Geist", -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; --display: var(--font); --ink: #0f1726; --body: #344054; --muted: #667085; --line: #e4e8ef; --soft: #f6f8fb; --good: #0b8a5c; --good-soft: #e3f5ec; --bad: #c4410b; --bad-soft: #fdede4; --accent-2: color-mix(in srgb, var(--accent) 55%, #1fb6c9); --accent-soft: color-mix(in srgb, var(--accent) 9%, #fff); --accent-line: color-mix(in srgb, var(--accent) 24%, #fff); --accent-deep: color-mix(in srgb, var(--accent) 68%, #070b1a); --grad: linear-gradient(90deg, var(--accent), var(--accent-2)); max-width: 920px; margin: 28px auto; color: var(--body); font-family: var(--font); }
.cover, .content { background: #fff; border-radius: 20px; box-shadow: 0 1px 2px rgba(16, 24, 40, .06), 0 24px 60px rgba(15, 23, 42, .10); }
.cover { position: relative; overflow: hidden; isolation: isolate; display: flex; flex-direction: column; gap: 30px; min-height: 620px; margin-bottom: 24px; padding: 44px 52px 40px; color: var(--ink); }
.cover-art { position: absolute; inset: 0; z-index: -1; pointer-events: none; }
.cover-art::after { content: ""; position: absolute; right: -140px; top: -140px; width: 440px; height: 440px; border-radius: 50%; }
.cover-top { display: flex; justify-content: space-between; align-items: center; gap: 20px; }
.brand { display: flex; align-items: center; gap: 12px; min-width: 0; }
.brand img { max-height: 52px; max-width: 190px; object-fit: contain; }
.brand strong { display: block; font-size: 15px; font-weight: 600; }
.brand span { display: block; font-size: 12px; opacity: .72; }
.brand svg { width: 34px; height: 34px; }
.cover-date { font-size: 12px; opacity: .72; white-space: nowrap; }
.cover-main { margin-top: auto; }
.kicker { display: inline-block; margin-bottom: 16px; color: var(--accent); font-size: 11px; font-weight: 600; letter-spacing: .14em; text-transform: uppercase; }
.cover h1 { max-width: 15em; margin: 0 0 16px; font-family: var(--display); font-size: 46px; font-weight: 700; line-height: 1.04; letter-spacing: -.03em; }
.cover-site { margin: 0; font-size: 17px; }
.cover-site strong { font-weight: 600; }
.cover-period { margin: 6px 0 0; font-size: 13px; opacity: .72; }
.highlights { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
.highlight { min-width: 0; padding: 15px 16px 14px; border: 1px solid var(--line); border-radius: 14px; background: #fff; }
.highlight span { display: block; overflow: hidden; color: var(--muted); font-size: 10.5px; font-weight: 600; letter-spacing: .06em; text-overflow: ellipsis; text-transform: uppercase; white-space: nowrap; }
.highlight strong { display: block; margin: 6px 0; color: var(--ink); font-size: 25px; font-weight: 650; letter-spacing: -.02em; line-height: 1.1; font-variant-numeric: tabular-nums; }
.cover-bottom { display: flex; justify-content: space-between; align-items: flex-end; gap: 24px; padding-top: 22px; border-top: 1px solid var(--line); }
.contents-title { margin: 0 0 8px; font-size: 10.5px; font-weight: 600; letter-spacing: .12em; text-transform: uppercase; opacity: .62; }
.contents { margin: 0; padding: 0; list-style: none; columns: 2; column-gap: 36px; font-size: 12.5px; }
.contents li { display: flex; gap: 10px; padding: 3px 0; break-inside: avoid; }
.contents b { color: var(--accent); font-weight: 600; font-variant-numeric: tabular-nums; }
.content { padding: 44px 52px 30px; }
.widget { padding: 30px 0 8px; border-top: 1px solid var(--line); }
.widget:first-child { padding-top: 0; border-top: 0; }
.widget-head { display: flex; align-items: flex-start; gap: 14px; margin-bottom: 16px; break-after: avoid; break-inside: avoid; }
.num { flex: none; display: grid; place-items: center; width: 36px; height: 36px; border-radius: 11px; background: var(--accent-soft); color: var(--accent); font-size: 13px; font-weight: 650; font-variant-numeric: tabular-nums; }
.widget h2 { margin: 4px 0 0; color: var(--heading, var(--ink)); font-family: var(--display); font-size: 21px; font-weight: 650; letter-spacing: -.015em; line-height: 1.25; }
.note { margin: 3px 0 0; color: var(--muted); font-size: 12px; }
.empty { margin: 4px 0 12px; padding: 18px; border: 1px dashed var(--line); border-radius: 12px; color: var(--muted); text-align: center; }
.lines { display: grid; gap: 8px; margin: 0 0 18px; padding: 0; list-style: none; }
.lines li { display: flex; align-items: flex-start; gap: 11px; padding: 11px 14px; border: 1px solid var(--line); border-radius: 12px; background: var(--soft); color: var(--ink); break-inside: avoid; }
.icon { flex: none; display: grid; place-items: center; width: 22px; height: 22px; border-radius: 50%; background: #e7eaf0; color: var(--muted); }
.icon svg { width: 14px; height: 14px; }
.tone-good .icon { background: var(--good-soft); color: var(--good); }
.tone-bad .icon { background: var(--bad-soft); color: var(--bad); }
.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; margin: 0 0 18px; }
.tile { position: relative; padding: 15px 16px 13px; border: 1px solid var(--line); border-radius: 13px; background: #fff; box-shadow: 0 1px 2px rgba(16, 24, 40, .05); break-inside: avoid; }
.tile::before { content: ""; position: absolute; top: 0; left: 16px; width: 34px; height: 3px; border-radius: 0 0 3px 3px; background: var(--grad); }
.tile span { display: block; color: var(--muted); font-size: 10.5px; font-weight: 600; letter-spacing: .06em; text-transform: uppercase; }
.tile strong { display: block; margin: 7px 0 6px; color: var(--ink); font-size: 25px; font-weight: 650; letter-spacing: -.02em; line-height: 1.1; font-variant-numeric: tabular-nums; }
.pill { display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: 999px; font-size: 11px; font-style: normal; font-weight: 600; line-height: 1.5; font-variant-numeric: tabular-nums; }
.pill svg { width: 8px; height: 8px; }
.pill.good { background: var(--good-soft); color: var(--good); }
.pill.bad { background: var(--bad-soft); color: var(--bad); }
.chart { margin: 0 0 18px; padding: 16px 18px 12px; border: 1px solid var(--line); border-radius: 13px; break-inside: avoid; }
.chart-title { margin: 0 0 8px; color: var(--heading, var(--ink)); font-size: 12.5px; font-weight: 600; }
svg { display: block; width: 100%; height: auto; }
.legend { display: flex; flex-wrap: wrap; gap: 16px; margin-top: 6px; color: var(--muted); font-size: 11px; }
.legend i { display: inline-block; width: 18px; height: 0; margin-right: 6px; border-top: 2.5px solid var(--accent); vertical-align: middle; }
.legend i.dashed { border-top: 2px dashed #98a2b3; }
.block-title { margin: 22px 0 10px; color: var(--heading, var(--ink)); font-size: 13px; font-weight: 600; break-after: avoid; }
.bars { margin: 0 0 18px; padding: 0; list-style: none; break-inside: avoid; }
.bars li { display: grid; grid-template-columns: minmax(120px, 32%) 1fr 72px; gap: 12px; align-items: center; margin: 7px 0; }
.bars .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bars .own .label { color: var(--ink); font-weight: 650; }
.track { height: 10px; overflow: hidden; border-radius: 999px; background: #eef1f6; }
.fill { display: block; height: 100%; border-radius: 999px; background: color-mix(in srgb, var(--accent) 30%, #cdd3de); }
.own .fill { background: var(--grad); }
.bars .value { color: var(--ink); text-align: right; font-variant-numeric: tabular-nums; }
.table-wrap { margin: 0 0 8px; overflow: hidden; border: 1px solid var(--line); border-radius: 12px; }
table { width: 100%; border-collapse: collapse; font-size: 11.5px; }
td:first-child { min-width: 9em; }
thead { display: table-header-group; }
th { padding: 9px 11px; background: var(--accent-soft); color: var(--ink); font-size: 10px; font-weight: 600; letter-spacing: .05em; text-align: left; text-transform: uppercase; }
td { padding: 8px 11px; border-top: 1px solid var(--line); vertical-align: top; overflow-wrap: break-word; }
td:first-child { color: var(--ink); font-weight: 500; }
tbody tr:nth-child(even) td { background: #fafbfd; }
td.number, th.number { text-align: right; font-variant-numeric: tabular-nums; }
td.number { white-space: nowrap; }
tr { break-inside: avoid; }
.grids { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 16px; margin: 0 0 18px; break-inside: avoid; }
.grid { margin: 0; padding: 12px; border: 1px solid var(--line); border-radius: 13px; }
.grid text { fill: #0b1020; font-size: 14px; font-weight: 700; text-anchor: middle; }
.grid .band-top3 { fill: #34d399; } .grid .band-top10 { fill: #facc15; } .grid .band-top20 { fill: #fb923c; } .grid .band-none { fill: #f87171; } .grid .band-error { fill: #94a3b8; }
.grid figcaption { display: grid; gap: 2px; margin-top: 8px; font-size: 11.5px; }
.grid figcaption span { color: var(--muted); }
footer { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 8px 16px; margin-top: 30px; padding-top: 16px; border-top: 1px solid var(--line); color: var(--muted); font-size: 11.5px; }
.made { display: inline-flex; align-items: center; gap: 7px; }
.made svg { width: 16px; height: 16px; }
@media print {
  body { background: #fff; }
  .report { max-width: none; margin: 0; }
  .cover { height: 296mm; min-height: 0; margin: 0; padding: 20mm 18mm 16mm; border-radius: 0; box-shadow: none; break-after: page; }
  .content { padding: 0; border-radius: 0; box-shadow: none; }
  .tiles { grid-template-columns: repeat(4, minmax(0, 1fr)); }
}
@media (max-width: 680px) {
  .report { margin: 0; }
  .cover, .content { border-radius: 0; }
  .cover { min-height: 0; padding: 28px 20px; }
  .cover-top { flex-direction: column; align-items: flex-start; }
  .cover h1 { font-size: 32px; }
  .highlights { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .cover-bottom { flex-direction: column; align-items: flex-start; }
  .contents { columns: 1; }
  .content { padding: 28px 20px; }
  .bars li { grid-template-columns: minmax(80px, 34%) 1fr 56px; }
}
"#;

/// White: a light cover with soft accent light, the default.
const CLEAN: &str = r"
.design-clean .cover::before { content: ''; position: absolute; top: 0; left: 0; right: 0; height: 6px; background: var(--grad); }
.design-clean .cover-art { background: radial-gradient(560px 360px at 94% -6%, color-mix(in srgb, var(--accent) 15%, transparent), transparent 70%), radial-gradient(460px 320px at -8% 108%, color-mix(in srgb, var(--accent-2) 13%, transparent), transparent 70%); }
.design-clean .cover-art::after { border: 1px solid color-mix(in srgb, var(--accent) 16%, transparent); box-shadow: 0 0 0 64px color-mix(in srgb, var(--accent) 4%, transparent), 0 0 0 128px color-mix(in srgb, var(--accent) 2.5%, transparent); }
.design-clean .highlight { box-shadow: 0 1px 2px rgba(16, 24, 40, .05), 0 8px 24px rgba(15, 23, 42, .05); }
";

/// Deep sea: the program's own abyss indigo and aqua on the cover.
const OCEAN: &str = r"
.design-ocean { --accent-2: #19b5c8; }
.design-ocean .cover { background: #0b1a3a; color: #fff; }
.design-ocean .cover-art { background: radial-gradient(720px 440px at 90% 2%, rgba(127, 231, 242, .30), transparent 62%), radial-gradient(660px 520px at -4% 104%, rgba(108, 99, 255, .40), transparent 62%), linear-gradient(155deg, #24357a 0%, #182559 46%, #0b1a3a 100%); }
.design-ocean .cover-art::after { border: 1px solid rgba(255, 255, 255, .14); box-shadow: 0 0 0 70px rgba(255, 255, 255, .03), 0 0 0 140px rgba(255, 255, 255, .02); }
.design-ocean .kicker, .design-ocean .contents b { color: #7fe7f2; }
.design-ocean .highlight { border-color: rgba(255, 255, 255, .16); background: rgba(255, 255, 255, .08); }
.design-ocean .highlight span { color: rgba(255, 255, 255, .72); }
.design-ocean .highlight strong { color: #fff; }
.design-ocean .cover .pill.good { background: rgba(143, 242, 191, .16); color: #8ff2bf; }
.design-ocean .cover .pill.bad { background: rgba(255, 179, 161, .16); color: #ffc2b3; }
.design-ocean .cover-bottom { border-top-color: rgba(255, 255, 255, .16); }
.design-ocean .num { background: linear-gradient(140deg, #24357a, #0b1a3a); color: #7fe7f2; }
.design-ocean th { background: #12224a; color: #e8edff; }
";

/// The agency's colour: the cover in the accent, section heads on a tint.
const ACCENT: &str = r"
.design-accent .cover { background: var(--accent-deep); color: var(--cover-ink); }
.design-accent .cover-art { background: radial-gradient(620px 420px at 100% 0%, rgba(255, 255, 255, .20), transparent 60%), linear-gradient(140deg, var(--accent) 0%, var(--accent-deep) 100%); }
.design-accent .cover-art::before { content: ''; position: absolute; right: -12%; bottom: -36%; width: 72%; height: 96%; border-radius: 50%; background: repeating-linear-gradient(135deg, rgba(255, 255, 255, .08) 0 2px, transparent 2px 22px); }
.design-accent .cover-art::after { border: 1px solid rgba(255, 255, 255, .22); }
.design-accent .kicker, .design-accent .contents b { color: inherit; opacity: .9; }
.design-accent .highlight { border-color: rgba(255, 255, 255, .24); background: rgba(255, 255, 255, .13); }
.design-accent .highlight span, .design-accent .highlight strong { color: inherit; }
.design-accent .highlight span { opacity: .78; }
.design-accent .cover .pill { background: rgba(255, 255, 255, .2); color: inherit; }
.design-accent .cover-bottom { border-top-color: rgba(255, 255, 255, .24); }
.design-accent .widget-head { padding: 12px 14px; border-radius: 14px; background: var(--accent-soft); }
.design-accent .num { background: var(--accent); color: var(--on-accent); }
.design-accent th { background: var(--accent); color: var(--on-accent); }
.design-accent .tile::before { left: 0; right: 0; width: auto; border-radius: 13px 13px 0 0; }
";

/// Elegant: ivory and serif headings, thin rules instead of boxes.
const ELEGANT: &str = r#"
.design-elegant { --display: "Iowan Old Style", "Palatino Linotype", Palatino, "Book Antiqua", Georgia, "Times New Roman", serif; --accent-2: var(--accent); --soft: #faf8f4; }
.design-elegant .cover { background: #fbfaf7; }
.design-elegant .cover-art { inset: 18px; border: 1px solid rgba(15, 23, 38, .14); box-shadow: inset 0 0 0 5px #fbfaf7, inset 0 0 0 6px rgba(15, 23, 38, .08); }
.design-elegant .cover-art::after { display: none; }
.design-elegant .kicker { letter-spacing: .32em; }
.design-elegant .cover h1 { font-size: 52px; font-weight: 500; letter-spacing: -.01em; }
.design-elegant .highlight { padding: 2px 0 2px 16px; border: 0; border-left: 1px solid rgba(15, 23, 38, .16); border-radius: 0; background: transparent; }
.design-elegant .highlight strong, .design-elegant .tile strong { font-family: var(--display); font-size: 29px; font-weight: 500; letter-spacing: 0; }
.design-elegant .num { width: auto; height: auto; padding-top: 1px; background: transparent; color: var(--accent); font-family: var(--display); font-size: 24px; font-weight: 500; }
.design-elegant .widget h2 { font-size: 25px; font-weight: 500; letter-spacing: 0; }
.design-elegant .tile { padding: 12px 2px 0; border: 0; border-top: 1.5px solid var(--ink); border-radius: 0; box-shadow: none; }
.design-elegant .tile::before { display: none; }
.design-elegant .lines li { border-width: 0 0 1px; border-radius: 0; background: transparent; }
.design-elegant .chart { border-width: 1px 0; border-radius: 0; }
.design-elegant .table-wrap { border: 0; border-radius: 0; }
.design-elegant th { border-bottom: 1.5px solid var(--ink); background: transparent; }
.design-elegant tbody tr:nth-child(even) td { background: transparent; }
"#;

/// A report's own colours over its design's: a cover of its own colour (dark or light), table
/// heads of their own colour. Set through classes and variables on the report.
const CUSTOM: &str = r"
.report.custom-cover .cover { background: var(--cover-bg); color: var(--cover-ink); }
.report.custom-cover .cover-art { inset: 0; border: 0; box-shadow: none; background: radial-gradient(640px 420px at 100% 0%, rgba(255, 255, 255, .16), transparent 60%), radial-gradient(560px 420px at 0% 100%, rgba(0, 0, 0, .10), transparent 62%); }
.report.custom-cover .cover-art::before { display: none; }
.report.custom-cover .cover-art::after { display: block; border: 1px solid color-mix(in srgb, var(--cover-ink) 16%, transparent); box-shadow: 0 0 0 64px color-mix(in srgb, var(--cover-ink) 3%, transparent); }
.report.cover-dark:not(.design-elegant) .highlight { border-color: rgba(255, 255, 255, .22); background: rgba(255, 255, 255, .11); }
.report.cover-light:not(.design-elegant) .highlight { border-color: rgba(15, 23, 38, .12); background: rgba(255, 255, 255, .72); }
.report.cover-dark .highlight span, .report.cover-dark .highlight strong { color: inherit; }
.report.cover-dark .highlight span { opacity: .76; }
.report.cover-light .highlight span { color: var(--muted); opacity: 1; }
.report.cover-light .highlight strong { color: var(--ink); }
.report.cover-dark .kicker, .report.cover-dark .contents b { color: inherit; opacity: .9; }
.report.cover-light .kicker, .report.cover-light .contents b { color: var(--accent); opacity: 1; }
.report.cover-dark .cover .pill { background: rgba(255, 255, 255, .18); color: inherit; }
.report.cover-dark .cover-bottom { border-top-color: rgba(255, 255, 255, .22); }
.report.cover-light .cover-bottom { border-top-color: rgba(15, 23, 38, .12); }
.report.custom-table th { border-bottom-color: var(--table-head); background: var(--table-head); color: var(--table-ink); }
";

fn design_style(design: &str) -> &'static str {
    match design {
        "ocean" => OCEAN,
        "accent" => ACCENT,
        "elegant" => ELEGANT,
        _ => CLEAN,
    }
}

/// The OctopusOn mark for light grounds (no namespace: it lives in HTML).
const MARK: &str = r##"<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="om-fill" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#4c3d99"/><stop offset=".55" stop-color="#2e2470"/><stop offset="1" stop-color="#1b1547"/></linearGradient></defs><g fill="url(#om-fill)"><path d="M31.95 3.93 L46.21 12.70 L46.21 28.72 L60.73 37.39 L49.34 43.59 L34.02 34.57 L34.07 18.85 L20.96 10.63 Z"/><path d="M17.69 12.39 L29.83 19.80 L29.83 34.67 L13.65 44.25 L13.71 59.01 L1.81 51.66 L1.86 38.05 L17.69 28.47 Z"/><path d="M31.95 38.10 L48.48 47.98 L62.19 40.27 L62.19 51.66 L48.08 60.07 L31.90 50.80 L16.28 59.62 L16.33 47.27 Z"/></g></svg>"##;

/// The mark on the cover's dark grounds.
const MARK_LIGHT: &str = r##"<svg viewBox="0 0 64 64" aria-hidden="true"><g fill="#ffffff"><path d="M31.95 3.93 L46.21 12.70 L46.21 28.72 L60.73 37.39 L49.34 43.59 L34.02 34.57 L34.07 18.85 L20.96 10.63 Z"/><path d="M17.69 12.39 L29.83 19.80 L29.83 34.67 L13.65 44.25 L13.71 59.01 L1.81 51.66 L1.86 38.05 L17.69 28.47 Z"/><path d="M31.95 38.10 L48.48 47.98 L62.19 40.27 L62.19 51.66 L48.08 60.07 L31.90 50.80 L16.28 59.62 L16.33 47.27 Z"/></g></svg>"##;

const UP: &str = r#"<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 9 8.5H1Z" fill="currentColor"/></svg>"#;
const DOWN: &str = r#"<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 8.5 1 1.5h8Z" fill="currentColor"/></svg>"#;
const CHECK: &str = r#"<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const ALERT: &str = r#"<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 4v5" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="8" cy="12" r="1.2" fill="currentColor"/></svg>"#;
const DOT: &str = r#"<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="2.6" fill="currentColor"/></svg>"#;

fn lines(out: &mut String, lines: &[Sentence]) {
    if lines.is_empty() {
        return;
    }
    out.push_str("<ul class=\"lines\">");
    for line in lines {
        let icon = match line.tone {
            "good" => CHECK,
            "bad" => ALERT,
            _ => DOT,
        };
        let _ = write!(
            out,
            "<li class=\"tone-{}\"><span class=\"icon\">{icon}</span><span>{}</span></li>",
            escape(line.tone),
            escape(&line.text)
        );
    }
    out.push_str("</ul>");
}

/// A change as a coloured pill with its direction.
fn pill(out: &mut String, tile: &Tile) {
    let Some(change) = &tile.change else {
        return;
    };
    let text = change.text.trim();
    let arrow = if text.starts_with('+') {
        UP
    } else if text
        .split_whitespace()
        .next()
        .is_some_and(|first| first.contains('-'))
    {
        DOWN
    } else {
        ""
    };
    let _ = write!(
        out,
        "<em class=\"pill {}\">{arrow}{}</em>",
        if change.good { "good" } else { "bad" },
        escape(text)
    );
}

fn tiles(out: &mut String, tiles: &[Tile], class: &str) {
    if tiles.is_empty() {
        return;
    }
    let _ = write!(out, "<div class=\"{class}s\">");
    for tile in tiles {
        let _ = write!(
            out,
            "<div class=\"{class}\"><span>{}</span><strong>{}</strong>",
            escape(&tile.label),
            escape(&tile.value)
        );
        pill(out, tile);
        out.push_str("</div>");
    }
    out.push_str("</div>");
}

/// A round upper bound for an axis (1, 2, 2.5 or 5 times a power of ten).
fn ceiling(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 1.0;
    }
    let power = 10_f64.powf(value.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|step| step * power)
        .find(|bound| *bound >= value)
        .unwrap_or(value)
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // point counts; axes, area, lines
fn chart(out: &mut String, chart: &Chart, id: usize, lang: &str) {
    const WIDTH: f64 = 680.0;
    const HEIGHT: f64 = 200.0;
    const LEFT: f64 = 46.0;
    const BOTTOM: f64 = 24.0;
    const TOP: f64 = 10.0;
    let points = chart.labels.len();
    if points < 2 {
        return;
    }
    let most = chart
        .series
        .iter()
        .flat_map(|(_, values, _)| values.iter().flatten())
        .copied()
        .fold(0.0_f64, f64::max);
    let top = ceiling(most);
    // Round steps: 2 is split in four (0.5 each), 1, 2.5 and 5 in five.
    let mantissa = top / 10_f64.powf(top.log10().floor());
    let steps: u8 = if (mantissa - 2.0).abs() < 1e-9 { 4 } else { 5 };
    let decimals =
        usize::from(top / f64::from(steps) < 1.0) + usize::from(top / f64::from(steps) < 0.1);
    let x = |index: usize| LEFT + (WIDTH - LEFT - 10.0) * index as f64 / (points - 1) as f64;
    let y = |value: f64| TOP + (HEIGHT - TOP - BOTTOM) * (1.0 - value / top);
    let _ = write!(
        out,
        "<figure class=\"chart\"><p class=\"chart-title\">{}</p><svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=\"img\" aria-label=\"{}\">\
         <defs><linearGradient id=\"area-{id}\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" style=\"stop-color:var(--accent);stop-opacity:.26\"/><stop offset=\"1\" style=\"stop-color:var(--accent);stop-opacity:0\"/></linearGradient>\
         <linearGradient id=\"line-{id}\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\"><stop offset=\"0\" style=\"stop-color:var(--accent)\"/><stop offset=\"1\" style=\"stop-color:var(--accent-2)\"/></linearGradient></defs>",
        escape(&chart.title),
        escape(&chart.title)
    );
    for step in 0..=steps {
        let value = top * f64::from(step) / f64::from(steps);
        let line = y(value);
        let _ = write!(
            out,
            "<line x1=\"{LEFT}\" x2=\"{}\" y1=\"{line:.1}\" y2=\"{line:.1}\" stroke=\"#e9edf3\"{}/><text x=\"{}\" y=\"{:.1}\" font-size=\"10\" fill=\"#8a93a6\" text-anchor=\"end\">{}</text>",
            WIDTH - 10.0,
            if step == 0 {
                ""
            } else {
                " stroke-dasharray=\"3 4\""
            },
            LEFT - 8.0,
            line + 3.5,
            escape(&super::texts::number(value, decimals, lang))
        );
    }
    for index in [0, points / 2, points - 1] {
        let anchor = match index {
            0 => "start",
            i if i == points - 1 => "end",
            _ => "middle",
        };
        let _ = write!(
            out,
            "<text x=\"{:.1}\" y=\"{}\" font-size=\"10\" fill=\"#8a93a6\" text-anchor=\"{anchor}\">{}</text>",
            x(index),
            HEIGHT - 6.0,
            escape(&chart.labels[index])
        );
    }
    let mut first_solid = true;
    for (_, values, dashed) in &chart.series {
        let mut path = String::new();
        let mut pen_down = false;
        for (index, value) in values.iter().enumerate() {
            match value {
                Some(value) => {
                    let _ = write!(
                        path,
                        "{}{:.1},{:.1} ",
                        if pen_down { "L" } else { "M" },
                        x(index),
                        y(*value)
                    );
                    pen_down = true;
                }
                None => pen_down = false,
            }
        }
        let path = path.trim_end();
        if *dashed {
            let _ = write!(
                out,
                "<path d=\"{path}\" fill=\"none\" stroke=\"#98a2b3\" stroke-width=\"1.6\" stroke-dasharray=\"5 4\" stroke-linejoin=\"round\"/>"
            );
            continue;
        }
        // The first solid line gets the shaded area beneath it, when it has no gaps.
        if first_solid && values.iter().all(Option::is_some) {
            let _ = write!(
                out,
                "<path d=\"{path} L{:.1},{:.1} L{:.1},{:.1} Z\" fill=\"url(#area-{id})\"/>",
                x(points - 1),
                y(0.0),
                x(0),
                y(0.0)
            );
        }
        first_solid = false;
        let _ = write!(
            out,
            "<path d=\"{path}\" fill=\"none\" stroke=\"url(#line-{id})\" stroke-width=\"2.6\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>"
        );
        if let Some((index, Some(value))) =
            values.iter().enumerate().rev().find(|(_, v)| v.is_some())
        {
            let _ = write!(
                out,
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4.2\" fill=\"#fff\" style=\"stroke:var(--accent-2)\" stroke-width=\"2.4\"/>",
                x(index),
                y(*value)
            );
        }
    }
    out.push_str("</svg>");
    if chart.series.len() > 1 {
        out.push_str("<div class=\"legend\">");
        for (name, _, dashed) in &chart.series {
            let _ = write!(
                out,
                "<span><i{}></i>{}</span>",
                if *dashed { " class=\"dashed\"" } else { "" },
                escape(name)
            );
        }
        out.push_str("</div>");
    }
    out.push_str("</figure>");
}

fn bars(out: &mut String, title: Option<&str>, bars: &[Bar], scale: Option<f64>) {
    if bars.is_empty() {
        return;
    }
    if let Some(title) = title {
        let _ = write!(out, "<h3 class=\"block-title\">{}</h3>", escape(title));
    }
    let most = scale
        .unwrap_or_else(|| bars.iter().map(|bar| bar.value).fold(0.0_f64, f64::max))
        .max(f64::MIN_POSITIVE);
    out.push_str("<ol class=\"bars\">");
    for bar in bars {
        let _ = write!(
            out,
            "<li{}><span class=\"label\" title=\"{label}\">{label}</span><span class=\"track\"><span class=\"fill\" style=\"width:{:.1}%\"></span></span><span class=\"value\">{}</span></li>",
            if bar.own { " class=\"own\"" } else { "" },
            (bar.value / most * 100.0).clamp(0.0, 100.0),
            escape(&bar.text),
            label = escape(&bar.label),
        );
    }
    out.push_str("</ol>");
}

fn table(out: &mut String, table: &Table) {
    if table.rows.is_empty() {
        return;
    }
    let numeric: Vec<bool> = (0..table.columns.len())
        .map(|column| {
            table
                .rows
                .iter()
                .any(|row| matches!(row.get(column), Some(Cell::Number { .. })))
        })
        .collect();
    if !table.title.is_empty() {
        let _ = write!(
            out,
            "<h3 class=\"block-title\">{}</h3>",
            escape(&table.title)
        );
    }
    out.push_str("<div class=\"table-wrap\"><table><thead><tr>");
    for (column, name) in table.columns.iter().enumerate() {
        let _ = write!(
            out,
            "<th{}>{}</th>",
            if numeric[column] {
                " class=\"number\""
            } else {
                ""
            },
            escape(name)
        );
    }
    out.push_str("</tr></thead><tbody>");
    for row in &table.rows {
        out.push_str("<tr>");
        for (column, cell) in row.iter().enumerate() {
            let class = if numeric.get(column).copied().unwrap_or(false) {
                " class=\"number\""
            } else {
                ""
            };
            let (Cell::Text(text) | Cell::Number { text, .. }) = cell;
            let _ = write!(out, "<td{class}>{}</td>", escape(text));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
}

fn widget(out: &mut String, widget: &Widget, number: usize, lang: &str) {
    let _ = write!(
        out,
        "<section class=\"widget\" id=\"{}\"><header class=\"widget-head\"><span class=\"num\">{number:02}</span><div><h2>{}</h2>",
        escape(&widget.id),
        escape(&widget.title)
    );
    if let Some(note) = &widget.note {
        let _ = write!(out, "<p class=\"note\">{}</p>", escape(note));
    }
    out.push_str("</div></header>");
    if widget.empty {
        let _ = write!(
            out,
            "<p class=\"empty\">{}</p>",
            escape(&text(lang, "no_data"))
        );
    } else {
        lines(out, &widget.lines);
        tiles(out, &widget.tiles, "tile");
        if let Some(line) = &widget.chart {
            chart(out, line, number, lang);
        }
        bars(
            out,
            widget.bars_title.as_deref(),
            &widget.bars,
            widget.bars_scale,
        );
        grids(out, &widget.grids);
        for each in &widget.tables {
            table(out, each);
        }
    }
    out.push_str("</section>");
}

/// Grids as squares, north at the top, each point's place in its colour.
fn grids(out: &mut String, grids: &[GridImage]) {
    if grids.is_empty() {
        return;
    }
    out.push_str("<div class=\"grids\">");
    for grid in grids {
        let side = grid.size.max(1) * 40;
        let _ = write!(
            out,
            "<figure class=\"grid\"><svg viewBox=\"0 0 {side} {side}\" role=\"img\" aria-label=\"{}\">",
            escape(&grid.title)
        );
        for (row, col, label, band) in &grid.cells {
            let (x, y) = (col * 40, row * 40);
            let _ = write!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"36\" height=\"36\" rx=\"7\" class=\"band-{band}\"/><text x=\"{}\" y=\"{}\">{}</text>",
                x + 2,
                y + 2,
                x + 20,
                y + 25,
                escape(label)
            );
        }
        let _ = write!(
            out,
            "</svg><figcaption><strong>{}</strong><span>{}</span></figcaption></figure>",
            escape(&grid.title),
            escape(&grid.note)
        );
    }
    out.push_str("</div>");
}

/// The first page: brand, title, project and period, highlights, contents.
fn cover(out: &mut String, view: &ReportView, dark: bool, lang: &str) {
    let branding = &view.branding;
    out.push_str("<section class=\"cover\"><div class=\"cover-art\" aria-hidden=\"true\"></div><header class=\"cover-top\"><div class=\"brand\">");
    if let Some(logo) = &branding.logo {
        let _ = write!(out, "<img src=\"{}\" alt=\"\">", escape(logo));
    } else if branding.company.is_none() && !branding.hide_octopuson {
        out.push_str(if dark { MARK_LIGHT } else { MARK });
    }
    if branding.company.is_some() || branding.website.is_some() {
        out.push_str("<div>");
        if let Some(company) = &branding.company {
            let _ = write!(out, "<strong>{}</strong>", escape(company));
        }
        if let Some(website) = &branding.website {
            let shown = website
                .split_once("://")
                .map_or(website.as_str(), |(_, rest)| rest)
                .trim_end_matches('/');
            let _ = write!(out, "<span>{}</span>", escape(shown));
        }
        out.push_str("</div>");
    } else if branding.logo.is_none() && !branding.hide_octopuson {
        out.push_str("<div><strong>OctopusOn SEO</strong></div>");
    }
    let _ = write!(
        out,
        "</div><span class=\"cover-date\">{}: {}</span></header>",
        escape(&text(lang, "prepared_on")),
        escape(&date(&view.generated, lang)),
    );
    let _ = write!(
        out,
        "<div class=\"cover-main\"><span class=\"kicker\">{}</span><h1>{}</h1><p class=\"cover-site\"><strong>{}</strong> · {}</p><p class=\"cover-period\">{}: {} – {}</p></div>",
        escape(&text(lang, "report_kicker")),
        escape(&view.name),
        escape(&view.project),
        escape(&view.domain),
        escape(&text(lang, "period")),
        escape(&date(&view.period.0, lang)),
        escape(&date(&view.period.1, lang)),
    );
    let highlights: Vec<Tile> = view
        .widgets
        .iter()
        .filter(|widget| !widget.empty)
        .flat_map(|widget| widget.tiles.iter().cloned())
        .take(HIGHLIGHTS)
        .collect();
    tiles(out, &highlights, "highlight");
    let _ = write!(
        out,
        "<div class=\"cover-bottom\"><div><p class=\"contents-title\">{}</p><ol class=\"contents\">",
        escape(&text(lang, "contents"))
    );
    for (index, part) in view.widgets.iter().enumerate() {
        let _ = write!(
            out,
            "<li><b>{:02}</b><span>{}</span></li>",
            index + 1,
            escape(&part.title)
        );
    }
    out.push_str("</ol></div></div></section>");
}

/// The report as a whole HTML page.
pub(crate) fn render(view: &ReportView) -> String {
    page(view, fonts())
}

/// The page without the embedded font (the demo adds it once for every design).
#[cfg(test)]
pub(super) fn render_without_fonts(view: &ReportView) -> String {
    page(view, "")
}

fn page(view: &ReportView, fonts: &str) -> String {
    let lang = view.language.as_str();
    let design = design_of(view);
    let accent = accent(view, design);
    let branding = &view.branding;
    let colors = &view.colors;
    // The cover's ink: its own colour's, else the design's.
    let cover_ink = match (colors.cover.as_deref().filter(|c| is_color(c)), design) {
        (Some(cover), _) => ink_on(cover),
        (None, "ocean") => WHITE,
        (None, "accent") => ink_on(accent),
        (None, _) => INK,
    };
    let mut classes = format!("report design-{design}");
    let mut style = format!(
        "--accent:{accent};--on-accent:{};--cover-ink:{cover_ink}",
        ink_on(accent)
    );
    if let Some(cover) = colors.cover.as_deref().filter(|c| is_color(c)) {
        let tone = if cover_ink == WHITE { "dark" } else { "light" };
        let _ = write!(classes, " custom-cover cover-{tone}");
        let _ = write!(style, ";--cover-bg:{cover}");
    }
    if let Some(table) = colors.table.as_deref().filter(|c| is_color(c)) {
        classes.push_str(" custom-table");
        let _ = write!(style, ";--table-head:{table};--table-ink:{}", ink_on(table));
    }
    for (name, value) in [("accent-2", &colors.accent2), ("heading", &colors.heading)] {
        if let Some(value) = value.as_deref().filter(|c| is_color(c)) {
            let _ = write!(style, ";--{name}:{value}");
        }
    }
    let mut out = String::with_capacity(160 * 1024);
    let _ = write!(
        out,
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'\">\
         <meta name=\"robots\" content=\"noindex,nofollow\"><meta name=\"referrer\" content=\"no-referrer\">\
         <title>{} · {}</title><style>{}{STYLE}{}{CUSTOM}</style></head><body>\
         <div class=\"{classes}\" style=\"{style}\">",
        escape(&view.name),
        escape(&view.domain),
        fonts,
        design_style(design),
    );
    cover(&mut out, view, cover_ink == WHITE, lang);
    out.push_str("<main class=\"content\">");
    for (index, part) in view.widgets.iter().enumerate() {
        widget(&mut out, part, index + 1, lang);
    }
    out.push_str("<footer><span>");
    if let Some(footer) = &branding.footer {
        out.push_str(&escape(footer));
    }
    out.push_str("</span>");
    if !branding.hide_octopuson {
        let _ = write!(
            out,
            "<span class=\"made\">{MARK}{}</span>",
            escape(&text(lang, "prepared_with"))
        );
    }
    out.push_str("</footer></main></div></body></html>");
    out
}
