//! Keys and tokens on the server, sealed with the master key.
//!
//! Every account's own API keys and its OctopusOn SEO token live in its folder, each value
//! encrypted with AES-256-GCM under the server's master key, the value's name bound in as
//! associated data so a sealed value cannot be moved to another name.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_secrets::{SecretBackend, SecretError};

/// Bytes of a nonce.
const NONCE: usize = 12;

/// A JSON file of sealed values.
pub struct SealedFile {
    path: PathBuf,
    cipher: Aes256Gcm,
    write: Mutex<()>,
}

impl std::fmt::Debug for SealedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedFile")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn store_error(message: impl Into<String>) -> SecretError {
    SecretError::Store(message.into())
}

impl SealedFile {
    /// Values in `path`, sealed with `key`.
    pub fn new(path: PathBuf, key: &[u8; 32]) -> Self {
        Self {
            path,
            cipher: Aes256Gcm::new(key.into()),
            write: Mutex::new(()),
        }
    }

    fn read(&self) -> Result<BTreeMap<String, String>, SecretError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| store_error(format!("{}: {error}", self.path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(source) => Err(SecretError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }

    fn write(&self, values: &BTreeMap<String, String>) -> Result<(), SecretError> {
        let bytes =
            serde_json::to_vec_pretty(values).map_err(|error| store_error(error.to_string()))?;
        octopuson_secrets::write_private(&self.path, &bytes)
    }

    fn seal(&self, key: &str, value: &str) -> Result<String, SecretError> {
        let mut nonce = [0_u8; NONCE];
        getrandom::fill(&mut nonce).map_err(|error| store_error(error.to_string()))?;
        let sealed = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: value.as_bytes(),
                    aad: key.as_bytes(),
                },
            )
            .map_err(|_| store_error("cannot seal the value"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend_from_slice(&sealed);
        Ok(STANDARD.encode(bytes))
    }

    fn open(&self, key: &str, text: &str) -> Result<String, SecretError> {
        let bytes = STANDARD
            .decode(text)
            .map_err(|_| store_error("a sealed value is damaged"))?;
        if bytes.len() <= NONCE {
            return Err(store_error("a sealed value is damaged"));
        }
        let (nonce, sealed) = bytes.split_at(NONCE);
        let plain = self
            .cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: sealed,
                    aad: key.as_bytes(),
                },
            )
            .map_err(|_| store_error("a sealed value cannot be opened with this master key"))?;
        String::from_utf8(plain).map_err(|_| store_error("a sealed value is damaged"))
    }
}

impl SecretBackend for SealedFile {
    fn name(&self) -> &'static str {
        "sealed-file"
    }

    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        self.read()?
            .get(key)
            .map(|text| self.open(key, text))
            .transpose()
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let _guard = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        let mut values = self.read()?;
        values.insert(key.to_owned(), self.seal(key, value)?);
        self.write(&values)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        let _guard = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        let mut values = self.read()?;
        if values.remove(key).is_some() {
            self.write(&values)?;
        }
        Ok(())
    }
}
