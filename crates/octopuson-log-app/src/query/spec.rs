use octopuson_query::template::{TEMPLATES, find};
use octopuson_query::{Expr, Query, parse};
use serde::{Deserialize, Serialize};

use super::QueryError;

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuerySpec {
    /// Query-language text, e.g. `status:404 group:url`.
    Text {
        /// The query.
        text: String,
    },
    /// A built-in template, optionally refined by query text.
    ///
    /// The refinement's filter is combined with the template's using AND; its `group`,
    /// `metric`, `sort` and `limit` replace the template's when given.
    Template {
        /// Template id, e.g. `top-urls`.
        id: String,
        /// Extra query text.
        #[serde(default)]
        refine: Option<String>,
    },
    /// A saved query (see [`crate::saved`]), optionally refined like a template.
    Saved {
        /// Saved query name.
        name: String,
        /// Extra query text.
        #[serde(default)]
        refine: Option<String>,
    },
    /// A syntax tree, e.g. from the desktop's visual filter builder.
    Ast {
        /// The query.
        query: Query,
    },
}

impl QuerySpec {
    /// Query text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Resolves the spec into a syntax tree (not yet validated). Saved queries are resolved
    /// by the use-cases that have a workspace.
    ///
    /// # Errors
    /// [`QueryError::Parse`], [`QueryError::UnknownTemplate`] or
    /// [`QueryError::SavedQueryUnavailable`].
    pub fn resolve(&self) -> Result<Query, QueryError> {
        match self {
            Self::Text { text } => parse_text(text),
            Self::Template { id, refine } => {
                let template = find(id).ok_or_else(|| QueryError::UnknownTemplate {
                    id: id.clone(),
                    available: TEMPLATES
                        .iter()
                        .map(|t| t.id)
                        .collect::<Vec<_>>()
                        .join(", "),
                })?;
                with_refinement(template.query, refine.as_deref())
            }
            // Resolving needs the catalog; `run_query` and `export_query` do that.
            Self::Saved { name, .. } => Err(QueryError::SavedQueryUnavailable(name.clone())),
            Self::Ast { query } => Ok(query.clone()),
        }
    }
}

/// `base` refined by optional extra text (see [`QuerySpec::Template`]).
pub(super) fn with_refinement(base: &str, refine: Option<&str>) -> Result<Query, QueryError> {
    let base = parse_text(base)?;
    match refine.map(str::trim) {
        Some(text) if !text.is_empty() => Ok(refined(base, parse_text(text)?)),
        _ => Ok(base),
    }
}

fn parse_text(text: &str) -> Result<Query, QueryError> {
    parse(text).map_err(|source| QueryError::Parse {
        text: text.to_owned(),
        source,
    })
}

fn refined(base: Query, refinement: Query) -> Query {
    let shape_changed = !refinement.group.is_empty() || !refinement.metrics.is_empty();
    Query {
        filter: Expr::and(base.filter.into_iter().chain(refinement.filter)),
        group: if shape_changed {
            refinement.group
        } else {
            base.group
        },
        metrics: if shape_changed {
            refinement.metrics
        } else {
            base.metrics
        },
        // Sort keys refer to result columns, so a new shape invalidates the template's.
        sort: if !refinement.sort.is_empty() || shape_changed {
            refinement.sort
        } else {
            base.sort
        },
        limit: refinement.limit.or(base.limit),
    }
}

/// A built-in report as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplateInfo {
    /// Stable identifier.
    pub id: &'static str,
    /// Short English title.
    pub title: &'static str,
    /// What the report answers.
    pub description: &'static str,
    /// Query text.
    pub query: &'static str,
}

/// All built-in report templates in menu order.
pub fn templates() -> Vec<TemplateInfo> {
    TEMPLATES
        .iter()
        .map(|t| TemplateInfo {
            id: t.id,
            title: t.title,
            description: t.description,
            query: t.query,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use octopuson_query::to_dsl;

    use super::*;

    fn template(id: &str, refine: Option<&str>) -> Result<Query, QueryError> {
        QuerySpec::Template {
            id: id.into(),
            refine: refine.map(Into::into),
        }
        .resolve()
    }

    #[test]
    fn refinement_adds_filters_and_keeps_the_shape() {
        let query = template("client-errors", Some("date:last7d limit:20")).unwrap();
        assert_eq!(
            to_dsl(&query),
            "status:4xx date:last7d group:url,status metric:hits,last_seen sort:hits:desc limit:20"
        );
    }

    #[test]
    fn refinement_with_grouping_replaces_shape_and_sort() {
        let query = template("server-errors", Some("group:day")).unwrap();
        assert_eq!(to_dsl(&query), "status:5xx group:day limit:500");
    }

    #[test]
    fn unknown_templates_and_bad_text_are_reported() {
        assert!(matches!(
            template("nope", None),
            Err(QueryError::UnknownTemplate { .. })
        ));
        let error = QuerySpec::text("status:abc").resolve().unwrap_err();
        assert!(matches!(error, QueryError::Parse { ref text, .. } if text == "status:abc"));
    }

    #[test]
    fn specs_deserialize_from_tagged_json() {
        let spec: QuerySpec =
            serde_json::from_str(r#"{"kind":"template","id":"top-urls"}"#).unwrap();
        assert_eq!(
            spec,
            QuerySpec::Template {
                id: "top-urls".into(),
                refine: None
            }
        );
    }
}
