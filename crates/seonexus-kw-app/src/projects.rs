//! Projects: checked and normalised before they are stored.

use seonexus_keywords::site_domain;
use seonexus_kw_store::{ProjectRow, ProjectSpec};

use crate::{KeywordWorkspace, KwAppError, now};

/// Engines a project can target.
pub const ENGINES: [&str; 3] = ["google", "yandex", "bing"];
/// Longest project name, in characters.
const MAX_NAME: usize = 100;

/// Checks a market and returns its engines without repeats, in order.
pub(crate) fn check_market(
    country: &str,
    language: &str,
    engines: &[String],
) -> Result<Vec<String>, String> {
    let code = |text: &str| text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_lowercase());
    if !code(country) {
        return Err(format!(
            "`{country}` is not a two-letter country code (tr, de, us)"
        ));
    }
    if !code(language) {
        return Err(format!(
            "`{language}` is not a two-letter language code (tr, en, de)"
        ));
    }
    let mut unique: Vec<String> = Vec::new();
    for engine in engines {
        let engine = engine.trim().to_ascii_lowercase();
        if !ENGINES.contains(&engine.as_str()) {
            return Err(format!(
                "unknown engine `{engine}` (google, yandex or bing)"
            ));
        }
        if !unique.contains(&engine) {
            unique.push(engine);
        }
    }
    if unique.is_empty() {
        return Err("choose at least one engine".to_owned());
    }
    Ok(unique)
}

/// A project as entered, checked and normalised: trimmed name, lower-case codes, engines
/// without repeats, the site and competitors as registrable domains.
///
/// # Errors
/// [`KwAppError::Invalid`] with what is wrong.
pub fn normalize_project(spec: &ProjectSpec) -> Result<ProjectSpec, KwAppError> {
    let name = spec.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(KwAppError::Invalid(format!(
            "a project needs a name of 1–{MAX_NAME} characters"
        )));
    }
    let country = spec.country.trim().to_ascii_lowercase();
    let language = spec.language.trim().to_ascii_lowercase();
    let engines = check_market(&country, &language, &spec.engines).map_err(KwAppError::Invalid)?;
    let domain = |text: &str| {
        site_domain(text.trim())
            .ok_or_else(|| KwAppError::Invalid(format!("`{}` is not a site", text.trim())))
    };
    let site = spec
        .site
        .as_deref()
        .filter(|site| !site.trim().is_empty())
        .map(domain)
        .transpose()?;
    let mut competitors: Vec<String> = Vec::new();
    for competitor in spec
        .competitors
        .iter()
        .filter(|text| !text.trim().is_empty())
    {
        let competitor = domain(competitor)?;
        if Some(&competitor) != site.as_ref() && !competitors.contains(&competitor) {
            competitors.push(competitor);
        }
    }
    Ok(ProjectSpec {
        name: name.to_owned(),
        country,
        language,
        engines,
        site,
        competitors,
    })
}

impl KeywordWorkspace {
    /// Creates a project.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an invalid spec; store errors.
    pub fn create_project(&self, spec: &ProjectSpec) -> Result<ProjectRow, KwAppError> {
        let spec = normalize_project(spec)?;
        let store = self.store()?;
        let id = store.create_project(&spec, &now())?;
        Ok(store.project(id)?)
    }

    /// Changes a project.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an invalid spec; store errors.
    pub fn update_project(&self, id: i64, spec: &ProjectSpec) -> Result<ProjectRow, KwAppError> {
        let spec = normalize_project(spec)?;
        let store = self.store()?;
        store.update_project(id, &spec)?;
        Ok(store.project(id)?)
    }

    /// Every project, newest first.
    ///
    /// # Errors
    /// Store errors.
    pub fn projects(&self) -> Result<Vec<ProjectRow>, KwAppError> {
        Ok(self.store()?.projects()?)
    }

    /// One project.
    ///
    /// # Errors
    /// Store errors, [`seonexus_kw_store::KwStoreError::NotFound`].
    pub fn project(&self, id: i64) -> Result<ProjectRow, KwAppError> {
        Ok(self.store()?.project(id)?)
    }

    /// Deletes a project with its keywords and clusterings.
    ///
    /// # Errors
    /// Store errors.
    pub fn delete_project(&self, id: i64) -> Result<(), KwAppError> {
        Ok(self.store()?.delete_project(id)?)
    }
}
