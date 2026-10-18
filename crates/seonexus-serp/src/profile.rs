//! Built-in HTML parsers of Google, Bing and Yandex, and user overrides of them.
//!
//! Each parser is a [`Mapping`] in a versioned JSON profile embedded in the binary. Engines
//! change their markup without notice, so a user can patch a profile with a JSON merge patch
//! (RFC 7396) on its mapping, test it against stored pages and keep working before an update
//! ships a new revision.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mapping::{CompiledMapping, Mapping};
use crate::{Engine, SerpError};

/// A parser profile: a mapping for one engine's result pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserProfile {
    /// Profile id (`google-html`).
    pub id: String,
    /// Revision, raised whenever the built-in selectors change.
    pub revision: u32,
    /// Engine whose pages it reads.
    pub engine: Engine,
    /// The selectors.
    pub mapping: Mapping,
}

impl ParserProfile {
    /// `id@revision`, recorded with every snapshot it parsed.
    pub fn key(&self) -> String {
        format!("{}@{}", self.id, self.revision)
    }

    /// Compiles the mapping.
    ///
    /// # Errors
    /// [`SerpError::InvalidSpec`] for invalid selectors.
    pub fn compile(&self) -> Result<CompiledMapping, SerpError> {
        self.mapping.compile()
    }
}

/// JSON source of an engine's built-in profile.
pub fn builtin_source(engine: Engine) -> &'static str {
    match engine {
        Engine::Google => include_str!("profiles/google-html.json"),
        Engine::Bing => include_str!("profiles/bing-html.json"),
        Engine::Yandex => include_str!("profiles/yandex-html.json"),
    }
}

/// An engine's built-in profile.
///
/// # Errors
/// [`SerpError::InvalidSpec`] if the embedded JSON is invalid (a build defect the tests catch).
pub fn builtin(engine: Engine) -> Result<ParserProfile, SerpError> {
    serde_json::from_str(builtin_source(engine))
        .map_err(|error| SerpError::InvalidSpec(format!("built-in {engine} profile: {error}")))
}

/// A user's patch of a built-in profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileOverride {
    /// Engine whose profile is patched.
    pub engine: Engine,
    /// Profile key the patch was written against (`google-html@1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// JSON merge patch applied to the profile's mapping; `null` removes a member.
    pub mapping: Value,
    /// Whether the override is used.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

const fn enabled() -> bool {
    true
}

/// The profile an engine is parsed with after an override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveProfile {
    /// The patched profile; its key gains a `+local` suffix.
    pub profile: ParserProfile,
    /// Set when the patch was written for another revision of the built-in profile.
    pub stale_base: Option<String>,
}

impl EffectiveProfile {
    /// Key recorded with snapshots: `google-html@1`, or `google-html@1+local` when patched.
    pub fn key(&self, patched: bool) -> String {
        if patched {
            format!("{}+local", self.profile.key())
        } else {
            self.profile.key()
        }
    }
}

/// Applies an override to the built-in profile of its engine.
///
/// # Errors
/// [`SerpError::InvalidSpec`] when the patched mapping is not a valid mapping or does not
/// compile.
pub fn apply_override(
    builtin: &ParserProfile,
    patch: &ProfileOverride,
) -> Result<EffectiveProfile, SerpError> {
    if patch.engine != builtin.engine {
        return Err(SerpError::InvalidSpec(format!(
            "the override is for {}, the profile for {}",
            patch.engine, builtin.engine
        )));
    }
    let mut mapping = serde_json::to_value(&builtin.mapping)
        .map_err(|error| SerpError::InvalidSpec(error.to_string()))?;
    merge_patch(&mut mapping, &patch.mapping);
    let mapping: Mapping = serde_json::from_value(mapping)
        .map_err(|error| SerpError::InvalidSpec(format!("patched mapping: {error}")))?;
    mapping.compile()?;
    let stale_base = patch
        .base
        .as_ref()
        .filter(|base| **base != builtin.key())
        .cloned();
    Ok(EffectiveProfile {
        profile: ParserProfile {
            mapping,
            ..builtin.clone()
        },
        stale_base,
    })
}

/// RFC 7396 JSON merge patch: objects merge member by member, `null` removes a member and
/// anything else replaces the target.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        patch.clone_into(target);
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in patch {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn builtin_profiles_compile() {
        for engine in Engine::ALL {
            let profile = builtin(engine).unwrap();
            assert_eq!(profile.engine, engine);
            assert_eq!(
                profile.key(),
                format!("{}@{}", profile.id, profile.revision)
            );
            assert!(profile.revision >= 1);
            profile.compile().unwrap();
        }
    }

    #[test]
    fn merge_patch_follows_rfc_7396() {
        let mut target = json!({"a": "b", "c": {"d": "e", "f": "g"}});
        merge_patch(&mut target, &json!({"a": "z", "c": {"f": null}}));
        assert_eq!(target, json!({"a": "z", "c": {"d": "e"}}));
        merge_patch(&mut target, &json!(["x"]));
        assert_eq!(target, json!(["x"]));
    }

    #[test]
    fn overrides_patch_selectors_and_flag_stale_bases() {
        let google = builtin(Engine::Google).unwrap();
        let patch = ProfileOverride {
            engine: Engine::Google,
            base: Some("google-html@0".to_owned()),
            mapping: json!({"results": {"fields": {"title": "h3.custom"}}}),
            enabled: true,
        };
        let effective = apply_override(&google, &patch).unwrap();
        assert_eq!(effective.stale_base.as_deref(), Some("google-html@0"));
        assert_eq!(
            effective.profile.mapping.results.fields["title"],
            crate::mapping::Selectors::One("h3.custom".to_owned())
        );
        assert_eq!(effective.key(true), "google-html@1+local");

        let broken = ProfileOverride {
            mapping: json!({"results": {"fields": {"title": "h3["}}}),
            ..patch
        };
        assert!(apply_override(&google, &broken).is_err());
        let wrong_engine = ProfileOverride {
            engine: Engine::Bing,
            mapping: json!({}),
            base: None,
            enabled: true,
        };
        assert!(apply_override(&google, &wrong_engine).is_err());
    }
}
