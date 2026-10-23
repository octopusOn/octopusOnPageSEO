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
    (
        "local",
        "Yerel SEO",
        "Local SEO",
        "Lokales SEO",
        "SEO local",
    ),
    (
        "local_note",
        "{business} · çevresinde {n}×{n} noktalık, {km} km'lik Google Haritalar ızgarası",
        "{business} · a {n}×{n} Google Maps grid reaching {km} km around it",
        "{business} · ein {n}×{n}-Google-Maps-Raster bis {km} km rundherum",
        "{business} · una cuadrícula {n}×{n} de Google Maps hasta {km} km a su alrededor",
    ),
    (
        "local_arp",
        "Haritada ortalama sıra",
        "Average map rank",
        "Durchschnittlicher Kartenrang",
        "Posición media en el mapa",
    ),
    (
        "local_solv",
        "Yerel ses payı",
        "Share of local voice",
        "Lokaler Sichtbarkeitsanteil",
        "Cuota de voz local",
    ),
    (
        "local_found",
        "Göründüğü noktalar",
        "Points it shows at",
        "Punkte mit Präsenz",
        "Puntos donde aparece",
    ),
    (
        "local_rating",
        "Google puanı",
        "Google rating",
        "Google-Bewertung",
        "Puntuación en Google",
    ),
    (
        "local_arp_short",
        "Ort. sıra",
        "Avg. rank",
        "Ø Rang",
        "Pos. media",
    ),
    (
        "local_solv_short",
        "Ses payı",
        "Local share",
        "Anteil",
        "Cuota",
    ),
    (
        "local_rivals",
        "Yerel pakette sizi geçenler",
        "Who beats you in the local pack",
        "Wer Sie im Local Pack überholt",
        "Quién le supera en el paquete local",
    ),
    (
        "local_business",
        "İşletme",
        "Business",
        "Unternehmen",
        "Negocio",
    ),
    (
        "local_top3_points",
        "İlk 3'te nokta",
        "Points in top 3",
        "Punkte in Top 3",
        "Puntos en top 3",
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
        "alert_email_subject",
        "{project}: {n} yeni uyarı",
        "{project}: {n} new alerts",
        "{project}: {n} neue Warnungen",
        "{project}: {n} alertas nuevas",
    ),
    (
        "alert_email_intro",
        "{project} projesinde {n} yeni uyarı var:",
        "{project} has {n} new alerts:",
        "{project} hat {n} neue Warnungen:",
        "{project} tiene {n} alertas nuevas:",
    ),
    (
        "alert_email_link",
        "Hepsi: {link}",
        "All of them: {link}",
        "Alle: {link}",
        "Todas: {link}",
    ),
    (
        "alert_rank_drop",
        "{count} kelime düştü ({tracker}): {keywords}",
        "{count} keywords fell ({tracker}): {keywords}",
        "{count} Keywords gefallen ({tracker}): {keywords}",
        "{count} palabras clave cayeron ({tracker}): {keywords}",
    ),
    (
        "alert_visibility_drop",
        "Görünürlük %{drop} düştü ({tracker}): {before} → {now}",
        "Visibility fell {drop}% ({tracker}): {before} → {now}",
        "Sichtbarkeit um {drop} % gefallen ({tracker}): {before} → {now}",
        "La visibilidad cayó un {drop} % ({tracker}): {before} → {now}",
    ),
    (
        "alert_gsc_drop",
        "Search Console tıklamaları %{drop} düştü: {before} → {clicks}",
        "Search Console clicks fell {drop}%: {before} → {clicks}",
        "Search-Console-Klicks um {drop} % gefallen: {before} → {clicks}",
        "Los clics de Search Console cayeron un {drop} %: {before} → {clicks}",
    ),
    (
        "alert_audit_health",
        "Site sağlığı {drop} puan düştü: {before} → {now}",
        "Site health fell {drop} points: {before} → {now}",
        "Website-Gesundheit um {drop} Punkte gefallen: {before} → {now}",
        "La salud del sitio cayó {drop} puntos: {before} → {now}",
    ),
    (
        "alert_audit_issues",
        "{count} yeni yüksek öncelikli sorun: {titles}",
        "{count} new high-priority problems: {titles}",
        "{count} neue Probleme mit hoher Priorität: {titles}",
        "{count} problemas nuevos de prioridad alta: {titles}",
    ),
    (
        "alert_ai_drop",
        "Yapay zekâda anılma {change} puan düştü: %{before} → %{now}",
        "AI mentions fell {change} points: {before}% → {now}%",
        "KI-Erwähnungen um {change} Punkte gefallen: {before} % → {now} %",
        "Las menciones en IA cayeron {change} puntos: {before} % → {now} %",
    ),
    (
        "alert_ai_gain",
        "Yapay zekâda anılma {change} puan arttı: %{before} → %{now}",
        "AI mentions rose {change} points: {before}% → {now}%",
        "KI-Erwähnungen um {change} Punkte gestiegen: {before} % → {now} %",
        "Las menciones en IA subieron {change} puntos: {before} % → {now} %",
    ),
    (
        "alert_local_drop",
        "Haritada görünürlük düştü ({count} kelime): {keywords}",
        "Map visibility fell ({count} keywords): {keywords}",
        "Kartensichtbarkeit gefallen ({count} Keywords): {keywords}",
        "La visibilidad en el mapa cayó ({count} palabras clave): {keywords}",
    ),
    (
        "alert_review",
        "{rating} yıldızlı yeni yorum ({author}): {text}",
        "New {rating}-star review ({author}): {text}",
        "Neue {rating}-Sterne-Bewertung ({author}): {text}",
        "Nueva reseña de {rating} estrellas ({author}): {text}",
    ),
    (
        "alert_rival_growth",
        "{domain} kelimelerini %{growth} artırdı (+{gained}); siz %{own}",
        "{domain} grew its keywords {growth}% (+{gained}); you {own}%",
        "{domain} steigerte seine Keywords um {growth} % (+{gained}); Sie {own} %",
        "{domain} aumentó sus palabras clave un {growth} % (+{gained}); tú {own} %",
    ),
    (
        "alert_links",
        "{new} yeni, {lost} kayıp link veren site: {domains}",
        "{new} new and {lost} lost linking sites: {domains}",
        "{new} neue und {lost} verlorene verlinkende Websites: {domains}",
        "{new} sitios que enlazan nuevos y {lost} perdidos: {domains}",
    ),
    (
        "email_online",
        "Çevrimiçi görüntüleyin: {link}",
        "View it online: {link}",
        "Online ansehen: {link}",
        "Véalo en línea: {link}",
    ),
    (
        "comparison",
        "Önce / sonra: başlangıçtan bugüne",
        "Before / after: since the start",
        "Vorher / nachher: seit Beginn",
        "Antes / después: desde el inicio",
    ),
    (
        "comparison_previous",
        "Önce / sonra: son anlık görüntüden bu yana",
        "Before / after: since the last snapshot",
        "Vorher / nachher: seit dem letzten Snapshot",
        "Antes / después: desde la última instantánea",
    ),
    (
        "report_kicker",
        "SEO ve GEO performans raporu",
        "SEO & GEO performance report",
        "SEO- & GEO-Leistungsbericht",
        "Informe de rendimiento SEO y GEO",
    ),
    (
        "contents",
        "Bu raporda",
        "In this report",
        "In diesem Bericht",
        "En este informe",
    ),
    (
        "snapshot_comparison",
        "Anlık görüntü karşılaştırması",
        "Snapshot comparison",
        "Snapshot-Vergleich",
        "Comparación de instantáneas",
    ),
    (
        "comparison_period",
        "{from} → {to} · {days} gün",
        "{from} → {to} · {days} days",
        "{from} → {to} · {days} Tage",
        "{from} → {to} · {days} días",
    ),
    (
        "no_comparison",
        "Karşılaştırma için projede en az iki anlık görüntü olmalı.",
        "Comparing needs at least two snapshots of the project.",
        "Für einen Vergleich braucht das Projekt mindestens zwei Snapshots.",
        "Para comparar, el proyecto necesita al menos dos instantáneas.",
    ),
    ("was", "önce", "was", "vorher", "antes"),
    ("metric", "Ölçüt", "Metric", "Kennzahl", "Métrica"),
    ("before", "Önce", "Before", "Vorher", "Antes"),
    ("after", "Sonra", "After", "Nachher", "Después"),
    (
        "change_percent",
        "Değişim %",
        "Change %",
        "Veränderung %",
        "Cambio %",
    ),
    (
        "all_metrics",
        "Tüm ölçütler",
        "All metrics",
        "Alle Kennzahlen",
        "Todas las métricas",
    ),
    (
        "you_and_competitors",
        "Siz ve rakipleriniz",
        "You and your competitors",
        "Sie und Ihre Mitbewerber",
        "Usted y su competencia",
    ),
    (
        "improved_keywords",
        "Yükselen kelimeler",
        "Keywords that moved up",
        "Gestiegene Keywords",
        "Palabras clave que subieron",
    ),
    (
        "declined_keywords",
        "Düşen kelimeler",
        "Keywords that moved down",
        "Gefallene Keywords",
        "Palabras clave que bajaron",
    ),
    (
        "new_keywords",
        "Yeni sıralanan kelimeler",
        "New ranking keywords",
        "Neu rankende Keywords",
        "Nuevas palabras clave posicionadas",
    ),
    (
        "lost_keywords",
        "Sıralaması kaybolan kelimeler",
        "Keywords no longer ranking",
        "Nicht mehr rankende Keywords",
        "Palabras clave que dejaron de posicionar",
    ),
    (
        "pages_up",
        "Trafiği artan sayfalar",
        "Pages gaining traffic",
        "Seiten mit mehr Traffic",
        "Páginas que ganan tráfico",
    ),
    (
        "pages_down",
        "Trafiği azalan sayfalar",
        "Pages losing traffic",
        "Seiten mit weniger Traffic",
        "Páginas que pierden tráfico",
    ),
    (
        "domains_gained",
        "Yeni link veren alan adları",
        "New referring domains",
        "Neue verweisende Domains",
        "Nuevos dominios de referencia",
    ),
    (
        "domains_lost",
        "Linki kaybolan alan adları",
        "Lost referring domains",
        "Verlorene verweisende Domains",
        "Dominios de referencia perdidos",
    ),
    (
        "tracked_changes",
        "Takip edilen kelimelerde değişim",
        "Tracked keyword changes",
        "Veränderungen getrackter Keywords",
        "Cambios en las palabras clave seguidas",
    ),
    (
        "issues_fixed",
        "Düzelen sorunlar",
        "Fixed issues",
        "Behobene Probleme",
        "Problemas corregidos",
    ),
    (
        "issues_new",
        "Yeni çıkan sorunlar",
        "New issues",
        "Neue Probleme",
        "Problemas nuevos",
    ),
    (
        "ai_gained",
        "Markanın yeni anıldığı AI yanıtları",
        "AI answers that now name the brand",
        "KI-Antworten, die die Marke jetzt nennen",
        "Respuestas de IA que ahora citan la marca",
    ),
    (
        "ai_lost",
        "Markanın artık anılmadığı AI yanıtları",
        "AI answers that no longer name the brand",
        "KI-Antworten, die die Marke nicht mehr nennen",
        "Respuestas de IA que ya no citan la marca",
    ),
    ("prompt", "Soru", "Question", "Frage", "Pregunta"),
    (
        "assistant",
        "Asistan",
        "Assistant",
        "Assistent",
        "Asistente",
    ),
    ("traffic", "Trafik", "Traffic", "Traffic", "Tráfico"),
    (
        "traffic_value",
        "Trafik değeri",
        "Traffic value",
        "Traffic-Wert",
        "Valor del tráfico",
    ),
    (
        "kpi_top3",
        "İlk 3'teki kelimeler",
        "Keywords in the top 3",
        "Keywords in den Top 3",
        "Palabras clave en el top 3",
    ),
    (
        "kpi_top10",
        "İlk 10'daki kelimeler",
        "Keywords in the top 10",
        "Keywords in den Top 10",
        "Palabras clave en el top 10",
    ),
    (
        "tracked_top10",
        "Takip edilenler (ilk 10)",
        "Tracked in the top 10",
        "Getrackt in den Top 10",
        "Seguidas en el top 10",
    ),
    (
        "gsc_position",
        "Search Console ortalama sıra",
        "Search Console average position",
        "Search-Console-Durchschnittsposition",
        "Posición media en Search Console",
    ),
    (
        "organic",
        "Organik arama",
        "Organic search",
        "Organische Suche",
        "Búsqueda orgánica",
    ),
    (
        "keyword_lists",
        "Anahtar kelime listeleri",
        "Keyword lists",
        "Keyword-Listen",
        "Listas de palabras clave",
    ),
    (
        "link_history",
        "Backlink geçmişi",
        "Backlinks over time",
        "Backlinks im Zeitverlauf",
        "Backlinks en el tiempo",
    ),
    (
        "gap",
        "Rakiplerle kelime boşluğu",
        "Keyword gap with competitors",
        "Keyword-Lücke zu den Mitbewerbern",
        "Brecha de palabras clave con la competencia",
    ),
    (
        "opportunities",
        "Önce yapılacaklar",
        "What to do next",
        "Was als Nächstes zu tun ist",
        "Qué hacer a continuación",
    ),
    (
        "keywords_new",
        "Yeni kelimeler",
        "New keywords",
        "Neue Keywords",
        "Palabras clave nuevas",
    ),
    (
        "keywords_up",
        "Yükselen kelimeler",
        "Keywords up",
        "Gestiegene Keywords",
        "Palabras clave que suben",
    ),
    (
        "keywords_down",
        "Düşen kelimeler",
        "Keywords down",
        "Gefallene Keywords",
        "Palabras clave que bajan",
    ),
    (
        "keywords_lost",
        "Kaybedilen kelimeler",
        "Keywords lost",
        "Verlorene Keywords",
        "Palabras clave perdidas",
    ),
    (
        "organic_trend",
        "Aylık organik trafik",
        "Organic traffic by month",
        "Organischer Traffic pro Monat",
        "Tráfico orgánico por mes",
    ),
    (
        "top_organic_keywords",
        "En çok trafik getiren kelimeler",
        "Keywords bringing the most traffic",
        "Keywords mit dem meisten Traffic",
        "Palabras clave que más tráfico traen",
    ),
    ("previous", "Önceki", "Before", "Vorher", "Antes"),
    (
        "domains_won",
        "Kazanılan alan adları",
        "Referring domains won",
        "Gewonnene verweisende Domains",
        "Dominios ganados",
    ),
    (
        "domains_lost_short",
        "Kaybedilen alan adları",
        "Referring domains lost",
        "Verlorene verweisende Domains",
        "Dominios perdidos",
    ),
    (
        "risky_sites",
        "Riskli siteler (yüksek / incelenen)",
        "Risky sites (high / judged)",
        "Riskante Websites (hoch / geprüft)",
        "Sitios de riesgo (alto / revisados)",
    ),
    (
        "disavowed",
        "Reddedilen siteler",
        "Sites disavowed",
        "Entwertete Websites",
        "Sitios desautorizados",
    ),
    (
        "prospects_open",
        "Süren link çalışmaları",
        "Link prospects in progress",
        "Laufende Link-Kandidaten",
        "Enlaces en curso",
    ),
    (
        "contacted",
        "iletişimde",
        "contacted",
        "kontaktiert",
        "contactados",
    ),
    (
        "prospects_won",
        "Kazanılan linkler",
        "Links won",
        "Gewonnene Links",
        "Enlaces conseguidos",
    ),
    (
        "referring_domains_trend",
        "Aylık referans alan adı sayısı",
        "Referring domains by month",
        "Verweisende Domains pro Monat",
        "Dominios de referencia por mes",
    ),
    (
        "links_by_period",
        "Kazanılan ve kaybedilen linkler",
        "Links won and lost",
        "Gewonnene und verlorene Links",
        "Enlaces ganados y perdidos",
    ),
    (
        "period_end",
        "Dönem sonu",
        "Period ending",
        "Zeitraum bis",
        "Periodo hasta",
    ),
    (
        "links_won",
        "Kazanılan backlink",
        "Backlinks won",
        "Gewonnene Backlinks",
        "Backlinks ganados",
    ),
    (
        "links_lost",
        "Kaybedilen backlink",
        "Backlinks lost",
        "Verlorene Backlinks",
        "Backlinks perdidos",
    ),
    (
        "risky_links",
        "En riskli link veren siteler",
        "Riskiest linking sites",
        "Riskanteste verlinkende Websites",
        "Sitios que enlazan con más riesgo",
    ),
    (
        "spam_score",
        "Spam puanı",
        "Spam score",
        "Spam-Score",
        "Puntuación de spam",
    ),
    ("risk", "Risk", "Risk", "Risiko", "Riesgo"),
    ("decision", "Karar", "Decision", "Entscheidung", "Decisión"),
    ("risk_high", "Yüksek", "High", "Hoch", "Alto"),
    ("risk_medium", "Orta", "Medium", "Mittel", "Medio"),
    ("risk_low", "Düşük", "Low", "Niedrig", "Bajo"),
    ("decision_keep", "Tut", "Keep", "Behalten", "Mantener"),
    ("decision_watch", "İzle", "Watch", "Beobachten", "Vigilar"),
    (
        "decision_disavow",
        "Reddet",
        "Disavow",
        "Entwerten",
        "Desautorizar",
    ),
    (
        "decision_none",
        "Karar yok",
        "Not decided",
        "Offen",
        "Sin decidir",
    ),
    (
        "gap_note",
        "{own} · karşılaştırılan: {competitors}",
        "{own} · compared with {competitors}",
        "{own} · verglichen mit {competitors}",
        "{own} · comparado con {competitors}",
    ),
    ("gap_shared", "Ortak", "Shared", "Gemeinsam", "Compartidas"),
    ("gap_missing", "Eksik", "Missing", "Fehlend", "Ausentes"),
    ("gap_weak", "Zayıf", "Weak", "Schwach", "Débiles"),
    ("gap_strong", "Güçlü", "Strong", "Stark", "Fuertes"),
    (
        "gap_untapped",
        "Kullanılmamış",
        "Untapped",
        "Ungenutzt",
        "Sin explotar",
    ),
    ("gap_unique", "Benzersiz", "Unique", "Einzigartig", "Únicas"),
    (
        "gap_missing_table",
        "Rakiplerin sıralanıp sizin sıralanmadığınız kelimeler",
        "Keywords competitors rank for and you don't",
        "Keywords, für die Mitbewerber ranken und Sie nicht",
        "Palabras clave en las que posiciona la competencia y tú no",
    ),
    (
        "gap_weak_table",
        "Rakiplerin sizden önde olduğu kelimeler",
        "Keywords where competitors rank above you",
        "Keywords, bei denen Mitbewerber vor Ihnen liegen",
        "Palabras clave en las que la competencia te supera",
    ),
    ("difficulty", "KD", "KD", "KD", "KD"),
    ("you", "Siz", "You", "Sie", "Tú"),
    (
        "best_rival",
        "En iyi rakip",
        "Best competitor",
        "Bester Mitbewerber",
        "Mejor competidor",
    ),
    ("lists", "Listeler", "Lists", "Listen", "Listas"),
    (
        "listed_keywords",
        "Listelenen kelimeler",
        "Keywords listed",
        "Gelistete Keywords",
        "Palabras clave en listas",
    ),
    (
        "total_volume",
        "Toplam aylık arama",
        "Monthly searches in all",
        "Suchanfragen pro Monat gesamt",
        "Búsquedas mensuales en total",
    ),
    (
        "list_size",
        "{n} kelime",
        "{n} keywords",
        "{n} Keywords",
        "{n} palabras clave",
    ),
    ("cpc", "TBM", "CPC", "CPC", "CPC"),
    ("intent", "Niyet", "Intent", "Intention", "Intención"),
    (
        "intent_informational",
        "Bilgi",
        "Informational",
        "Informational",
        "Informativa",
    ),
    (
        "intent_navigational",
        "Yönlendirme",
        "Navigational",
        "Navigational",
        "Navegacional",
    ),
    (
        "intent_commercial",
        "Ticari",
        "Commercial",
        "Kommerziell",
        "Comercial",
    ),
    (
        "intent_transactional",
        "İşlem",
        "Transactional",
        "Transaktional",
        "Transaccional",
    ),
    (
        "opportunities_open",
        "Açık fırsatlar",
        "Open opportunities",
        "Offene Chancen",
        "Oportunidades abiertas",
    ),
    (
        "opportunities_gain",
        "Tahmini ek tıklama / ay",
        "Estimated extra clicks / month",
        "Geschätzte zusätzliche Klicks / Monat",
        "Clics extra estimados / mes",
    ),
    (
        "opportunities_top",
        "Etkisine ve emeğine göre ilk işler",
        "First jobs by impact and effort",
        "Die ersten Aufgaben nach Wirkung und Aufwand",
        "Primeras tareas por impacto y esfuerzo",
    ),
    ("what", "Ne", "What", "Was", "Qué"),
    ("subject", "Konu", "About", "Worum", "Sobre"),
    (
        "gain",
        "Tık / ay",
        "Clicks / month",
        "Klicks / Monat",
        "Clics / mes",
    ),
    ("effort", "Emek", "Effort", "Aufwand", "Esfuerzo"),
    ("effort_low", "Az", "Low", "Gering", "Bajo"),
    ("effort_medium", "Orta", "Medium", "Mittel", "Medio"),
    ("effort_high", "Çok", "High", "Hoch", "Alto"),
    (
        "pages_count",
        "{n} sayfa",
        "{n} pages",
        "{n} Seiten",
        "{n} páginas",
    ),
    (
        "sites_count",
        "{n} site",
        "{n} sites",
        "{n} Websites",
        "{n} sitios",
    ),
    (
        "opp_near_top",
        "İlk sıralara yakın",
        "Close to the top",
        "Nah an der Spitze",
        "Cerca de lo más alto",
    ),
    (
        "opp_dropped",
        "Düşen kelime",
        "Keyword that fell",
        "Gefallenes Keyword",
        "Palabra clave que cayó",
    ),
    (
        "opp_low_ctr",
        "Düşük tıklama oranı",
        "Low click-through rate",
        "Niedrige Klickrate",
        "CTR baja",
    ),
    (
        "opp_losing_page",
        "Tıklama kaybeden sayfa",
        "Page losing clicks",
        "Seite verliert Klicks",
        "Página que pierde clics",
    ),
    (
        "opp_cannibal",
        "Yamyamlaşma",
        "Cannibalisation",
        "Kannibalisierung",
        "Canibalización",
    ),
    (
        "opp_audit_issue",
        "Denetim sorunu",
        "Audit issue",
        "Audit-Problem",
        "Problema de auditoría",
    ),
    (
        "opp_orphans",
        "Yetim sayfalar",
        "Orphan pages",
        "Verwaiste Seiten",
        "Páginas huérfanas",
    ),
    (
        "opp_weak_links",
        "Zayıf iç linkli sayfalar",
        "Weakly linked pages",
        "Schwach verlinkte Seiten",
        "Páginas con pocos enlaces internos",
    ),
    (
        "opp_deep_pages",
        "Çok derindeki sayfalar",
        "Pages buried deep",
        "Tief vergrabene Seiten",
        "Páginas demasiado profundas",
    ),
    (
        "opp_vitals",
        "Core Web Vitals",
        "Core Web Vitals",
        "Core Web Vitals",
        "Core Web Vitals",
    ),
    (
        "opp_slow_page",
        "Yavaş sayfa",
        "Slow page",
        "Langsame Seite",
        "Página lenta",
    ),
    (
        "opp_ai_gap",
        "AI rakibi anıyor, sizi değil",
        "AI names a rival, not you",
        "KI nennt einen Mitbewerber, nicht Sie",
        "La IA nombra a un rival, no a ti",
    ),
    (
        "opp_low_content",
        "İçerik puanı düşük",
        "Low content score",
        "Niedriger Content-Score",
        "Puntuación de contenido baja",
    ),
    (
        "opp_local_weak",
        "Haritada zayıf",
        "Weak on the map",
        "Schwach auf der Karte",
        "Débil en el mapa",
    ),
    (
        "opp_rival_growth",
        "Hızla büyüyen rakip",
        "Fast-growing competitor",
        "Schnell wachsender Mitbewerber",
        "Competidor en rápido crecimiento",
    ),
    (
        "opp_lost_links",
        "Geri kazanılacak linkler",
        "Links to win back",
        "Zurückzugewinnende Links",
        "Enlaces por recuperar",
    ),
    (
        "risky_links_kpi",
        "Riskli link veren siteler",
        "Risky linking sites",
        "Riskante verlinkende Websites",
        "Sitios de riesgo que enlazan",
    ),
    (
        "local_changes",
        "Haritada değişen kelimeler",
        "Map keywords that moved",
        "Bewegte Karten-Keywords",
        "Palabras clave del mapa que cambiaron",
    ),
    (
        "risky_new",
        "Yeni riskli link veren siteler",
        "New risky linking sites",
        "Neue riskante verlinkende Websites",
        "Nuevos sitios de riesgo que enlazan",
    ),
    (
        "ai_trend",
        "Çalıştırmalara göre AI görünürlüğü (kesikli: alıntılanma, ses payı)",
        "AI visibility run by run (dashed: citations, share of voice)",
        "KI-Sichtbarkeit je Durchlauf (gestrichelt: Zitate, Sichtbarkeitsanteil)",
        "Visibilidad en IA por ejecución (discontinuas: citas, cuota de voz)",
    ),
    (
        "top_cited_domains",
        "AI yanıtlarının en çok alıntıladığı alan adları",
        "Domains AI answers cite most",
        "Von KI-Antworten am häufigsten zitierte Domains",
        "Dominios que más citan las respuestas de IA",
    ),
    (
        "cited_answers",
        "Alıntılayan yanıt",
        "Answers citing it",
        "Zitierende Antworten",
        "Respuestas que lo citan",
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
