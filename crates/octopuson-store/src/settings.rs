/// Resource limits of the storage engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreSettings {
    /// DuckDB memory limit in MiB; larger operations spill to disk. DuckDB's own default
    /// (80% of RAM) is too greedy for a desktop application.
    pub memory_limit_mb: u32,
}

impl Default for StoreSettings {
    fn default() -> Self {
        Self {
            memory_limit_mb: 1024,
        }
    }
}
