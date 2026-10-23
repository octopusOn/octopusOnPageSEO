//! A comparison told in words, in Turkish, English, German or Spanish: the good news first, then
//! what needs attention. Sentences are chosen by rules from the numbers, so the same two
//! snapshots always read the same.

use serde::Serialize;

use super::compare::{Comparison, Kpi};
use crate::report::texts::{number, percent};

/// Sentences at most.
const SENTENCES: usize = 8;

/// A change smaller than this, in percent, is not told.
const NOTABLE_PERCENT: f64 = 1.0;

/// `(key, tr, en, de, es)`.
const TEXTS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "traffic_up",
        "Organik trafik {pct} arttı ({before} → {after}).",
        "Organic traffic rose {pct} ({before} → {after}).",
        "Der organische Traffic stieg um {pct} ({before} → {after}).",
        "El tráfico orgánico subió un {pct} ({before} → {after}).",
    ),
    (
        "traffic_down",
        "Organik trafik {pct} azaldı ({before} → {after}).",
        "Organic traffic fell {pct} ({before} → {after}).",
        "Der organische Traffic sank um {pct} ({before} → {after}).",
        "El tráfico orgánico bajó un {pct} ({before} → {after}).",
    ),
    (
        "entered_top10",
        "{n} kelime ilk 10'a girdi.",
        "{n} keywords entered the top 10.",
        "{n} Keywords kamen in die Top 10.",
        "{n} palabras clave entraron en el top 10.",
    ),
    (
        "left_top10",
        "{n} kelime ilk 10'dan çıktı.",
        "{n} keywords dropped out of the top 10.",
        "{n} Keywords fielen aus den Top 10.",
        "{n} palabras clave salieron del top 10.",
    ),
    (
        "keywords_won",
        "{n} yeni kelimede sıralama alındı.",
        "The site now ranks for {n} new keywords.",
        "Die Website rankt jetzt für {n} neue Keywords.",
        "El sitio posiciona ahora {n} palabras clave nuevas.",
    ),
    (
        "keywords_lost",
        "{n} kelimede sıralama kaybedildi.",
        "{n} keywords no longer rank.",
        "{n} Keywords ranken nicht mehr.",
        "{n} palabras clave dejaron de posicionar.",
    ),
    (
        "domains_won",
        "{n} yeni alan adı link verdi.",
        "{n} new domains link to the site.",
        "{n} neue Domains verlinken die Website.",
        "{n} dominios nuevos enlazan al sitio.",
    ),
    (
        "domains_lost",
        "{n} alan adının linki artık yok.",
        "{n} domains no longer link to the site.",
        "{n} Domains verlinken die Website nicht mehr.",
        "{n} dominios ya no enlazan al sitio.",
    ),
    (
        "oa_up",
        "OA {before} puandan {after} puana yükseldi.",
        "OA rose from {before} to {after}.",
        "Die OA stieg von {before} auf {after}.",
        "La OA subió de {before} a {after}.",
    ),
    (
        "oa_down",
        "OA {before} puandan {after} puana düştü.",
        "OA fell from {before} to {after}.",
        "Die OA sank von {before} auf {after}.",
        "La OA bajó de {before} a {after}.",
    ),
    (
        "health_up",
        "Site sağlığı {before} puandan {after} puana çıktı.",
        "Site health rose from {before} to {after}.",
        "Die Website-Gesundheit stieg von {before} auf {after}.",
        "La salud del sitio subió de {before} a {after}.",
    ),
    (
        "health_down",
        "Site sağlığı {before} puandan {after} puana düştü.",
        "Site health fell from {before} to {after}.",
        "Die Website-Gesundheit sank von {before} auf {after}.",
        "La salud del sitio bajó de {before} a {after}.",
    ),
    (
        "issues_fixed",
        "{n} sorun türü tamamen düzeldi.",
        "{n} kinds of issues were fixed.",
        "{n} Fehlerarten wurden behoben.",
        "Se corrigieron {n} tipos de problemas.",
    ),
    (
        "issues_new",
        "{n} yeni sorun türü çıktı.",
        "{n} new kinds of issues appeared.",
        "{n} neue Fehlerarten sind aufgetreten.",
        "Aparecieron {n} tipos de problemas nuevos.",
    ),
    (
        "ai_up",
        "AI yanıtlarında bahsedilme oranı {before} → {after} yükseldi.",
        "Mentions in AI answers rose from {before} to {after}.",
        "Erwähnungen in KI-Antworten stiegen von {before} auf {after}.",
        "Las menciones en respuestas de IA subieron de {before} a {after}.",
    ),
    (
        "ai_down",
        "AI yanıtlarında bahsedilme oranı {before} → {after} düştü.",
        "Mentions in AI answers fell from {before} to {after}.",
        "Erwähnungen in KI-Antworten sanken von {before} auf {after}.",
        "Las menciones en respuestas de IA bajaron de {before} a {after}.",
    ),
    (
        "ai_gained",
        "{n} soruda AI asistanları markayı anmaya başladı.",
        "AI assistants started naming the brand in {n} answers.",
        "KI-Assistenten nennen die Marke jetzt in {n} Antworten.",
        "Los asistentes de IA empezaron a nombrar la marca en {n} respuestas.",
    ),
    (
        "ai_lost",
        "{n} soruda marka artık anılmıyor.",
        "The brand is no longer named in {n} answers.",
        "Die Marke wird in {n} Antworten nicht mehr genannt.",
        "La marca ya no se nombra en {n} respuestas.",
    ),
    (
        "clicks_up",
        "Search Console tıklamaları {pct} arttı ({before} → {after}).",
        "Search Console clicks rose {pct} ({before} → {after}).",
        "Die Search-Console-Klicks stiegen um {pct} ({before} → {after}).",
        "Los clics de Search Console subieron un {pct} ({before} → {after}).",
    ),
    (
        "clicks_down",
        "Search Console tıklamaları {pct} azaldı ({before} → {after}).",
        "Search Console clicks fell {pct} ({before} → {after}).",
        "Die Search-Console-Klicks sanken um {pct} ({before} → {after}).",
        "Los clics de Search Console bajaron un {pct} ({before} → {after}).",
    ),
    (
        "visibility_up",
        "Takip edilen kelimelerde görünürlük {before} → {after} yükseldi.",
        "Visibility of tracked keywords rose from {before} to {after}.",
        "Die Sichtbarkeit der getrackten Keywords stieg von {before} auf {after}.",
        "La visibilidad de las palabras seguidas subió de {before} a {after}.",
    ),
    (
        "visibility_down",
        "Takip edilen kelimelerde görünürlük {before} → {after} düştü.",
        "Visibility of tracked keywords fell from {before} to {after}.",
        "Die Sichtbarkeit der getrackten Keywords sank von {before} auf {after}.",
        "La visibilidad de las palabras seguidas bajó de {before} a {after}.",
    ),
    (
        "competitor_up",
        "Rakip {domain} trafiğini {pct} artırdı.",
        "Competitor {domain} grew its traffic by {pct}.",
        "Der Mitbewerber {domain} steigerte seinen Traffic um {pct}.",
        "El competidor {domain} aumentó su tráfico un {pct}.",
    ),
    (
        "local_up",
        "Haritada yerel ses payı {before} → {after} yükseldi.",
        "Share of local voice on the map rose from {before} to {after}.",
        "Der lokale Sichtbarkeitsanteil auf der Karte stieg von {before} auf {after}.",
        "La cuota de voz local en el mapa subió de {before} a {after}.",
    ),
    (
        "local_down",
        "Haritada yerel ses payı {before} → {after} düştü.",
        "Share of local voice on the map fell from {before} to {after}.",
        "Der lokale Sichtbarkeitsanteil auf der Karte sank von {before} auf {after}.",
        "La cuota de voz local en el mapa bajó de {before} a {after}.",
    ),
    (
        "risky_new",
        "{n} yeni riskli site link veriyor; inceleyip gerekirse reddedin.",
        "{n} new risky sites link to the site; review them and disavow where needed.",
        "{n} neue riskante Websites verlinken die Website; prüfen und bei Bedarf entwerten.",
        "{n} sitios de riesgo nuevos enlazan al sitio; revísalos y desautoriza si hace falta.",
    ),
    (
        "risky_cleared",
        "{n} riskli site temizlendi ya da reddedildi.",
        "{n} risky linking sites were cleared or disavowed.",
        "{n} riskante verlinkende Websites wurden bereinigt oder entwertet.",
        "Se limpiaron o desautorizaron {n} sitios de riesgo.",
    ),
    (
        "nothing",
        "İki anlık görüntü arasında kayda değer bir değişiklik yok.",
        "Nothing notable changed between the two snapshots.",
        "Zwischen den beiden Momentaufnahmen hat sich nichts Nennenswertes geändert.",
        "No hubo cambios notables entre las dos instantáneas.",
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

fn fill(lang: &str, key: &str, values: &[(&str, String)]) -> String {
    let text = TEXTS
        .iter()
        .find(|(name, ..)| *name == key)
        .map_or(key, |&(_, tr, en, de, es)| [tr, en, de, es][column(lang)]);
    values.iter().fold(text.to_owned(), |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

/// A sentence of the summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sentence {
    /// `good` or `bad`.
    pub tone: &'static str,
    /// The sentence.
    pub text: String,
}

/// A candidate with how much it matters.
struct Candidate {
    good: bool,
    weight: f64,
    text: String,
}

fn kpi<'a>(comparison: &'a Comparison, key: &str) -> Option<&'a Kpi> {
    comparison.kpis.iter().find(|kpi| kpi.key == key)
}

/// The summary of a comparison in `lang`.
#[allow(clippy::too_many_lines)] // one rule per kind of news
pub fn summarize(comparison: &Comparison, lang: &str) -> Vec<Sentence> {
    let mut candidates: Vec<Candidate> = Vec::new();
    let whole = |value: f64| number(value, 0, lang);
    let mut add = |good: bool, weight: f64, key: &str, values: &[(&str, String)]| {
        candidates.push(Candidate {
            good,
            weight,
            text: fill(lang, key, values),
        });
    };
    // Changes of a number told with their percentage.
    for (key, up, down, weight) in [
        ("traffic", "traffic_up", "traffic_down", 100.0),
        ("clicks", "clicks_up", "clicks_down", 60.0),
    ] {
        if let Some(found) = kpi(comparison, key)
            && let (Some(before), Some(after), Some(pct)) =
                (found.before, found.after, found.percent)
            && pct.abs() >= NOTABLE_PERCENT
        {
            add(
                pct > 0.0,
                weight + pct.abs().min(100.0),
                if pct > 0.0 { up } else { down },
                &[
                    ("pct", percent(pct.abs(), lang)),
                    ("before", whole(before)),
                    ("after", whole(after)),
                ],
            );
        }
    }
    // Scores told before and after.
    for (key, up, down, weight, as_percent) in [
        ("oa", "oa_up", "oa_down", 50.0, false),
        ("health", "health_up", "health_down", 45.0, false),
        ("mention_rate", "ai_up", "ai_down", 55.0, true),
        ("visibility", "visibility_up", "visibility_down", 40.0, true),
        ("local_solv", "local_up", "local_down", 40.0, true),
    ] {
        if let Some(found) = kpi(comparison, key)
            && let (Some(before), Some(after)) = (found.before, found.after)
            && (after - before).abs() >= 0.5
        {
            let show = |value: f64| {
                if as_percent {
                    percent(value, lang)
                } else {
                    whole(value)
                }
            };
            add(
                after > before,
                weight + (after - before).abs(),
                if after > before { up } else { down },
                &[("before", show(before)), ("after", show(after))],
            );
        }
    }
    // Counts of what moved.
    let mut count = |n: usize, good: bool, weight: f64, key: &str| {
        if n > 0 {
            #[allow(clippy::cast_precision_loss)]
            let n_f = n as f64;
            add(good, weight + n_f.min(100.0), key, &[("n", whole(n_f))]);
        }
    };
    if let Some(keywords) = &comparison.keywords {
        count(keywords.entered_top10, true, 70.0, "entered_top10");
        count(keywords.gained, true, 30.0, "keywords_won");
        count(keywords.left_top10, false, 70.0, "left_top10");
        count(keywords.lost, false, 30.0, "keywords_lost");
    }
    if let Some(domains) = &comparison.domains {
        count(domains.gained, true, 35.0, "domains_won");
        count(domains.lost, false, 35.0, "domains_lost");
    }
    if let Some(audit) = &comparison.audit {
        count(audit.fixed.len(), true, 25.0, "issues_fixed");
        count(audit.new_issues.len(), false, 25.0, "issues_new");
    }
    if let Some(ai) = &comparison.ai {
        count(ai.gained.len(), true, 30.0, "ai_gained");
        count(ai.lost.len(), false, 30.0, "ai_lost");
    }
    if let Some(links) = &comparison.link_audit {
        count(links.cleared.len(), true, 20.0, "risky_cleared");
        count(links.new_risky.len(), false, 40.0, "risky_new");
    }
    // The competitor that grew most, as something to watch.
    if let Some((domain, pct)) = comparison
        .sites
        .iter()
        .filter(|site| !site.own)
        .filter_map(|site| {
            let traffic = site.kpis.iter().find(|kpi| kpi.key == "traffic")?;
            Some((site.domain.clone(), traffic.percent?))
        })
        .filter(|(_, pct)| *pct >= 5.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
    {
        add(
            false,
            20.0 + pct.min(100.0),
            "competitor_up",
            &[("domain", domain), ("pct", percent(pct, lang))],
        );
    }
    if candidates.is_empty() {
        return vec![Sentence {
            tone: "note",
            text: fill(lang, "nothing", &[]),
        }];
    }
    candidates.sort_by(|a, b| b.good.cmp(&a.good).then(b.weight.total_cmp(&a.weight)));
    let good: Vec<&Candidate> = candidates.iter().filter(|c| c.good).collect();
    let bad: Vec<&Candidate> = candidates.iter().filter(|c| !c.good).collect();
    // Good news first, but always room for what needs attention.
    let bad_room = bad.len().min(3);
    good.iter()
        .take(SENTENCES - bad_room)
        .chain(bad.iter().take(SENTENCES))
        .take(SENTENCES)
        .map(|c| Sentence {
            tone: if c.good { "good" } else { "bad" },
            text: c.text.clone(),
        })
        .collect()
}
