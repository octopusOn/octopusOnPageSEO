//! What searchers of a keyword want: to learn, to compare, to buy, to go somewhere or to find
//! a place nearby.

use std::collections::BTreeMap;

use seonexus_serp::{FeatureKind, SerpSnapshot};
use seonexus_text::{Language, fold, lower, tokens};
use serde::{Deserialize, Serialize};

use crate::breakdown::round;
use crate::signals::PageSignals;

/// Version of the intent model.
pub const INTENT_MODEL: &str = "intent-1";

/// A search intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentClass {
    /// Wants to know something.
    Informational,
    /// Wants a particular site or page.
    Navigational,
    /// Compares before buying.
    Commercial,
    /// Wants to buy or act.
    Transactional,
    /// Wants something nearby.
    Local,
}

/// The intent of a keyword with its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    /// Most likely intent.
    pub primary: IntentClass,
    /// Second intent, when it has at least a quarter of the weight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<IntentClass>,
    /// Share of the evidence for each intent.
    pub scores: BTreeMap<IntentClass, f64>,
    /// i18n keys of the evidence (`intent.modifier.fiyat`, `intent.feature.local-pack`).
    pub signals: Vec<String>,
    /// Model version.
    pub model: String,
}

const INFORMATIONAL: &[&str] = &[
    "nedir",
    "nasil",
    "ne demek",
    "neden",
    "nicin",
    "niye",
    "rehber",
    "rehberi",
    "tarif",
    "tarifi",
    "anlami",
    "ornek",
    "ornekleri",
    "ozellikleri",
    "kimdir",
    "hakkinda",
    "ne zaman",
    "kac",
    "nerede yetisir",
    "faydalari",
    "zararlari",
    "belirtileri",
    "what",
    "how",
    "why",
    "when",
    "who",
    "guide",
    "tutorial",
    "meaning",
    "definition",
    "examples",
    "ideas",
    "tips",
    "benefits",
];
const COMMERCIAL: &[&str] = &[
    "en iyi",
    "karsilastirma",
    "yorum",
    "yorumlari",
    "inceleme",
    "alternatif",
    "alternatifleri",
    "tavsiye",
    "onerileri",
    "hangisi",
    "vs",
    "mi yoksa",
    "best",
    "top",
    "review",
    "reviews",
    "versus",
    "compare",
    "comparison",
    "alternative",
    "alternatives",
];
const TRANSACTIONAL: &[&str] = &[
    "fiyat",
    "fiyati",
    "fiyatlari",
    "satin al",
    "satin",
    "siparis",
    "ucuz",
    "indirim",
    "kampanya",
    "kupon",
    "satilik",
    "kiralik",
    "magaza",
    "sepet",
    "kargo",
    "taksit",
    "ucret",
    "ucreti",
    "ucretleri",
    "teklif",
    "indir",
    "buy",
    "price",
    "prices",
    "cheap",
    "deal",
    "deals",
    "discount",
    "coupon",
    "order",
    "shop",
    "sale",
    "cost",
    "download",
];
const LOCAL: &[&str] = &[
    "yakinimdaki",
    "yakinimda",
    "en yakin",
    "nerede",
    "adres",
    "adresi",
    "near me",
    "nearby",
    "open now",
    "directions",
];
const NAVIGATIONAL: &[&str] = &[
    "giris",
    "giris yap",
    "login",
    "resmi site",
    "resmi",
    "official",
    "sign in",
    "iletisim",
    "musteri hizmetleri",
    "uygulama",
    "app",
];

/// Turkish provinces and the largest districts, folded.
const PLACES: &str = "adana adiyaman afyonkarahisar afyon agri amasya ankara antalya artvin aydin balikesir bilecik bingol bitlis bolu burdur bursa canakkale cankiri corum denizli diyarbakir edirne elazig erzincan erzurum eskisehir gaziantep antep giresun gumushane hakkari hatay isparta mersin istanbul izmir kars kastamonu kayseri kirklareli kirsehir kocaeli izmit konya kutahya malatya manisa kahramanmaras maras mardin mugla bodrum fethiye marmaris mus nevsehir kapadokya nigde ordu rize sakarya adapazari samsun siirt sinop sivas tekirdag tokat trabzon tunceli sanliurfa urfa usak van yozgat zonguldak aksaray bayburt karaman kirikkale batman sirnak bartin ardahan igdir yalova karabuk kilis osmaniye duzce kadikoy besiktas sisli uskudar bakirkoy beylikduzu esenyurt atasehir maltepe kartal pendik umraniye beyoglu fatih bagcilar sariyer avcilar cankaya kecioren yenimahalle etimesgut karsiyaka bornova konak buca nilufer osmangazi muratpasa konyaalti alanya seyhan selcuklu";

fn has_phrase(words: &[String], phrase: &str) -> bool {
    let wanted: Vec<&str> = phrase.split(' ').collect();
    words.windows(wanted.len()).any(|window| {
        window
            .iter()
            .zip(&wanted)
            .all(|(word, wanted)| word == wanted)
    })
}

/// Evidence collected for each intent.
#[derive(Default)]
struct Evidence {
    scores: BTreeMap<IntentClass, f64>,
    signals: Vec<String>,
}

impl Evidence {
    fn add(&mut self, class: IntentClass, weight: f64, signal: String) {
        *self.scores.entry(class).or_default() += weight;
        self.signals.push(signal);
    }

    /// Modifiers, place names and, with a result page, brand names in the keyword.
    fn keyword(&mut self, snapshot: Option<&SerpSnapshot>, words: &[String]) {
        for (class, list) in [
            (IntentClass::Informational, INFORMATIONAL),
            (IntentClass::Commercial, COMMERCIAL),
            (IntentClass::Transactional, TRANSACTIONAL),
            (IntentClass::Local, LOCAL),
            (IntentClass::Navigational, NAVIGATIONAL),
        ] {
            for modifier in list.iter().filter(|modifier| has_phrase(words, modifier)) {
                self.add(class, 1.0, format!("intent.modifier:{modifier}"));
            }
        }
        if let Some(place) = PLACES
            .split(' ')
            .find(|place| words.iter().any(|word| word == place))
        {
            self.add(IntentClass::Local, 0.8, format!("intent.place:{place}"));
        }
        // The keyword is the name of a ranking site ("trendyol", "sahibinden giriş").
        if let Some(first) = snapshot.and_then(|snapshot| snapshot.organic.first()) {
            let label = first.domain.split('.').next().unwrap_or_default();
            if label.len() > 2 && words.iter().any(|word| word.replace('-', "") == label) {
                self.add(
                    IntentClass::Navigational,
                    1.5,
                    format!("intent.brand:{}", first.domain),
                );
            }
        }
    }

    /// Blocks of the first result page.
    fn features(&mut self, snapshot: &SerpSnapshot) {
        let first_page =
            |kind: FeatureKind| snapshot.features_of(kind).any(|feature| feature.page == 1);
        if first_page(FeatureKind::Shopping) {
            self.add(
                IntentClass::Transactional,
                0.8,
                "intent.feature:shopping".to_owned(),
            );
        }
        if snapshot.top_ad_count() >= 3 {
            self.add(
                IntentClass::Transactional,
                0.6,
                "intent.feature:ads".to_owned(),
            );
        }
        if first_page(FeatureKind::LocalPack) {
            self.add(
                IntentClass::Local,
                1.0,
                "intent.feature:local-pack".to_owned(),
            );
        }
        for (kind, weight) in [
            (FeatureKind::FeaturedSnippet, 0.4),
            (FeatureKind::PeopleAlsoAsk, 0.4),
            (FeatureKind::AiOverview, 0.4),
            (FeatureKind::Videos, 0.3),
        ] {
            if first_page(kind) {
                self.add(
                    IntentClass::Informational,
                    weight,
                    format!("intent.feature:{}", kind.as_str()),
                );
            }
        }
        if first_page(FeatureKind::KnowledgePanel)
            && snapshot
                .organic
                .first()
                .is_some_and(|result| !result.sitelinks.is_empty())
        {
            self.add(
                IntentClass::Navigational,
                0.8,
                "intent.feature:sitelinks".to_owned(),
            );
        }
    }

    /// Structured data the ranking pages use.
    #[allow(clippy::cast_precision_loss)] // page counts
    fn schema(&mut self, pages: &[PageSignals]) {
        let live: Vec<&PageSignals> = pages.iter().filter(|page| page.is_live()).collect();
        if live.is_empty() {
            return;
        }
        let share = |types: &[&str]| {
            live.iter()
                .filter(|page| {
                    page.schema_types
                        .iter()
                        .any(|own| types.iter().any(|schema| own.eq_ignore_ascii_case(schema)))
                })
                .count() as f64
                / live.len() as f64
        };
        for (class, types, weight) in [
            (
                IntentClass::Transactional,
                &["Product", "Offer", "AggregateOffer"][..],
                0.8,
            ),
            (
                IntentClass::Informational,
                &["Article", "BlogPosting", "NewsArticle", "HowTo", "FAQPage"][..],
                0.6,
            ),
            (
                IntentClass::Local,
                &[
                    "LocalBusiness",
                    "Restaurant",
                    "Store",
                    "Hotel",
                    "MedicalBusiness",
                ][..],
                0.8,
            ),
            (
                IntentClass::Commercial,
                &["Review", "AggregateRating", "ItemList"][..],
                0.5,
            ),
        ] {
            let share = share(types);
            if share >= 0.2 {
                self.add(class, weight * share, format!("intent.schema:{}", types[0]));
            }
        }
    }
}

/// Folded words of a keyword, as the modifier lists are written.
fn folded_words(keyword: &str, language: Language) -> Vec<String> {
    tokens(keyword, language)
        .iter()
        .map(|word| fold(&lower(word, language)))
        .collect()
}

/// The intent of a snapshot's keyword.
pub fn intent(snapshot: &SerpSnapshot, pages: &[PageSignals]) -> Intent {
    let language = Language::from_code(&snapshot.query.language);
    let words = folded_words(&snapshot.query.keyword, language);
    let mut evidence = Evidence::default();
    evidence.keyword(Some(snapshot), &words);
    evidence.features(snapshot);
    evidence.schema(pages);
    decide(evidence)
}

/// The intent a keyword states by itself (modifiers such as `nasıl` or `fiyat`, place names),
/// for keyword lists without result pages; `None` when it states none.
pub fn keyword_intent(keyword: &str, language: &str) -> Option<Intent> {
    let language = Language::from_code(language);
    let mut evidence = Evidence::default();
    evidence.keyword(None, &folded_words(keyword, language));
    (!evidence.scores.is_empty()).then(|| decide(evidence))
}

fn decide(evidence: Evidence) -> Intent {
    let Evidence {
        mut scores,
        signals,
    } = evidence;
    let total: f64 = scores.values().sum();
    if total <= 0.0 {
        scores.insert(IntentClass::Informational, 1.0);
    } else {
        for value in scores.values_mut() {
            *value = round(*value / total);
        }
    }
    let mut ranked: Vec<(IntentClass, f64)> = scores
        .iter()
        .map(|(class, value)| (*class, *value))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Intent {
        primary: ranked[0].0,
        secondary: ranked
            .get(1)
            .filter(|(_, value)| *value >= 0.25)
            .map(|(class, _)| *class),
        scores,
        signals,
        model: INTENT_MODEL.to_owned(),
    }
}
