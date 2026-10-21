//! A report as one self-contained HTML page: the agency's branding, every part with its tiles,
//! an SVG chart, bars and tables, styled inline for the screen and for A4 print. Its own
//! Content-Security-Policy forbids every request and script, so the page shows the same in an
//! app preview, behind a share link and in the PDF printer.

use std::fmt::Write as _;

use super::texts::{date, text};
use super::{Bar, Cell, Chart, ReportView, Table, Tile, Widget};

/// The accent colour when the agency has none.
const ACCENT: &str = "#1e6fd9";

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

/// An accent that is a `#rrggbb` colour, else the default.
fn accent(view: &ReportView) -> &str {
    view.branding
        .accent
        .as_deref()
        .filter(|accent| {
            accent.len() == 7
                && accent.starts_with('#')
                && accent[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
        .unwrap_or(ACCENT)
}

const STYLE: &str = r#"
@page { size: A4; margin: 14mm 12mm 16mm; }
* { box-sizing: border-box; }
html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
body { margin: 0; background: #eef2f7; color: #18202e; font: 13px/1.5 -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; }
.report { max-width: 920px; margin: 24px auto; padding: 32px 36px; background: #fff; border-radius: 14px; box-shadow: 0 10px 40px rgba(20, 32, 60, 0.08); }
.cover { display: flex; gap: 20px; align-items: flex-start; justify-content: space-between; padding-bottom: 18px; border-bottom: 3px solid var(--accent); margin-bottom: 22px; }
.brand { display: flex; gap: 12px; align-items: center; min-width: 0; }
.brand img { max-height: 54px; max-width: 180px; object-fit: contain; }
.brand strong { display: block; font-size: 15px; }
.brand span { color: #5b6476; font-size: 12px; }
.title { text-align: right; }
.title h1 { margin: 0 0 4px; font-size: 22px; line-height: 1.2; color: #0f1726; }
.title p { margin: 0; color: #5b6476; font-size: 12px; }
.widget { margin: 0 0 26px; }
.widget h2 { margin: 0 0 2px; font-size: 16px; color: var(--accent); break-after: avoid; }
.note { margin: 0 0 10px; color: #6b7385; font-size: 11px; }
.empty { margin: 6px 0 0; color: #6b7385; font-style: italic; }
.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; margin: 10px 0 14px; break-inside: avoid; }
.tile { padding: 10px 12px; border: 1px solid #e3e8f0; border-radius: 10px; background: #f9fbfd; }
.tile span { display: block; color: #5b6476; font-size: 11px; text-transform: uppercase; letter-spacing: .03em; }
.tile strong { display: block; margin-top: 2px; font-size: 20px; color: #0f1726; font-variant-numeric: tabular-nums; }
.tile em { font-style: normal; font-size: 11px; font-weight: 600; }
.good { color: #0f8a5f; }
.bad { color: #c2410c; }
figure { margin: 6px 0 16px; break-inside: avoid; }
figcaption { color: #5b6476; font-size: 11px; margin-bottom: 4px; }
svg { display: block; width: 100%; height: auto; }
.legend { display: flex; gap: 14px; color: #5b6476; font-size: 11px; margin-top: 4px; }
.legend i { display: inline-block; width: 18px; height: 0; border-top: 2px solid var(--accent); vertical-align: middle; margin-right: 6px; }
.legend i.dashed { border-top: 2px dashed #94a3b8; }
.bars { margin: 6px 0 16px; padding: 0; list-style: none; break-inside: avoid; }
.bars li { display: grid; grid-template-columns: minmax(110px, 34%) 1fr 64px; gap: 10px; align-items: center; margin: 4px 0; }
.bars .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bars .own .label { font-weight: 700; }
.track { height: 9px; border-radius: 999px; background: #edf1f7; overflow: hidden; }
.fill { display: block; height: 100%; border-radius: 999px; background: #94a3b8; }
.own .fill { background: var(--accent); }
.bars .value { text-align: right; font-variant-numeric: tabular-nums; }
h3 { margin: 12px 0 6px; font-size: 12px; color: #39414f; break-after: avoid; }
table { width: 100%; border-collapse: collapse; font-size: 11.5px; margin-bottom: 8px; }
thead { display: table-header-group; }
th { text-align: left; color: #5b6476; font-weight: 600; border-bottom: 1px solid #d7dde7; padding: 5px 6px; }
td { border-bottom: 1px solid #eef1f6; padding: 5px 6px; vertical-align: top; overflow-wrap: anywhere; }
td.number, th.number { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
tr { break-inside: avoid; }
footer { margin-top: 18px; padding-top: 10px; border-top: 1px solid #e3e8f0; color: #6b7385; font-size: 11px; display: flex; flex-wrap: wrap; gap: 6px 14px; justify-content: space-between; }
@media print { body { background: #fff; } .report { margin: 0; padding: 0; max-width: none; box-shadow: none; border-radius: 0; } .tiles { grid-template-columns: repeat(4, minmax(0, 1fr)); } }
@media (max-width: 640px) { .report { margin: 0; padding: 20px 16px; border-radius: 0; } .cover { flex-direction: column; } .title { text-align: left; } .bars li { grid-template-columns: minmax(80px, 34%) 1fr 56px; } }
"#;

fn tiles(out: &mut String, tiles: &[Tile]) {
    if tiles.is_empty() {
        return;
    }
    out.push_str("<div class=\"tiles\">");
    for tile in tiles {
        let _ = write!(
            out,
            "<div class=\"tile\"><span>{}</span><strong>{}</strong>",
            escape(&tile.label),
            escape(&tile.value)
        );
        if let Some(change) = &tile.change {
            let _ = write!(
                out,
                "<em class=\"{}\">{}</em>",
                if change.good { "good" } else { "bad" },
                escape(&change.text)
            );
        }
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

#[allow(clippy::cast_precision_loss)] // point counts
fn chart(out: &mut String, chart: &Chart, accent: &str, lang: &str) {
    const WIDTH: f64 = 680.0;
    const HEIGHT: f64 = 190.0;
    const LEFT: f64 = 46.0;
    const BOTTOM: f64 = 22.0;
    const TOP: f64 = 8.0;
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
    let x = |index: usize| LEFT + (WIDTH - LEFT - 8.0) * index as f64 / (points - 1) as f64;
    let y = |value: f64| TOP + (HEIGHT - TOP - BOTTOM) * (1.0 - value / top);
    let _ = write!(
        out,
        "<figure><figcaption>{}</figcaption><svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=\"img\" aria-label=\"{}\">",
        escape(&chart.title),
        escape(&chart.title)
    );
    for step in 0..=2 {
        let value = top * f64::from(step) / 2.0;
        let line = y(value);
        let _ = write!(
            out,
            "<line x1=\"{LEFT}\" x2=\"{}\" y1=\"{line:.1}\" y2=\"{line:.1}\" stroke=\"#e6ebf2\"/><text x=\"{}\" y=\"{:.1}\" font-size=\"10\" fill=\"#6b7385\" text-anchor=\"end\">{}</text>",
            WIDTH - 8.0,
            LEFT - 6.0,
            line + 3.5,
            escape(&super::texts::number(value, usize::from(top < 10.0), lang))
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
            "<text x=\"{:.1}\" y=\"{}\" font-size=\"10\" fill=\"#6b7385\" text-anchor=\"{anchor}\">{}</text>",
            x(index),
            HEIGHT - 6.0,
            escape(&chart.labels[index])
        );
    }
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
        let (color, dash, width) = if *dashed {
            ("#94a3b8", " stroke-dasharray=\"5 4\"", 1.5)
        } else {
            (accent, "", 2.2)
        };
        let _ = write!(
            out,
            "<path d=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width}\" stroke-linejoin=\"round\"{dash}/>",
            path.trim_end()
        );
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
        let _ = write!(out, "<h3>{}</h3>", escape(title));
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
        let _ = write!(out, "<h3>{}</h3>", escape(&table.title));
    }
    out.push_str("<table><thead><tr>");
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
    out.push_str("</tbody></table>");
}

fn widget(out: &mut String, widget: &Widget, accent: &str, lang: &str) {
    let _ = write!(
        out,
        "<section class=\"widget\" id=\"{}\"><h2>{}</h2>",
        escape(&widget.id),
        escape(&widget.title)
    );
    if let Some(note) = &widget.note {
        let _ = write!(out, "<p class=\"note\">{}</p>", escape(note));
    }
    if widget.empty {
        let _ = write!(
            out,
            "<p class=\"empty\">{}</p>",
            escape(&text(lang, "no_data"))
        );
    } else {
        tiles(out, &widget.tiles);
        if let Some(line) = &widget.chart {
            chart(out, line, accent, lang);
        }
        bars(
            out,
            widget.bars_title.as_deref(),
            &widget.bars,
            widget.bars_scale,
        );
        for each in &widget.tables {
            table(out, each);
        }
    }
    out.push_str("</section>");
}

/// The report as a whole HTML page.
pub(crate) fn render(view: &ReportView) -> String {
    let lang = view.language.as_str();
    let accent = accent(view);
    let branding = &view.branding;
    let mut out = String::with_capacity(32 * 1024);
    let _ = write!(
        out,
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">\
         <meta name=\"robots\" content=\"noindex,nofollow\"><meta name=\"referrer\" content=\"no-referrer\">\
         <title>{} · {}</title><style>:root{{--accent:{accent};}}{STYLE}</style></head><body><main class=\"report\">",
        escape(&view.name),
        escape(&view.domain),
    );
    out.push_str("<header class=\"cover\"><div class=\"brand\">");
    if let Some(logo) = &branding.logo {
        let _ = write!(out, "<img src=\"{}\" alt=\"\">", escape(logo));
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
    }
    let _ = write!(
        out,
        "</div><div class=\"title\"><h1>{}</h1><p>{} · {}</p><p>{}: {} – {} · {}: {}</p></div></header>",
        escape(&view.name),
        escape(&view.project),
        escape(&view.domain),
        escape(&text(lang, "period")),
        escape(&date(&view.period.0, lang)),
        escape(&date(&view.period.1, lang)),
        escape(&text(lang, "prepared_on")),
        escape(&date(&view.generated, lang)),
    );
    for part in &view.widgets {
        widget(&mut out, part, accent, lang);
    }
    out.push_str("<footer><span>");
    if let Some(footer) = &branding.footer {
        out.push_str(&escape(footer));
    }
    out.push_str("</span>");
    if !branding.hide_octopuson {
        let _ = write!(out, "<span>{}</span>", escape(&text(lang, "prepared_with")));
    }
    out.push_str("</footer></main></body></html>");
    out
}
