use std::sync::Arc;

use crate::elb::ElbFormat;
use crate::format::LogFormat;
use crate::haproxy::HaproxyFormat;
use crate::json::TraefikJsonFormat;
use crate::json_any::JsonFormat;
use crate::template::presets;
use crate::w3c::W3cFormat;

/// The set of formats available for parsing and detection.
#[derive(Debug, Clone, Default)]
pub struct FormatRegistry {
    formats: Vec<Arc<dyn LogFormat>>,
}

impl FormatRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry with all built-in formats: Traefik CLF and JSON, combined, common, IIS,
    /// CloudFront, AWS ELB/ALB, HAProxy and generic JSON. Formats described by a header in the
    /// file itself (`#Fields:`, CSV header rows) are added by [`detect`](crate::detect).
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(TraefikJsonFormat));
        registry.register(Arc::new(JsonFormat));
        registry.register(Arc::new(ElbFormat));
        registry.register(Arc::new(HaproxyFormat));
        for w3c in [W3cFormat::iis(), W3cFormat::cloudfront()]
            .into_iter()
            .flatten()
        {
            registry.register(Arc::new(w3c));
        }
        // Presets are static token lists covered by tests; a failure here is a programming
        // error that `builtin_registry_contains_all_presets` catches.
        for preset in [
            presets::traefik_clf(),
            presets::combined(),
            presets::common(),
        ]
        .into_iter()
        .flatten()
        {
            registry.register(Arc::new(preset));
        }
        registry
    }

    /// Adds a format. A format with the same id replaces the existing one.
    pub fn register(&mut self, format: Arc<dyn LogFormat>) {
        self.formats.retain(|existing| existing.id() != format.id());
        self.formats.push(format);
    }

    /// Looks up a format by id.
    pub fn get(&self, id: &str) -> Option<&Arc<dyn LogFormat>> {
        self.formats.iter().find(|format| format.id() == id)
    }

    /// Iterates over all registered formats.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn LogFormat>> {
        self.formats.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_contains_all_presets() {
        let registry = FormatRegistry::builtin();
        let mut ids: Vec<_> = registry.iter().map(|f| f.id().to_owned()).collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "aws-elb",
                "cloudfront",
                "combined",
                "common",
                "haproxy",
                "iis",
                "json",
                "traefik-clf",
                "traefik-json"
            ]
        );
    }

    #[test]
    fn register_replaces_same_id() {
        let mut registry = FormatRegistry::builtin();
        let before = registry.iter().count();
        registry.register(Arc::new(TraefikJsonFormat));
        assert_eq!(registry.iter().count(), before);
    }
}
