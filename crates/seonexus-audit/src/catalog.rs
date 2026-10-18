//! The issue catalog, loaded from `issues.json`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const CATALOG_JSON: &str = include_str!("../issues.json");

/// Every check and category.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    /// Categories in display order.
    pub categories: Vec<CategoryDef>,
    /// Checks in display order.
    pub issues: Vec<IssueDef>,
}

impl Catalog {
    /// The check `id`.
    pub fn issue(&self, id: &str) -> Option<&IssueDef> {
        self.issues.iter().find(|issue| issue.id == id)
    }

    /// The category `id`.
    pub fn category(&self, id: &str) -> Option<&CategoryDef> {
        self.categories.iter().find(|category| category.id == id)
    }
}

/// The catalog compiled into the binary.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    // The file is part of the source tree; a test proves it parses.
    CATALOG.get_or_init(|| serde_json::from_str(CATALOG_JSON).unwrap_or_default())
}

/// A group of checks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryDef {
    /// Stable id, e.g. `titles`.
    pub id: String,
    /// English name.
    pub en: String,
    /// Turkish name.
    pub tr: String,
}

impl CategoryDef {
    /// The name in `lang`.
    pub fn name(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Tr => &self.tr,
        }
    }
}

/// One check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDef {
    /// Stable id, e.g. `title-missing`.
    pub id: String,
    /// Category id.
    pub category: String,
    /// How serious a finding is.
    pub kind: IssueKind,
    /// How urgent fixing it usually is.
    pub priority: Priority,
    /// English texts.
    pub en: IssueText,
    /// Turkish texts.
    pub tr: IssueText,
}

impl IssueDef {
    /// The texts in `lang`.
    pub fn text(&self, lang: Lang) -> &IssueText {
        match lang {
            Lang::En => &self.en,
            Lang::Tr => &self.tr,
        }
    }
}

/// Human-readable texts of a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueText {
    /// Short name.
    pub title: String,
    /// What was found and why it matters.
    pub description: String,
    /// How to fix it.
    pub fix: String,
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueKind {
    /// Something is wrong and should be fixed.
    Issue,
    /// Probably wrong; check it.
    Warning,
    /// Could be improved.
    Opportunity,
}

impl IssueKind {
    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Warning => "warning",
            Self::Opportunity => "opportunity",
        }
    }
}

/// How urgent fixing a finding usually is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Fix first.
    High,
    /// Fix soon.
    Medium,
    /// Fix when convenient.
    Low,
}

impl Priority {
    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// Languages of the catalog texts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// English.
    #[default]
    En,
    /// Turkish.
    Tr,
}

impl Lang {
    /// The language of a code such as `tr` or `tr-TR`; English otherwise.
    pub fn from_code(code: &str) -> Self {
        if code.trim().to_ascii_lowercase().starts_with("tr") {
            Self::Tr
        } else {
            Self::En
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete() {
        let catalog = catalog();
        assert!(
            catalog.issues.len() >= 100,
            "{} checks",
            catalog.issues.len()
        );
        for issue in &catalog.issues {
            assert!(catalog.category(&issue.category).is_some(), "{}", issue.id);
            for text in [&issue.en, &issue.tr] {
                assert!(
                    !text.title.is_empty() && !text.description.is_empty() && !text.fix.is_empty()
                );
            }
        }
        let title = catalog.issue("title-missing").unwrap();
        assert_eq!(
            (title.kind, title.priority),
            (IssueKind::Issue, Priority::High)
        );
        assert_eq!(
            title.text(Lang::from_code("tr-TR")).title,
            "Başlık (title) yok"
        );
    }
}
