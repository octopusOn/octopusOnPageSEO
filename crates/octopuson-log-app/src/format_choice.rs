use std::path::Path;
use std::sync::Arc;

use octopuson_logparse::{
    FormatRegistry, LogFormat, TemplateError, TemplateFormat, detect, input::InputError,
};

use crate::sample::read_sample;

/// Number of non-empty lines used for automatic detection.
const DETECTION_SAMPLE_LINES: usize = 200;

/// How the log format of an import is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatChoice {
    /// Detect from the first lines of the first file.
    Auto,
    /// A built-in format id such as `traefik-clf`.
    Builtin(String),
    /// A custom Nginx `log_format` string.
    Nginx(String),
    /// A custom Apache `LogFormat` string.
    Apache(String),
}

/// A built-in log format as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FormatInfo {
    /// Id used with [`FormatChoice::Builtin`].
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Every built-in log format.
pub fn builtin_formats() -> Vec<FormatInfo> {
    FormatRegistry::builtin()
        .iter()
        .map(|format| FormatInfo {
            id: format.id().to_owned(),
            name: format.display_name().to_owned(),
        })
        .collect()
}

/// A resolved format plus how confident detection was (`None` when chosen explicitly).
#[derive(Debug, Clone)]
pub(crate) struct ResolvedFormat {
    pub(crate) format: Arc<dyn LogFormat>,
    pub(crate) confidence: Option<f64>,
}

/// The requested format could not be resolved.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FormatError {
    /// No built-in format has this id.
    #[error("unknown format `{requested}`; available: {available}")]
    Unknown {
        /// Requested id.
        requested: String,
        /// Comma-separated list of valid ids.
        available: String,
    },
    /// A custom format string is invalid.
    #[error("invalid custom log format")]
    Template(#[from] TemplateError),
    /// No format matched the sample.
    #[error("could not detect the log format of `{0}`; pass --format or a custom format")]
    NotDetected(String),
    /// The sample could not be read.
    #[error(transparent)]
    Input(#[from] InputError),
    /// A stored format specification is malformed.
    #[error("invalid stored format specification `{0}`")]
    InvalidSpec(String),
}

const AUTO_SPEC: &str = "auto";
const BUILTIN_PREFIX: &str = "builtin";
const NGINX_PREFIX: &str = "nginx";
const APACHE_PREFIX: &str = "apache";

impl FormatChoice {
    /// Serializes the choice for storage, e.g. `builtin:traefik-clf` or `nginx:<format>`.
    pub fn to_spec(&self) -> String {
        match self {
            Self::Auto => AUTO_SPEC.to_owned(),
            Self::Builtin(id) => format!("{BUILTIN_PREFIX}:{id}"),
            Self::Nginx(format) => format!("{NGINX_PREFIX}:{format}"),
            Self::Apache(format) => format!("{APACHE_PREFIX}:{format}"),
        }
    }

    /// Parses a value produced by [`to_spec`](Self::to_spec).
    ///
    /// # Errors
    /// [`FormatError::InvalidSpec`] for unknown prefixes.
    pub fn from_spec(spec: &str) -> Result<Self, FormatError> {
        if spec == AUTO_SPEC {
            return Ok(Self::Auto);
        }
        let invalid = || FormatError::InvalidSpec(spec.to_owned());
        let (kind, value) = spec.split_once(':').ok_or_else(invalid)?;
        match kind {
            BUILTIN_PREFIX => Ok(Self::Builtin(value.to_owned())),
            NGINX_PREFIX => Ok(Self::Nginx(value.to_owned())),
            APACHE_PREFIX => Ok(Self::Apache(value.to_owned())),
            _ => Err(invalid()),
        }
    }

    /// Checks that an explicit choice is usable (known id, valid custom format).
    ///
    /// # Errors
    /// See [`FormatError`].
    pub fn validate(&self, registry: &FormatRegistry) -> Result<(), FormatError> {
        self.resolve_explicit(registry).map(|_| ())
    }

    /// Resolves the choice into a concrete format. `first_file` is sampled for `Auto`.
    pub(crate) fn resolve(
        &self,
        registry: &FormatRegistry,
        first_file: &Path,
    ) -> Result<ResolvedFormat, FormatError> {
        if let Some(format) = self.resolve_explicit(registry)? {
            return Ok(ResolvedFormat {
                format,
                confidence: None,
            });
        }
        let sample = read_sample(first_file, DETECTION_SAMPLE_LINES)?;
        let detection = detect(registry, &sample)
            .ok_or_else(|| FormatError::NotDetected(first_file.display().to_string()))?;
        Ok(ResolvedFormat {
            confidence: Some(detection.confidence()),
            format: detection.format,
        })
    }

    /// The format of an explicit choice, or `None` for [`FormatChoice::Auto`].
    fn resolve_explicit(
        &self,
        registry: &FormatRegistry,
    ) -> Result<Option<Arc<dyn LogFormat>>, FormatError> {
        let format: Arc<dyn LogFormat> = match self {
            Self::Auto => return Ok(None),
            Self::Builtin(id) => registry
                .get(id)
                .cloned()
                .ok_or_else(|| FormatError::Unknown {
                    requested: id.clone(),
                    available: registry
                        .iter()
                        .map(|f| f.id())
                        .collect::<Vec<_>>()
                        .join(", "),
                })?,
            Self::Nginx(format) => Arc::new(TemplateFormat::from_nginx(
                "custom-nginx",
                "Custom Nginx",
                format,
            )?),
            Self::Apache(format) => Arc::new(TemplateFormat::from_apache(
                "custom-apache",
                "Custom Apache",
                format,
            )?),
        };
        Ok(Some(format))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_round_trip() {
        for choice in [
            FormatChoice::Auto,
            FormatChoice::Builtin("traefik-clf".into()),
            FormatChoice::Nginx("$remote_addr - [$time_local] \"$request\" $status".into()),
            FormatChoice::Apache("%h %l %u %t \"%r\" %>s %b".into()),
        ] {
            assert_eq!(FormatChoice::from_spec(&choice.to_spec()).unwrap(), choice);
        }
        assert!(FormatChoice::from_spec("iis:x").is_err());
        assert!(FormatChoice::from_spec("garbage").is_err());
    }

    #[test]
    fn lists_builtin_formats() {
        let ids: Vec<String> = builtin_formats().into_iter().map(|f| f.id).collect();
        assert!(ids.contains(&"traefik-clf".to_owned()));
        assert!(ids.contains(&"combined".to_owned()));
    }

    #[test]
    fn validate_rejects_unknown_builtin() {
        let registry = FormatRegistry::builtin();
        assert!(FormatChoice::Auto.validate(&registry).is_ok());
        assert!(
            FormatChoice::Builtin("lighttpd-xyz".into())
                .validate(&registry)
                .is_err()
        );
    }
}
