//! Import Google's Desktop client JSON without retaining the downloaded file or exposing secrets.
use super::{OpportunityWorkspace, oauth};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::sync::PoisonError;

const CLIENT_ACCOUNT: &str = "oauth-desktop-client";
const MAX_CLIENT_BYTES: u64 = 64 * 1024;

// Deliberately not Debug: neither errors nor IPC responses may include the client secret.
#[derive(Serialize, Deserialize)]
pub(super) struct DesktopClient {
    pub client_id: String,
    pub client_secret: Option<String>,
}

pub(super) fn configured_client() -> Result<Option<DesktopClient>, String> {
    if let Some(saved) = oauth::load_refresh(CLIENT_ACCOUNT)? {
        return serde_json::from_str(&saved)
            .map(Some)
            .map_err(|_| "Saved Google OAuth configuration is invalid".into());
    }
    let id = std::env::var("OCTOPUSSEO_GOOGLE_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("OCTOPUSSEO_GOOGLE_CLIENT_ID").map(str::to_owned));
    Ok(id
        .filter(|id| valid_client_id(id))
        .map(|client_id| DesktopClient {
            client_id,
            client_secret: std::env::var("OCTOPUSSEO_GOOGLE_CLIENT_SECRET")
                .ok()
                .or_else(|| option_env!("OCTOPUSSEO_GOOGLE_CLIENT_SECRET").map(str::to_owned)),
        }))
}

fn valid_client_id(id: &str) -> bool {
    id.len() > ".apps.googleusercontent.com".len()
        && id.len() <= 256
        && id.ends_with(".apps.googleusercontent.com")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
}

fn parse_client(bytes: &[u8]) -> Result<DesktopClient, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Choose the original Desktop app OAuth JSON downloaded from Google Cloud")?;
    let installed = value.get("installed").filter(|v| v.is_object()).ok_or(
        "A Desktop app OAuth client is required; Web clients and service-account keys are unsupported",
    )?;
    let client: DesktopClient = serde_json::from_value(installed.clone())
        .map_err(|_| "The Google Desktop client JSON is missing its client ID")?;
    if !valid_client_id(&client.client_id) {
        return Err("Invalid Google Desktop OAuth client ID".into());
    }
    if client
        .client_secret
        .as_ref()
        .is_none_or(|s| s.is_empty() || s.len() > 1024 || s.bytes().any(|b| b.is_ascii_control()))
    {
        return Err("The Google Desktop client JSON is missing a valid client secret".into());
    }
    // Endpoints and redirect URIs from the file are never used. OAuth uses fixed Google hosts
    // and a fresh loopback port, so an edited JSON file cannot redirect credentials elsewhere.
    Ok(client)
}

impl OpportunityWorkspace {
    /// # Errors
    /// Rejects non-desktop/oversized files, active connections and credential-store failures.
    pub fn import_google_client(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::open(path).map_err(|_| "Cannot read the selected OAuth file")?;
        if !file
            .metadata()
            .map_err(|_| "Cannot inspect the OAuth file")?
            .is_file()
        {
            return Err("Choose a Google Desktop OAuth JSON file".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_CLIENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read the selected OAuth file")?;
        if bytes.len() as u64 > MAX_CLIENT_BYTES {
            return Err("Google OAuth configuration must be smaller than 64 KiB".into());
        }
        let client = parse_client(&bytes)?;
        let _attempts = oauth::configuration_guard()?;
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        if !self.read()?.connections.is_empty() {
            return Err(
                "Disconnect existing Google accounts before replacing the OAuth client".into(),
            );
        }
        let encoded = serde_json::to_string(&client)
            .map_err(|_| "Cannot encode the Google OAuth configuration")?;
        oauth::save_refresh(CLIENT_ACCOUNT, &encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_google_desktop_clients_are_accepted_without_trusting_file_endpoints() {
        for bytes in [
            br#"{"web":{"client_id":"x.apps.googleusercontent.com"}}"#.as_slice(),
            br#"{"type":"service_account","private_key":"secret"}"#,
            br#"{"installed":{"client_id":"https://evil.example","client_secret":"secret"}}"#,
            br#"{"installed":{"client_id":"x.apps.googleusercontent.com"}}"#,
            br"not json secret",
        ] {
            let error = parse_client(bytes).err().unwrap();
            assert!(!error.contains("not json secret"));
            assert!(!error.contains("evil.example"));
        }
        let parsed = parse_client(br#"{"installed":{"client_id":"123-example.apps.googleusercontent.com","client_secret":"fixture-value","token_uri":"https://evil.example","redirect_uris":["https://evil.example"]}}"#).unwrap();
        assert_eq!(parsed.client_id, "123-example.apps.googleusercontent.com");
        assert_eq!(parsed.client_secret.as_deref(), Some("fixture-value"));
    }
    #[test]
    fn oversized_client_files_leave_workspace_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = OpportunityWorkspace::new(dir.path().join("data"));
        let file = dir.path().join("client.json");
        std::fs::write(
            &file,
            vec![b' '; usize::try_from(MAX_CLIENT_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert!(
            workspace
                .import_google_client(&file)
                .unwrap_err()
                .contains("64 KiB")
        );
        assert!(!workspace.root.exists());
    }
}
