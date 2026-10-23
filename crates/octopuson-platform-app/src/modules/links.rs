//! Link work: decisions on linking sites after the toxic-link audit (keep, watch, disavow), the
//! disavow file Google takes, and link prospects with their outreach state. All free: these
//! are the user's own records.

use octopuson_platform::{LinkDecision, LinkProspect, normalize_domain};
use serde::{Deserialize, Serialize};

use crate::router::{Context, Router};
use crate::{AppError, now};

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

fn link_decisions(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<LinkDecision>, AppError> {
    Ok(context.workspace.store()?.link_decisions(ask.project_id)?)
}

#[derive(Debug, Deserialize)]
struct DecideAsk {
    project_id: i64,
    /// Linking sites decided about together.
    domains: Vec<String>,
    /// `keep`, `watch` or `disavow`; none forgets the decision.
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// Decides about linking sites; returns every decision of the project.
fn decide_links(context: &Context<'_>, ask: DecideAsk) -> Result<Vec<LinkDecision>, AppError> {
    if ask.domains.is_empty() {
        return Err(AppError::Invalid(
            "domains: choose at least one site".to_owned(),
        ));
    }
    let store = context.workspace.store()?;
    let at = now();
    for domain in &ask.domains {
        store.decide_link(
            ask.project_id,
            domain,
            ask.decision.as_deref(),
            ask.note.as_deref(),
            &at,
        )?;
    }
    Ok(store.link_decisions(ask.project_id)?)
}

/// A disavow file to upload to Google Search Console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DisavowFile {
    file_name: String,
    text: String,
    domains: usize,
}

/// The disavow file of the sites decided `disavow`: one `domain:` line each, as Google takes it
/// (plain text, UTF-8, `#` comments).
fn disavow_text(site: &str, domains: &[String], date: &str) -> String {
    let mut text = format!(
        "# Disavow file for {site}\n# Made by OctopusOn GEO-SEO Platform on {date}\n# {} domains\n",
        domains.len()
    );
    for domain in domains {
        text.push_str("domain:");
        text.push_str(domain);
        text.push('\n');
    }
    text
}

fn disavow_file(context: &Context<'_>, ask: ProjectAsk) -> Result<DisavowFile, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let domains: Vec<String> = store
        .link_decisions(ask.project_id)?
        .into_iter()
        .filter(|decision| decision.decision == "disavow")
        .map(|decision| decision.domain)
        .collect();
    let date: String = now().chars().take(10).collect();
    Ok(DisavowFile {
        file_name: format!("disavow-{}.txt", normalize_domain(&project.domain)),
        text: disavow_text(&project.domain, &domains, &date),
        domains: domains.len(),
    })
}

fn link_prospects(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<LinkProspect>, AppError> {
    Ok(context.workspace.store()?.prospects(ask.project_id)?)
}

#[derive(Debug, Deserialize)]
struct ProspectAsk {
    project_id: i64,
    prospect: LinkProspect,
}

fn save_link_prospect(context: &Context<'_>, ask: ProspectAsk) -> Result<LinkProspect, AppError> {
    Ok(context
        .workspace
        .store()?
        .save_prospect(ask.project_id, &ask.prospect, &now())?)
}

#[derive(Debug, Deserialize)]
struct RemoveAsk {
    project_id: i64,
    domain: String,
}

fn remove_link_prospect(context: &Context<'_>, ask: RemoveAsk) -> Result<(), AppError> {
    Ok(context
        .workspace
        .store()?
        .remove_prospect(ask.project_id, &ask.domain)?)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("link_decisions", link_decisions);
    router.add("decide_links", decide_links);
    router.add("disavow_file", disavow_file);
    router.add("link_prospects", link_prospects);
    router.add("save_link_prospect", save_link_prospect);
    router.add("remove_link_prospect", remove_link_prospect);
}

#[cfg(test)]
mod tests {
    use super::disavow_text;

    #[test]
    fn the_disavow_file_has_a_domain_line_each() {
        let text = disavow_text(
            "ornek.com",
            &["spam.xyz".to_owned(), "farm.net".to_owned()],
            "2026-10-23",
        );
        assert_eq!(
            text,
            "# Disavow file for ornek.com\n# Made by OctopusOn GEO-SEO Platform on 2026-10-23\n# 2 domains\ndomain:spam.xyz\ndomain:farm.net\n"
        );
    }
}
