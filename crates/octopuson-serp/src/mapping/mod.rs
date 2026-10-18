//! Declarative mappings from a response body to results.
//!
//! A [`Mapping`] says where results, features, the result count and error messages are in a
//! response, in one of three [`Format`]s:
//!
//! - `json`: `JSONPath` (`$.organic[*]`) or JSON Pointer (`/organic`) expressions;
//! - `html`: CSS selectors, optionally followed by what to read: `@attr`, `::text` (default),
//!   `::own-text` or `::html`;
//! - `text`: regular expressions; a field reads the group named `v`, else group 1.
//!
//! Every selector may be a list of alternatives tried in order, so a mapping can keep working
//! across markup variants. `results.items` selects one node per result and `results.fields`
//! are evaluated relative to it. Known result fields are `url`, `title`, `snippet`,
//! `displayed_url`, `date`, `position`, `absolute_position`, `sitelink_titles[]` and
//! `sitelink_urls[]` (a `[]` suffix collects every match); feature fields are `text` and `url`.
//!
//! [`CompiledMapping::parse`] turns one response into a [`ParsedPage`]; the built-in engine
//! parsers ([`crate::profile`]) and user-defined providers ([`crate::spec`]) both use it.

mod assemble;
mod probe;

use std::collections::{BTreeMap, HashMap, HashSet};

use ego_tree::NodeId;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use url::Url;

pub use assemble::{Coverage, ParsedPage};

pub(crate) use self::probe::{Probe, json_text};
use self::probe::{compile_regex, regex_field};
use crate::challenge::{ChallengeRule, ChallengeRules};
use crate::{FeatureKind, SerpError};

/// Fields a result mapping may name.
pub const RESULT_FIELDS: &[&str] = &[
    "url",
    "title",
    "snippet",
    "displayed_url",
    "date",
    "position",
    "absolute_position",
    "sitelink_titles[]",
    "sitelink_urls[]",
];

/// Fields a feature mapping may name.
pub const FEATURE_FIELDS: &[&str] = &["text", "url"];

/// How a response body is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// JSON, selected with `JSONPath` or JSON Pointer.
    Json,
    /// HTML, selected with CSS.
    Html,
    /// Anything else, selected with regular expressions.
    Text,
}

/// One selector or alternatives tried in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Selectors {
    /// A single selector.
    One(String),
    /// Alternatives; the first that matches wins.
    Many(Vec<String>),
}

impl Selectors {
    /// The selectors in order.
    pub fn as_list(&self) -> Vec<&str> {
        match self {
            Self::One(one) => vec![one.as_str()],
            Self::Many(many) => many.iter().map(String::as_str).collect(),
        }
    }
}

impl From<&str> for Selectors {
    fn from(value: &str) -> Self {
        Self::One(value.to_owned())
    }
}

/// Where the results are and how they are cleaned up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemsSpec {
    /// One node per result (JSON, HTML) or one match per result (text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Selectors>,
    /// Text only: a regular expression whose named groups are the fields of each match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    /// Field selectors, relative to an item.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Selectors>,
    /// HTML only: items inside an element matching one of these selectors are skipped
    /// (results quoted by features, such as answers inside "People also ask").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_within: Vec<String>,
    /// Results whose URL starts with one of these prefixes are skipped (links back to the
    /// engine itself).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_urls: Vec<String>,
    /// Text only: decode HTML entities and strip tags in extracted values.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decode_entities: bool,
}

/// A feature block and its entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSpec {
    /// What the block is (`people-also-ask`, `ads-top`, ...; `paa` and other short names are
    /// accepted).
    #[serde(serialize_with = "kind_out", deserialize_with = "kind_in")]
    pub kind: FeatureKind,
    /// The block: an HTML element, a JSON node or a text match. One feature per HTML match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<Selectors>,
    /// HTML only: the block's first text must contain one of these phrases (case-insensitive),
    /// e.g. "People also ask" and "Diğer kullanıcılar şunları da sordu"; robust against class
    /// name changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<String>,
    /// Entries, relative to the block (HTML) or the response (JSON, text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Selectors>,
    /// `text` and `url` selectors, relative to an entry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Selectors>,
}

/// A clean-up step applied to result and feature URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostStep {
    /// Unwrap Google's `/url?q=` redirects.
    GoogleRedirect,
    /// Unwrap Bing's `/ck/a?...&u=a1<base64>` redirects.
    BingRedirect,
}

/// Where everything is in a response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    /// How the body is read.
    pub format: Format,
    /// Error message of a failed request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Selectors>,
    /// Result count the engine reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<Selectors>,
    /// Organic results.
    pub results: ItemsSpec,
    /// Feature blocks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureSpec>,
    /// Pages that are a consent dialog, CAPTCHA or block instead of results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenges: Vec<ChallengeRule>,
    /// URL clean-up steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post: Vec<PostStep>,
}

impl Mapping {
    /// Validates and compiles the mapping.
    ///
    /// # Errors
    /// [`SerpError::InvalidSpec`] for invalid selectors, unknown field names or selectors that
    /// do not suit the format.
    pub fn compile(&self) -> Result<CompiledMapping, SerpError> {
        CompiledMapping::new(self)
    }
}

#[derive(Debug, Clone)]
struct Field {
    name: String,
    list: bool,
    probes: Vec<Probe>,
}

#[derive(Debug, Clone)]
struct CompiledItems {
    items: Vec<Probe>,
    regex: Option<Regex>,
    fields: Vec<Field>,
    exclude_within: Vec<Selector>,
    skip_urls: Vec<String>,
    decode_entities: bool,
}

#[derive(Debug, Clone)]
struct CompiledFeature {
    kind: FeatureKind,
    container: Vec<Probe>,
    anchors: Vec<String>,
    items: Vec<Probe>,
    fields: Vec<Field>,
}

/// A mapping ready to run.
#[derive(Debug, Clone)]
pub struct CompiledMapping {
    format: Format,
    error: Vec<Probe>,
    total: Vec<Probe>,
    results: CompiledItems,
    features: Vec<CompiledFeature>,
    challenges: ChallengeRules,
    post: Vec<PostStep>,
}

/// Field values of one result or feature entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Record {
    /// Document order of the node it came from (HTML only).
    pub order: Option<usize>,
    pub values: BTreeMap<String, String>,
    pub lists: BTreeMap<String, Vec<String>>,
}

/// One feature block as extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeatureRecord {
    pub kind: FeatureKind,
    pub order: Option<usize>,
    pub items: Vec<Record>,
}

/// Everything extracted from one response, before clean-up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Extracted {
    pub error: Option<String>,
    pub total: Option<String>,
    pub results: Vec<Record>,
    pub features: Vec<FeatureRecord>,
}

fn compile_probes(format: Format, selectors: Option<&Selectors>) -> Result<Vec<Probe>, SerpError> {
    selectors
        .map(Selectors::as_list)
        .unwrap_or_default()
        .into_iter()
        .map(|source| Probe::compile(format, source))
        .collect()
}

fn compile_fields(
    format: Format,
    fields: &BTreeMap<String, Selectors>,
    allowed: &[&str],
) -> Result<Vec<Field>, SerpError> {
    fields
        .iter()
        .map(|(name, selectors)| {
            if !allowed.contains(&name.as_str()) {
                return Err(SerpError::InvalidSpec(format!(
                    "unknown field `{name}`; expected one of {}",
                    allowed.join(", ")
                )));
            }
            Ok(Field {
                name: name.trim_end_matches("[]").to_owned(),
                list: name.ends_with("[]"),
                probes: compile_probes(format, Some(selectors))?,
            })
        })
        .collect()
}

fn compile_items(format: Format, spec: &ItemsSpec) -> Result<CompiledItems, SerpError> {
    let invalid = |message: &str| Err(SerpError::InvalidSpec(message.to_owned()));
    match format {
        Format::Text if spec.regex.is_none() && spec.items.is_none() => {
            return invalid("text results need `regex` or `items`");
        }
        Format::Json | Format::Html if spec.regex.is_some() => {
            return invalid("`results.regex` is only for the text format");
        }
        Format::Json | Format::Html if spec.items.is_none() => {
            return invalid("results need `items`");
        }
        _ => {}
    }
    if format != Format::Html && !spec.exclude_within.is_empty() {
        return invalid("`exclude_within` is only for the html format");
    }
    let regex = spec.regex.as_deref().map(compile_regex).transpose()?;
    if let Some(regex) = &regex {
        let names: Vec<&str> = regex.capture_names().flatten().collect();
        if !names.contains(&"url") || !names.contains(&"title") {
            return invalid("the results regex needs named groups `url` and `title`");
        }
    }
    let fields = compile_fields(format, &spec.fields, RESULT_FIELDS)?;
    if regex.is_none() {
        for required in ["url", "title"] {
            if !fields.iter().any(|field| field.name == required) {
                return Err(SerpError::InvalidSpec(format!(
                    "results need a `{required}` field"
                )));
            }
        }
    }
    let exclude_within = spec
        .exclude_within
        .iter()
        .map(|source| {
            Selector::parse(source).map_err(|error| {
                SerpError::InvalidSpec(format!("invalid CSS selector `{source}`: {error}"))
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(CompiledItems {
        items: compile_probes(format, spec.items.as_ref())?,
        regex,
        fields,
        exclude_within,
        skip_urls: spec.skip_urls.clone(),
        decode_entities: spec.decode_entities,
    })
}

fn compile_feature(format: Format, feature: &FeatureSpec) -> Result<CompiledFeature, SerpError> {
    if feature.container.is_none() && feature.items.is_none() {
        return Err(SerpError::InvalidSpec(format!(
            "feature `{}` needs `container` or `items`",
            feature.kind.as_str()
        )));
    }
    if format == Format::Html && feature.container.is_none() {
        return Err(SerpError::InvalidSpec(format!(
            "html feature `{}` needs a `container`",
            feature.kind.as_str()
        )));
    }
    Ok(CompiledFeature {
        kind: feature.kind,
        container: compile_probes(format, feature.container.as_ref())?,
        anchors: feature
            .anchors
            .iter()
            .map(|anchor| crate::text::squash(anchor).to_lowercase())
            .filter(|anchor| !anchor.is_empty())
            .collect(),
        items: compile_probes(format, feature.items.as_ref())?,
        fields: compile_fields(format, &feature.fields, FEATURE_FIELDS)?,
    })
}

impl CompiledMapping {
    fn new(mapping: &Mapping) -> Result<Self, SerpError> {
        let format = mapping.format;
        Ok(Self {
            format,
            error: compile_probes(format, mapping.error.as_ref())?,
            total: compile_probes(format, mapping.total.as_ref())?,
            results: compile_items(format, &mapping.results)?,
            features: mapping
                .features
                .iter()
                .map(|feature| compile_feature(format, feature))
                .collect::<Result<_, _>>()?,
            challenges: ChallengeRules::compile(&mapping.challenges)?,
            post: mapping.post.clone(),
        })
    }

    /// The format the mapping reads.
    pub fn format(&self) -> Format {
        self.format
    }

    /// Reads one response: result page `page` (from 1), fetched from `page_url` (the final URL
    /// after redirects, used to resolve relative links and to detect challenges).
    ///
    /// # Errors
    /// [`SerpError::Parse`] when a JSON mapping gets a body that is not JSON.
    pub fn parse(&self, body: &str, page_url: &Url, page: u16) -> Result<ParsedPage, SerpError> {
        let (extracted, challenge) = match self.format {
            Format::Json => {
                let challenge = self.challenges.detect(page_url.as_str(), body, None);
                if challenge.is_some() && serde_json::from_str::<Value>(body).is_err() {
                    (Extracted::default(), challenge)
                } else {
                    let value: Value = serde_json::from_str(body).map_err(|error| {
                        SerpError::Parse(format!("the response is not JSON: {error}"))
                    })?;
                    (self.extract_json(&value), challenge)
                }
            }
            Format::Html => {
                let document = Html::parse_document(body);
                let challenge = self
                    .challenges
                    .detect(page_url.as_str(), body, Some(&document));
                (self.extract_html(&document), challenge)
            }
            Format::Text => (
                self.extract_text(body),
                self.challenges.detect(page_url.as_str(), body, None),
            ),
        };
        Ok(assemble::assemble(
            extracted,
            challenge,
            &assemble::Cleanup {
                base: page_url,
                post: &self.post,
                skip_urls: &self.results.skip_urls,
            },
            page,
        ))
    }

    fn extract_json(&self, root: &Value) -> Extracted {
        let first_text = |probes: &[Probe], value: &Value| {
            probes
                .iter()
                .find_map(|probe| probe.json(value).into_iter().find_map(json_text))
        };
        let nodes = |probes: &[Probe], value: &'_ Value| -> Vec<Value> {
            probes
                .iter()
                .map(|probe| flatten(probe.json(value)))
                .find(|nodes| !nodes.is_empty())
                .unwrap_or_default()
        };
        let record = |fields: &[Field], value: &Value| {
            let mut record = Record::default();
            for field in fields {
                if field.list {
                    let values: Vec<String> = field
                        .probes
                        .iter()
                        .map(|probe| {
                            flatten(probe.json(value))
                                .iter()
                                .filter_map(json_text)
                                .collect::<Vec<_>>()
                        })
                        .find(|values| !values.is_empty())
                        .unwrap_or_default();
                    record.lists.insert(field.name.clone(), values);
                } else if let Some(text) = first_text(&field.probes, value) {
                    record.values.insert(field.name.clone(), text);
                }
            }
            record
        };

        let results = nodes(&self.results.items, root)
            .iter()
            .map(|item| record(&self.results.fields, item))
            .collect();
        let features = self
            .features
            .iter()
            .filter_map(|feature| {
                let container = if feature.container.is_empty() {
                    None
                } else {
                    Some(nodes(&feature.container, root))
                };
                if container.as_ref().is_some_and(Vec::is_empty) {
                    return None;
                }
                let items: Vec<Record> = if feature.items.is_empty() {
                    container
                        .iter()
                        .flatten()
                        .map(|node| record(&feature.fields, node))
                        .collect()
                } else {
                    let scope = container
                        .as_ref()
                        .and_then(|nodes| nodes.first())
                        .unwrap_or(root);
                    nodes(&feature.items, scope)
                        .iter()
                        .map(|item| record(&feature.fields, item))
                        .collect()
                };
                (container.is_some() || !items.is_empty()).then(|| FeatureRecord {
                    kind: feature.kind,
                    order: None,
                    items: items.into_iter().filter(|item| !item.is_empty()).collect(),
                })
            })
            .collect();
        Extracted {
            error: first_text(&self.error, root),
            total: first_text(&self.total, root),
            results,
            features,
        }
    }

    fn extract_html(&self, document: &Html) -> Extracted {
        let root = document.root_element();
        let order: HashMap<NodeId, usize> = document
            .tree
            .root()
            .descendants()
            .enumerate()
            .map(|(index, node)| (node.id(), index))
            .collect();
        let read_first = |probes: &[Probe], scope: ElementRef<'_>| {
            probes.iter().find_map(|probe| {
                probe
                    .elements(scope)
                    .into_iter()
                    .find_map(|element| probe.read(element))
            })
        };
        let record = |fields: &[Field], element: ElementRef<'_>| {
            let mut record = Record {
                order: order.get(&element.id()).copied(),
                ..Record::default()
            };
            for field in fields {
                if field.list {
                    let values = field
                        .probes
                        .iter()
                        .map(|probe| {
                            probe
                                .elements(element)
                                .into_iter()
                                .filter_map(|found| probe.read(found))
                                .collect::<Vec<_>>()
                        })
                        .find(|values| !values.is_empty())
                        .unwrap_or_default();
                    record.lists.insert(field.name.clone(), values);
                } else if let Some(text) = read_first(&field.probes, element) {
                    record.values.insert(field.name.clone(), text);
                }
            }
            record
        };

        let excluded: HashSet<NodeId> = self
            .results
            .exclude_within
            .iter()
            .flat_map(|selector| document.select(selector).map(|element| element.id()))
            .collect();
        let results = first_elements(&self.results.items, root)
            .into_iter()
            .filter(|element| {
                excluded.is_empty()
                    || !std::iter::once(**element)
                        .chain(element.ancestors())
                        .any(|node| excluded.contains(&node.id()))
            })
            .map(|element| record(&self.results.fields, element))
            .collect();

        let mut features = Vec::new();
        for feature in &self.features {
            for container in feature_blocks(feature, root) {
                let items = if feature.items.is_empty() {
                    let entry = record(&feature.fields, container);
                    if entry.is_empty() {
                        Vec::new()
                    } else {
                        vec![entry]
                    }
                } else {
                    first_elements(&feature.items, container)
                        .into_iter()
                        .map(|element| record(&feature.fields, element))
                        .filter(|entry| !entry.is_empty())
                        .collect()
                };
                features.push(FeatureRecord {
                    kind: feature.kind,
                    order: order.get(&container.id()).copied(),
                    items,
                });
            }
        }
        Extracted {
            error: read_first(&self.error, root),
            total: read_first(&self.total, root),
            results,
            features,
        }
    }

    fn extract_text(&self, body: &str) -> Extracted {
        let reader = TextReader {
            decode_entities: self.results.decode_entities,
        };
        let results = if let Some(regex) = &self.results.regex {
            let names: Vec<&str> = regex.capture_names().flatten().collect();
            regex
                .captures_iter(body)
                .map(|captures| {
                    let mut record = Record::default();
                    for name in &names {
                        if let Some(found) = captures.name(name) {
                            let value = reader.clean(found.as_str());
                            if !value.is_empty() {
                                record.values.insert((*name).to_owned(), value);
                            }
                        }
                    }
                    record
                })
                .collect()
        } else {
            TextReader::blocks(&self.results.items, body)
                .iter()
                .map(|block| reader.record(&self.results.fields, block))
                .collect()
        };
        let features = self
            .features
            .iter()
            .filter_map(|feature| {
                let present = feature.container.is_empty()
                    || !TextReader::blocks(&feature.container, body).is_empty();
                if !present {
                    return None;
                }
                let items: Vec<Record> = TextReader::blocks(&feature.items, body)
                    .iter()
                    .map(|block| reader.record(&feature.fields, block))
                    .filter(|item| !item.is_empty())
                    .collect();
                (!feature.container.is_empty() || !items.is_empty()).then_some(FeatureRecord {
                    kind: feature.kind,
                    order: None,
                    items,
                })
            })
            .collect();
        Extracted {
            error: reader.first(&self.error, body),
            total: reader.first(&self.total, body),
            results,
            features,
        }
    }
}

/// Regular-expression reads of a text body.
struct TextReader {
    decode_entities: bool,
}

impl TextReader {
    fn clean(&self, text: &str) -> String {
        let text = crate::text::squash(text);
        if self.decode_entities {
            decode_entities(&text)
        } else {
            text
        }
    }

    fn first(&self, probes: &[Probe], haystack: &str) -> Option<String> {
        probes
            .iter()
            .find_map(|probe| probe.regex().and_then(|regex| regex_field(regex, haystack)))
            .map(|text| self.clean(&text))
            .filter(|text| !text.is_empty())
    }

    fn blocks(probes: &[Probe], haystack: &str) -> Vec<String> {
        probes
            .iter()
            .filter_map(Probe::regex)
            .map(|regex| {
                regex
                    .find_iter(haystack)
                    .map(|found| found.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .find(|found| !found.is_empty())
            .unwrap_or_default()
    }

    fn record(&self, fields: &[Field], block: &str) -> Record {
        let mut record = Record::default();
        for field in fields {
            if field.list {
                let values = field
                    .probes
                    .iter()
                    .filter_map(Probe::regex)
                    .map(|regex| {
                        regex
                            .captures_iter(block)
                            .filter_map(|captures| {
                                captures
                                    .name("v")
                                    .or_else(|| captures.get(1))
                                    .or_else(|| captures.get(0))
                                    .map(|found| self.clean(found.as_str()))
                            })
                            .filter(|value| !value.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .find(|values| !values.is_empty())
                    .unwrap_or_default();
                record.lists.insert(field.name.clone(), values);
            } else if let Some(text) = self.first(&field.probes, block) {
                record.values.insert(field.name.clone(), text);
            }
        }
        record
    }
}

/// Elements of the first probe that selects any inside `scope`.
fn first_elements<'a>(probes: &[Probe], scope: ElementRef<'a>) -> Vec<ElementRef<'a>> {
    probes
        .iter()
        .map(|probe| probe.elements(scope))
        .find(|elements| !elements.is_empty())
        .unwrap_or_default()
}

/// Blocks of an HTML feature: the first container alternative that yields blocks whose
/// opening text contains an anchor phrase (when anchors are set), keeping only the outermost
/// of nested matches.
fn feature_blocks<'a>(feature: &CompiledFeature, root: ElementRef<'a>) -> Vec<ElementRef<'a>> {
    for probe in &feature.container {
        let mut accepted: Vec<ElementRef<'a>> = Vec::new();
        let mut accepted_ids: HashSet<NodeId> = HashSet::new();
        for candidate in probe.elements(root) {
            if candidate
                .ancestors()
                .any(|ancestor| accepted_ids.contains(&ancestor.id()))
            {
                continue;
            }
            if !feature.anchors.is_empty() {
                let head = crate::text::squash(&readable_text(candidate))
                    .chars()
                    .take(400)
                    .collect::<String>()
                    .to_lowercase();
                if !feature.anchors.iter().any(|anchor| head.contains(anchor)) {
                    continue;
                }
            }
            accepted_ids.insert(candidate.id());
            accepted.push(candidate);
        }
        if !accepted.is_empty() {
            return accepted;
        }
    }
    Vec::new()
}

/// Text a reader sees in an element: style sheets, scripts and templates left out (Bing
/// starts its answer blocks with a `<style>` element).
fn readable_text(element: ElementRef<'_>) -> String {
    let mut text = String::new();
    for node in element.descendants() {
        let Some(chunk) = node.value().as_text() else {
            continue;
        };
        let hidden = node
            .ancestors()
            .take_while(|ancestor| ancestor.id() != element.id())
            .any(|ancestor| {
                ancestor.value().as_element().is_some_and(|tag| {
                    matches!(tag.name(), "style" | "script" | "noscript" | "template")
                })
            });
        if !hidden {
            text.push_str(chunk);
        }
    }
    text
}

impl Record {
    fn is_empty(&self) -> bool {
        self.values.is_empty() && self.lists.values().all(Vec::is_empty)
    }
}

/// JSON nodes, with a single array node expanded into its elements (so `$.organic` and
/// `$.organic[*]` select the same items).
fn flatten(nodes: Vec<&Value>) -> Vec<Value> {
    match nodes.as_slice() {
        [Value::Array(items)] => items.clone(),
        _ => nodes.into_iter().cloned().collect(),
    }
}

/// Text of an HTML fragment: entities decoded, tags dropped, whitespace collapsed.
fn decode_entities(fragment: &str) -> String {
    let parsed = Html::parse_fragment(fragment);
    crate::text::squash(&parsed.root_element().text().collect::<String>())
}

fn kind_in<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FeatureKind, D::Error> {
    let name = String::deserialize(deserializer)?;
    name.parse().map_err(serde::de::Error::custom)
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes fields by reference
fn kind_out<S: Serializer>(kind: &FeatureKind, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(kind.as_str())
}

#[cfg(test)]
mod tests;
