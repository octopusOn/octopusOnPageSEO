//! Adding keywords by hand or from sources, with their identities, stems and intents.

use octopuson_keywords::normalize;
use octopuson_kw_store::{
    Added, IntentRecord, KeywordPage, KeywordQuery, KeywordStatus, KwStore, NewKeyword,
};
use octopuson_serp_score::{IntentClass, keyword_intent};
use octopuson_text::Language;
use serde::{Deserialize, Serialize};

use crate::{KeywordWorkspace, KwAppError, now};

/// Most keywords one addition takes.
pub const MAX_KEYWORDS: usize = 200_000;

/// What adding keywords did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddOutcome {
    /// New, known and given keywords.
    #[serde(flatten)]
    pub added: Added,
    /// Lines that are not keywords (empty, only punctuation, too long).
    pub rejected: u64,
}

/// Stored name of an intent.
pub const fn intent_name(intent: IntentClass) -> &'static str {
    match intent {
        IntentClass::Informational => "informational",
        IntentClass::Navigational => "navigational",
        IntentClass::Commercial => "commercial",
        IntentClass::Transactional => "transactional",
        IntentClass::Local => "local",
    }
}

/// The intent of a stored name.
pub fn parse_intent(name: &str) -> Option<IntentClass> {
    Some(match name {
        "informational" => IntentClass::Informational,
        "navigational" => IntentClass::Navigational,
        "commercial" => IntentClass::Commercial,
        "transactional" => IntentClass::Transactional,
        "local" => IntentClass::Local,
        _ => return None,
    })
}

/// Keywords ready for the store, and how many texts were not keywords.
pub(crate) fn prepare(
    texts: impl IntoIterator<Item = (String, Option<String>)>,
    language: Language,
    source: &str,
) -> (Vec<NewKeyword>, u64) {
    let mut keywords = Vec::new();
    let mut rejected = 0;
    for (text, seed) in texts {
        match normalize(&text, language) {
            Some(normalized) => keywords.push(NewKeyword {
                keyword: normalized.text,
                key: normalized.key,
                stems: normalized.stems,
                source: source.to_owned(),
                seed,
            }),
            None => rejected += 1,
        }
    }
    (keywords, rejected)
}

/// Gives keywords without an intent the intent their words state.
pub(crate) fn fill_intents(
    store: &mut KwStore,
    project_id: i64,
    language: &str,
) -> Result<(), KwAppError> {
    let records: Vec<IntentRecord> = store
        .keywords_without_intent(project_id)?
        .into_iter()
        .filter_map(|(keyword_id, keyword)| {
            keyword_intent(&keyword, language).map(|intent| IntentRecord {
                keyword_id,
                primary: intent_name(intent.primary).to_owned(),
                secondary: intent
                    .secondary
                    .map(|secondary| intent_name(secondary).to_owned()),
                source: "words".to_owned(),
            })
        })
        .collect();
    if !records.is_empty() {
        store.put_intents(&records)?;
    }
    Ok(())
}

impl KeywordWorkspace {
    /// Adds keywords typed or pasted by the user (one per entry) to a project.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for too many keywords; store errors.
    pub fn add_keywords(
        &self,
        project_id: i64,
        texts: &[String],
        source: &str,
        seed: Option<&str>,
    ) -> Result<AddOutcome, KwAppError> {
        if texts.len() > MAX_KEYWORDS {
            return Err(KwAppError::Invalid(format!(
                "add at most {MAX_KEYWORDS} keywords at once"
            )));
        }
        let mut store = self.store()?;
        let project = store.project(project_id)?;
        let language = Language::from_code(&project.spec.language);
        let (keywords, rejected) = prepare(
            texts
                .iter()
                .map(|text| (text.clone(), seed.map(str::to_owned))),
            language,
            source,
        );
        let added = store.add_keywords(project_id, &keywords, &now())?;
        fill_intents(&mut store, project_id, &project.spec.language)?;
        Ok(AddOutcome { added, rejected })
    }

    /// A page of a project's keywords.
    ///
    /// The search text is folded like keyword identities, so `turk` finds `Türk`.
    ///
    /// # Errors
    /// Store errors.
    pub fn keywords(
        &self,
        project_id: i64,
        query: &KeywordQuery,
    ) -> Result<KeywordPage, KwAppError> {
        let store = self.store()?;
        let project = store.project(project_id)?;
        let mut query = query.clone();
        query.search = query
            .search
            .as_deref()
            .and_then(|text| normalize(text, Language::from_code(&project.spec.language)))
            .map(|normalized| normalized.key);
        Ok(store.keywords(project_id, &query)?)
    }

    /// Leaves keywords out of clustering and exports, or takes them back.
    ///
    /// # Errors
    /// Store errors.
    pub fn set_status(
        &self,
        project_id: i64,
        ids: &[i64],
        status: KeywordStatus,
    ) -> Result<u64, KwAppError> {
        Ok(self.store()?.set_status(project_id, ids, status)?)
    }

    /// Adds and removes tags.
    ///
    /// # Errors
    /// Store errors.
    pub fn tag(
        &self,
        project_id: i64,
        ids: &[i64],
        add: &[String],
        remove: &[String],
    ) -> Result<(), KwAppError> {
        Ok(self.store()?.tag(project_id, ids, add, remove)?)
    }
}
