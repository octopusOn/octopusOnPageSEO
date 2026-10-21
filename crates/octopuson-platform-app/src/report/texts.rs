//! The words of a report in its language (Turkish, English, German, Spanish), and numbers
//! written the way that language writes them.

/// `(key, tr, en, de, es)`.
const TEXTS: &[(&str, &str, &str, &str, &str)] = &[
    ("summary", "Özet", "Summary", "Übersicht", "Resumen"),
    (
        "rankings",
        "Sıralamalar",
        "Rankings",
        "Rankings",
        "Posiciones",
    ),
    (
        "competitors",
        "Rakiplere karşı görünürlük",
        "Visibility against competitors",
        "Sichtbarkeit gegenüber Mitbewerbern",
        "Visibilidad frente a la competencia",
    ),
    (
        "search_console",
        "Search Console",
        "Search Console",
        "Search Console",
        "Search Console",
    ),
    (
        "site_audit",
        "Site denetimi",
        "Site audit",
        "Website-Audit",
        "Auditoría del sitio",
    ),
    (
        "ai_visibility",
        "AI görünürlüğü",
        "AI visibility",
        "KI-Sichtbarkeit",
        "Visibilidad en IA",
    ),
    (
        "backlinks",
        "Backlink'ler",
        "Backlinks",
        "Backlinks",
        "Backlinks",
    ),
    ("period", "Dönem", "Period", "Zeitraum", "Periodo"),
    (
        "prepared_on",
        "Hazırlanma tarihi",
        "Prepared on",
        "Erstellt am",
        "Preparado el",
    ),
    (
        "prepared_with",
        "OctopusOn SEO ile hazırlandı",
        "Prepared with OctopusOn SEO",
        "Erstellt mit OctopusOn SEO",
        "Preparado con OctopusOn SEO",
    ),
    (
        "no_data",
        "Bu bölüm için henüz veri yok.",
        "No data for this part yet.",
        "Für diesen Teil gibt es noch keine Daten.",
        "Aún no hay datos para esta parte.",
    ),
    (
        "visibility",
        "Görünürlük",
        "Visibility",
        "Sichtbarkeit",
        "Visibilidad",
    ),
    (
        "average_position",
        "Ortalama sıra",
        "Average position",
        "Durchschnittliche Position",
        "Posición media",
    ),
    (
        "ranked",
        "İlk 100'deki kelimeler",
        "Keywords in the top 100",
        "Keywords in den Top 100",
        "Palabras clave en el top 100",
    ),
    (
        "top3_top10",
        "İlk 3 / ilk 10",
        "Top 3 / top 10",
        "Top 3 / Top 10",
        "Top 3 / top 10",
    ),
    (
        "improved",
        "Yükselen",
        "Improved",
        "Verbessert",
        "Mejoradas",
    ),
    (
        "declined",
        "Düşen",
        "Declined",
        "Verschlechtert",
        "Empeoradas",
    ),
    ("keyword", "Kelime", "Keyword", "Keyword", "Palabra clave"),
    ("position", "Sıra", "Position", "Position", "Posición"),
    ("change", "Değişim", "Change", "Änderung", "Cambio"),
    ("best", "En iyi", "Best", "Beste", "Mejor"),
    ("volume", "Hacim", "Volume", "Volumen", "Volumen"),
    ("page", "Sayfa", "Page", "Seite", "Página"),
    (
        "top_keywords",
        "En çok aranan takip edilen kelimeler",
        "Most searched tracked keywords",
        "Meistgesuchte verfolgte Keywords",
        "Palabras clave seguidas más buscadas",
    ),
    (
        "visibility_trend",
        "Görünürlük",
        "Visibility",
        "Sichtbarkeit",
        "Visibilidad",
    ),
    ("domain", "Alan adı", "Domain", "Domain", "Dominio"),
    ("share", "Pay", "Share", "Anteil", "Cuota"),
    (
        "keywords",
        "Kelimeler",
        "Keywords",
        "Keywords",
        "Palabras clave",
    ),
    ("clicks", "Tıklama", "Clicks", "Klicks", "Clics"),
    (
        "impressions",
        "Gösterim",
        "Impressions",
        "Impressionen",
        "Impresiones",
    ),
    ("ctr", "TO", "CTR", "CTR", "CTR"),
    (
        "organic_sessions",
        "Organik oturum",
        "Organic sessions",
        "Organische Sitzungen",
        "Sesiones orgánicas",
    ),
    (
        "key_events",
        "Önemli etkinlik",
        "Key events",
        "Schlüsselereignisse",
        "Eventos clave",
    ),
    (
        "clicks_daily",
        "Günlük tıklama (kesikli: önceki dönem)",
        "Daily clicks (dashed: period before)",
        "Tägliche Klicks (gestrichelt: Vorzeitraum)",
        "Clics diarios (discontinua: periodo anterior)",
    ),
    (
        "top_queries",
        "En çok tıklanan sorgular",
        "Top queries by clicks",
        "Suchanfragen mit den meisten Klicks",
        "Consultas con más clics",
    ),
    (
        "top_pages",
        "En çok tıklanan sayfalar",
        "Top pages by clicks",
        "Seiten mit den meisten Klicks",
        "Páginas con más clics",
    ),
    ("query", "Sorgu", "Query", "Suchanfrage", "Consulta"),
    ("health", "Sağlık", "Health", "Zustand", "Salud"),
    (
        "geo_readiness",
        "GEO hazırlığı",
        "GEO readiness",
        "GEO-Bereitschaft",
        "Preparación GEO",
    ),
    (
        "pages_crawled",
        "Taranan sayfa",
        "Pages crawled",
        "Gecrawlte Seiten",
        "Páginas rastreadas",
    ),
    ("findings", "Bulgu", "Findings", "Befunde", "Hallazgos"),
    (
        "top_issues",
        "Öne çıkan sorunlar",
        "Main issues",
        "Wichtigste Probleme",
        "Problemas principales",
    ),
    ("issue", "Sorun", "Issue", "Problem", "Problema"),
    ("count", "Adet", "Count", "Anzahl", "Cantidad"),
    (
        "mention_rate",
        "Anılma oranı",
        "Mention rate",
        "Nennungsrate",
        "Tasa de mención",
    ),
    (
        "citation_rate",
        "Kaynak gösterilme",
        "Citation rate",
        "Zitierrate",
        "Tasa de cita",
    ),
    (
        "share_of_voice",
        "Ses payı",
        "Share of voice",
        "Share of Voice",
        "Cuota de voz",
    ),
    (
        "by_assistant",
        "Asistanlara göre anılma oranı",
        "Mention rate by assistant",
        "Nennungsrate nach Assistent",
        "Tasa de mención por asistente",
    ),
    (
        "brands",
        "Markaların ses payı",
        "Share of voice by brand",
        "Share of Voice nach Marke",
        "Cuota de voz por marca",
    ),
    (
        "authority",
        "Otorite (OA)",
        "Authority (OA)",
        "Autorität (OA)",
        "Autoridad (OA)",
    ),
    (
        "backlink_count",
        "Backlink",
        "Backlinks",
        "Backlinks",
        "Backlinks",
    ),
    (
        "referring_domains",
        "Referans alan adı",
        "Referring domains",
        "Verweisende Domains",
        "Dominios de referencia",
    ),
    (
        "broken_backlinks",
        "Kırık backlink",
        "Broken backlinks",
        "Defekte Backlinks",
        "Backlinks rotos",
    ),
    (
        "organic_traffic",
        "Organik trafik (tahmini)",
        "Organic traffic (estimated)",
        "Organischer Traffic (geschätzt)",
        "Tráfico orgánico (estimado)",
    ),
    (
        "organic_keywords",
        "Organik kelime",
        "Organic keywords",
        "Organische Keywords",
        "Palabras clave orgánicas",
    ),
    (
        "vs_before",
        "önceki döneme göre",
        "vs the period before",
        "ggü. Vorzeitraum",
        "frente al periodo anterior",
    ),
    (
        "email_attached",
        "{name} raporu ektedir.",
        "The {name} report is attached.",
        "Der Bericht {name} ist angehängt.",
        "Adjuntamos el informe {name}.",
    ),
    (
        "email_html_attached",
        "{name} raporu ektedir; tarayıcıda açabilirsiniz.",
        "The {name} report is attached; open it in a browser.",
        "Der Bericht {name} ist angehängt; öffnen Sie ihn im Browser.",
        "Adjuntamos el informe {name}; ábralo en un navegador.",
    ),
    (
        "email_online",
        "Çevrimiçi görüntüleyin: {link}",
        "View it online: {link}",
        "Online ansehen: {link}",
        "Véalo en línea: {link}",
    ),
];

fn column(lang: &str) -> usize {
    match lang {
        "tr" => 0,
        "de" => 2,
        "es" => 3,
        _ => 1,
    }
}

/// A word of the report in `lang`; the key itself when unknown.
pub(crate) fn text(lang: &str, key: &str) -> String {
    TEXTS.iter().find(|(name, ..)| *name == key).map_or_else(
        || key.to_owned(),
        |&(_, tr, en, de, es)| [tr, en, de, es][column(lang)].to_owned(),
    )
}

/// A word with `{name}` placeholders filled.
pub(crate) fn fill(lang: &str, key: &str, values: &[(&str, &str)]) -> String {
    values.iter().fold(text(lang, key), |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

/// A number with grouping and `decimals` places, as `lang` writes it (1.234,5 or 1,234.5).
pub(crate) fn number(value: f64, decimals: usize, lang: &str) -> String {
    if !value.is_finite() {
        return "—".to_owned();
    }
    let (group, point) = if lang == "en" { (',', '.') } else { ('.', ',') };
    let text = format!("{:.*}", decimals, value.abs());
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(group);
        }
        grouped.push(digit);
    }
    let sign = if value < 0.0 && text.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
        "-"
    } else {
        ""
    };
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{point}{fraction}")
    }
}

/// A percentage given as 0–100, the way `lang` writes it (%12,5 in Turkish).
pub(crate) fn percent(value: f64, lang: &str) -> String {
    let digits = usize::from(value.abs() < 10.0);
    let text = number(value, digits, lang);
    if lang == "tr" {
        format!("%{text}")
    } else if lang == "en" {
        format!("{text}%")
    } else {
        format!("{text} %")
    }
}

/// A day `YYYY-MM-DD` as `lang` writes dates (21.10.2026, 21/10/2026, 2026-10-21).
pub(crate) fn date(day: &str, lang: &str) -> String {
    let parts: Vec<&str> = day.get(..10).unwrap_or(day).split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return day.to_owned();
    };
    match lang {
        "en" | "es" => format!("{day}/{month}/{year}"),
        _ => format!("{day}.{month}.{year}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_numbers_dates_and_words_by_language() {
        assert_eq!(number(1_234_567.891, 1, "tr"), "1.234.567,9");
        assert_eq!(number(1_234_567.891, 0, "en"), "1,234,568");
        assert_eq!(number(-12.5, 1, "de"), "-12,5");
        assert_eq!(number(-0.01, 1, "en"), "0.0");
        assert_eq!(percent(12.6, "tr"), "%13");
        assert_eq!(percent(2.54, "en"), "2.5%");
        assert_eq!(percent(2.54, "de"), "2,5 %");
        assert_eq!(date("2026-10-21", "de"), "21.10.2026");
        assert_eq!(date("2026-10-21T09:00:00Z", "en"), "21/10/2026");
        assert_eq!(text("es", "clicks"), "Clics");
        assert_eq!(
            text("fr", "clicks"),
            "Clicks",
            "unknown languages read English"
        );
        assert_eq!(text("tr", "missing"), "missing");
        assert_eq!(
            fill("en", "email_online", &[("link", "https://x")]),
            "View it online: https://x"
        );
        let languages = TEXTS
            .iter()
            .all(|(_, tr, en, de, es)| [tr, en, de, es].iter().all(|text| !text.is_empty()));
        assert!(languages);
    }
}
