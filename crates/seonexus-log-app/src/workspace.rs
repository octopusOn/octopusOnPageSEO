use std::path::{Path, PathBuf};

use seonexus_store::{DataDir, StoreError, StoreSettings};

/// Location of all persisted data used by the use-cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    data_dir: DataDir,
    settings: StoreSettings,
}

impl Workspace {
    /// Uses an explicit data directory.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: DataDir::new(root),
            settings: StoreSettings::default(),
        }
    }

    /// Uses `root` if given, otherwise the platform's application data directory.
    ///
    /// # Errors
    /// [`StoreError::NoDataDir`] when no default location exists.
    pub fn resolve(root: Option<PathBuf>) -> Result<Self, StoreError> {
        match root {
            Some(root) => Ok(Self::at(root)),
            None => DataDir::platform_default().map(|data_dir| Self {
                data_dir,
                settings: StoreSettings::default(),
            }),
        }
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        self.data_dir.root()
    }

    /// Caps the storage engine's memory use (MiB); larger operations spill to disk.
    #[must_use]
    pub fn with_memory_limit_mb(mut self, megabytes: u32) -> Self {
        self.settings.memory_limit_mb = megabytes;
        self
    }

    /// Storage engine memory limit in MiB.
    pub fn memory_limit_mb(&self) -> u32 {
        self.settings.memory_limit_mb
    }

    pub(crate) fn settings(&self) -> StoreSettings {
        self.settings
    }

    pub(crate) fn data_dir(&self) -> &DataDir {
        &self.data_dir
    }
}
