//! Scores that show their working.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One part of a score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    /// Stable id (`placement`, `coverage`, ...).
    pub id: String,
    /// Weight in the model.
    pub weight: f64,
    /// The measured value (words, a ratio, a rank), when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The value mapped onto 0–1; `None` when the signal is missing.
    pub normalized: Option<f64>,
    /// Points it adds to the score (0–100 in total).
    pub contribution: f64,
    /// i18n key of the explanation (`content.placement.title_missing`).
    pub reason: String,
    /// Values the explanation mentions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
}

/// A score from 0 to 100 with its parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Breakdown {
    /// The score, rounded to one decimal.
    pub score: f64,
    /// Model version (`content-1`).
    pub model: String,
    /// Share of the model's weight backed by data (1 when no signal was missing).
    pub confidence: f64,
    /// The parts, in model order.
    pub components: Vec<Component>,
}

impl Breakdown {
    /// The score as a whole number.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0–100
    pub fn rounded(&self) -> u8 {
        self.score.round().clamp(0.0, 100.0) as u8
    }

    /// A component by id.
    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }
}

/// Collects the parts of a score.
#[derive(Debug, Clone, Default)]
pub(crate) struct Builder {
    components: Vec<Component>,
}

impl Builder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds a part; `normalized` is clamped to 0–1, `None` marks a missing signal.
    pub(crate) fn add(
        &mut self,
        id: &str,
        weight: f64,
        value: Option<f64>,
        normalized: Option<f64>,
        reason: &str,
    ) -> &mut Component {
        self.components.push(Component {
            id: id.to_owned(),
            weight,
            value,
            normalized: normalized.map(|normalized| normalized.clamp(0.0, 1.0)),
            contribution: 0.0,
            reason: reason.to_owned(),
            params: BTreeMap::new(),
        });
        let last = self.components.len() - 1;
        &mut self.components[last]
    }

    /// The score: the weighted mean of the parts with data, renormalised over their weights.
    pub(crate) fn finish(mut self, model: &str) -> Breakdown {
        let total: f64 = self
            .components
            .iter()
            .map(|component| component.weight)
            .sum();
        let available: f64 = self
            .components
            .iter()
            .filter(|component| component.normalized.is_some())
            .map(|component| component.weight)
            .sum();
        for component in &mut self.components {
            component.contribution = match component.normalized {
                Some(normalized) if available > 0.0 => {
                    round(normalized * component.weight / available * 100.0)
                }
                _ => 0.0,
            };
        }
        let score = if available > 0.0 {
            self.components
                .iter()
                .filter_map(|component| {
                    component
                        .normalized
                        .map(|normalized| normalized * component.weight)
                })
                .sum::<f64>()
                / available
                * 100.0
        } else {
            0.0
        };
        Breakdown {
            score: round(score.clamp(0.0, 100.0)),
            model: model.to_owned(),
            confidence: if total > 0.0 {
                round(available / total)
            } else {
                0.0
            },
            components: self.components,
        }
    }
}

impl Component {
    /// Adds a value the explanation mentions.
    pub(crate) fn with(&mut self, key: &str, value: impl Into<Value>) -> &mut Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

/// Rounds to one decimal (two for values below 1).
pub(crate) fn round(value: f64) -> f64 {
    if value.abs() < 1.0 {
        (value * 100.0).round() / 100.0
    } else {
        (value * 10.0).round() / 10.0
    }
}

/// Median of values (0 for none).
pub(crate) fn median(values: &mut [f64]) -> f64 {
    percentile(values, 0.5)
}

/// Linear-interpolated percentile `q` (0–1) of values (0 for none).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(crate) fn percentile(values: &mut [f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let position = q.clamp(0.0, 1.0) * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    values[lower] + (values[upper] - values[lower]) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renormalises_over_available_signals() {
        let mut builder = Builder::new();
        builder.add("a", 30.0, None, Some(1.0), "a");
        builder.add("b", 10.0, None, None, "b");
        builder.add("c", 10.0, None, Some(0.0), "c");
        let breakdown = builder.finish("test-1");
        assert!((breakdown.score - 75.0).abs() < 1e-9);
        assert!((breakdown.confidence - 0.8).abs() < 1e-9);
        assert_eq!(breakdown.component("a").unwrap().contribution, 75.0);
        assert_eq!(breakdown.component("b").unwrap().contribution, 0.0);
        assert_eq!(Builder::new().finish("empty").score, 0.0);
    }

    #[test]
    fn computes_percentiles() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(percentile(&mut [10.0, 20.0, 30.0, 40.0, 50.0], 0.25), 20.0);
        assert_eq!(median(&mut []), 0.0);
    }
}
