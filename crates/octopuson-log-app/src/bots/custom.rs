//! User-defined bots, stored as `custom-bots.json` in the data directory.

use std::fs;
use std::io;

use octopuson_botverify::{CustomBot, CustomBotError, validate_custom_bots};
use serde::{Deserialize, Serialize};

use super::BotError;
use super::ranges::write_atomically;
use crate::workspace::Workspace;

const FILE_VERSION: u32 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
struct CustomBotsFile {
    version: u32,
    bots: Vec<CustomBot>,
}

/// The custom bots, validated.
///
/// # Errors
/// [`BotError::InvalidCustomBots`] when the file cannot be read or holds invalid bots.
pub fn list_custom_bots(workspace: &Workspace) -> Result<Vec<CustomBot>, BotError> {
    let path = workspace.data_dir().custom_bots_file();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(BotError::InvalidCustomBots(format!(
                "{}: {error}",
                path.display()
            )));
        }
    };
    parse_custom_bots(&text)
}

/// Parses a custom bots file (also accepted for import: `{"bots": [...]}` or a bare array).
///
/// # Errors
/// [`BotError::InvalidCustomBots`].
pub fn parse_custom_bots(text: &str) -> Result<Vec<CustomBot>, BotError> {
    let bots = match serde_json::from_str::<CustomBotsFile>(text) {
        Ok(file) => file.bots,
        Err(file_error) => serde_json::from_str::<Vec<CustomBot>>(text)
            .map_err(|_| BotError::InvalidCustomBots(file_error.to_string()))?,
    };
    Ok(validate_custom_bots(bots)?)
}

/// Validates and stores the complete list of custom bots; returns it normalized. Stored hits
/// are re-classified the next time a site is opened.
///
/// # Errors
/// [`BotError::InvalidCustomBots`] for invalid bots, [`BotError::Write`] when the file cannot
/// be written.
pub fn save_custom_bots(
    workspace: &Workspace,
    bots: Vec<CustomBot>,
) -> Result<Vec<CustomBot>, BotError> {
    let bots = validate_custom_bots(bots)?;
    let path = workspace.data_dir().custom_bots_file();
    let file = CustomBotsFile {
        version: FILE_VERSION,
        bots,
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| BotError::InvalidCustomBots(e.to_string()))?;
    write_atomically(&path, &json).map_err(|source| BotError::Write { path, source })?;
    Ok(file.bots)
}

/// Custom bots for classification: an unreadable file is reported once and ignored, so a
/// typo never stops imports or queries.
pub(crate) fn custom_bots_or_none(workspace: &Workspace) -> Vec<CustomBot> {
    list_custom_bots(workspace).unwrap_or_else(|error| {
        tracing::warn!(%error, "ignoring custom bots");
        Vec::new()
    })
}

impl From<CustomBotError> for BotError {
    fn from(error: CustomBotError) -> Self {
        Self::InvalidCustomBots(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_lists_and_parses_imports() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::at(dir.path());
        assert_eq!(list_custom_bots(&workspace).unwrap().len(), 0);
        let saved = save_custom_bots(
            &workspace,
            vec![CustomBot {
                id: "Acme-Monitor".into(),
                patterns: vec!["AcmeMonitor".into()],
                ..CustomBot::default()
            }],
        )
        .unwrap();
        assert_eq!(saved[0].id, "acme-monitor");
        assert_eq!(list_custom_bots(&workspace).unwrap(), saved);
        let bare = r#"[{"id": "x-bot", "patterns": ["xbot"], "category": "seo"}]"#;
        assert_eq!(parse_custom_bots(bare).unwrap()[0].category, "seo");
        assert!(matches!(
            save_custom_bots(&workspace, vec![CustomBot::default()]),
            Err(BotError::InvalidCustomBots(_))
        ));
        std::fs::write(workspace.data_dir().custom_bots_file(), "{oops").unwrap();
        assert!(list_custom_bots(&workspace).is_err());
        assert_eq!(custom_bots_or_none(&workspace).len(), 0);
    }
}
