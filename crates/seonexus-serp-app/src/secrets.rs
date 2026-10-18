//! API keys and other provider secrets, kept by the shared OctopusOn SEO secret store
//! (`seonexus-secrets`) under the SERP Checker's own keychain service.
//!
//! Values live in the operating system's credential store (macOS Keychain, Windows
//! Credential Manager), or in `secrets.json` readable by the owner only where there is none
//! (Linux) or when `OCTOPUSSEO_SECRET_STORE=file`. An environment variable
//! `OCTOPUSSEO_SECRET_<PROVIDER>_<NAME>` (upper case, `-` as `_`) overrides both, for scripts.
//!
//! Every saved secret is bound to the origins its provider sent requests to when it was
//! saved (`secret-bindings.json`, no values). If a provider spec is edited or imported so that
//! it points elsewhere, the secret counts as unset until it is entered again, so a spec can
//! never quietly send a key to a new host.

use std::path::Path;

#[cfg(any(target_os = "macos", windows))]
pub use seonexus_secrets::KeychainBackend;
pub use seonexus_secrets::{FileBackend, SecretBackend, SecretError, SecretState, Secrets};

/// Service name in the OS credential store.
pub const KEYCHAIN_SERVICE: &str = "octopusSEO SERP Finder";

/// The platform's default backend: the OS credential store where there is one.
pub fn default_backend(data_dir: &Path) -> Box<dyn SecretBackend> {
    seonexus_secrets::default_backend(data_dir, KEYCHAIN_SERVICE)
}
