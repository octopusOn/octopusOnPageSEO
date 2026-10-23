//! Local search: a business's Google profile, the businesses of its kind around it, where it
//! shows on Google Maps from every point of a grid around it, and its reviews.
//!
//! A grid check reads one Maps result page per point, looked at from the point's coordinates:
//! live, or through the standard queue for less than a third of the price. Reviews are read
//! through the queue only (DataForSEO has no live reviews).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::prices::{
    BUSINESS_INFO_LIVE, LISTINGS_ROW, LISTINGS_TASK, REVIEWS_TEN, SERP_LIVE_PAGE,
    SERP_STANDARD_PAGE,
};
use crate::{DfsError, Market, count, items_of, number, target_domain};

/// Endpoint of a business's Google profile.
pub const BUSINESS_INFO: &str = "business_data/google/my_business_info/live";
/// Endpoint of the business listings around a place.
pub const LISTINGS: &str = "business_data/business_listings/search/live";
/// Endpoint of a Maps result page asked live.
pub const MAPS_LIVE: &str = "serp/google/maps/live/advanced";
/// Endpoint taking Maps result pages for the standard queue.
pub const MAPS_POST: &str = "serp/google/maps/task_post";
/// Endpoint taking reviews tasks (the standard queue).
pub const REVIEWS_POST: &str = "business_data/google/reviews/task_post";

/// Where a queued Maps page is read, free, by the id the queue gave it.
pub fn maps_get(id: &str) -> String {
    format!("serp/google/maps/task_get/advanced/{id}")
}

/// Where queued reviews are read, free, by the id the queue gave them.
pub fn reviews_get(id: &str) -> String {
    format!("business_data/google/reviews/task_get/{id}")
}

/// Points a side of a grid may have.
pub const GRID_SIZES: &[u32] = &[3, 5, 7, 9];

/// Smallest and largest distance from a grid's centre to its edge, kilometres.
pub const RADII_KM: std::ops::RangeInclusive<f64> = 0.2..=50.0;

/// Results read on each point's Maps page: the business's place and the first three.
pub const MAPS_DEPTH: u32 = 20;

/// Businesses kept from each point: the first three.
pub const TOP: usize = 3;

/// The place counted for a point where the business is not among the results (average total
/// rank).
pub const NOT_FOUND: u16 = 21;

/// Businesses of the kind read around the business.
pub const LISTING_ROWS: u32 = 50;

/// Most reviews read at once.
pub const MAX_REVIEWS: u32 = 500;

/// Longest keyword DataForSEO takes.
const MAX_KEYWORD: usize = 700;

/// Kilometres in a degree of latitude.
const KM_PER_DEGREE: f64 = 111.32;

fn text(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// A coordinate with at most seven decimals and no trailing zeros.
fn decimal(value: f64) -> String {
    let text = format!("{value:.7}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn round7(value: f64) -> f64 {
    (value * 1e7).round() / 1e7
}

/// One decimal.
fn tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// DataForSEO's `yyyy-mm-dd hh:mm:ss +00:00` as RFC 3339.
fn rfc3339(text: &str) -> Option<String> {
    let (date, time) = text.get(..19)?.split_once(' ')?;
    (date.len() == 10 && time.len() == 8).then(|| format!("{date}T{time}Z"))
}

/// A keyword with its spacing cleaned.
fn clean_keyword(keyword: &str) -> Result<String, DfsError> {
    let keyword = keyword.split_whitespace().collect::<Vec<_>>().join(" ");
    if keyword.is_empty() {
        return Err(DfsError::Config("keyword: enter a keyword".to_owned()));
    }
    if keyword.chars().count() > MAX_KEYWORD {
        return Err(DfsError::Config(
            "keyword: at most 700 characters".to_owned(),
        ));
    }
    Ok(keyword)
}

// ------------------------------------------------------------------------------------- Grid

/// A point of a grid: its row (north to south), column (west to east) and coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridPoint {
    /// Row, from the north.
    pub row: u32,
    /// Column, from the west.
    pub col: u32,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
}

/// The points of a `size` × `size` grid around a centre, reaching `radius_km` from it towards
/// each side, row by row from the north.
///
/// # Errors
/// [`DfsError::Config`] for a size other than 3, 5, 7 or 9, a radius outside
/// [`RADII_KM`], or a centre that is not on the map.
pub fn grid(lat: f64, lng: f64, size: u32, radius_km: f64) -> Result<Vec<GridPoint>, DfsError> {
    if !GRID_SIZES.contains(&size) {
        return Err(DfsError::Config(
            "grid: 3, 5, 7 or 9 points a side".to_owned(),
        ));
    }
    if !RADII_KM.contains(&radius_km) {
        return Err(DfsError::Config(
            "radius: 0.2–50 km from the centre".to_owned(),
        ));
    }
    if !(-85.0..=85.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return Err(DfsError::Config(
            "centre: the business has no place on the map".to_owned(),
        ));
    }
    let half = f64::from(size - 1) / 2.0;
    let step = radius_km / half;
    let mut points = Vec::with_capacity((size * size) as usize);
    for row in 0..size {
        for col in 0..size {
            let north = (half - f64::from(row)) * step;
            let east = (f64::from(col) - half) * step;
            points.push(GridPoint {
                row,
                col,
                lat: round7(lat + north / KM_PER_DEGREE),
                lng: round7(lng + east / (KM_PER_DEGREE * lat.to_radians().cos())),
            });
        }
    }
    Ok(points)
}

/// The Maps zoom a grid is looked at with: closer for a smaller area.
pub fn zoom_for(radius_km: f64) -> u8 {
    match radius_km {
        r if r <= 1.0 => 16,
        r if r <= 3.0 => 15,
        r if r <= 7.0 => 14,
        r if r <= 15.0 => 13,
        _ => 12,
    }
}

/// The task of one point's Maps result page.
///
/// # Errors
/// [`DfsError::Config`] for an empty or too long keyword.
pub fn maps_task(
    keyword: &str,
    point: &GridPoint,
    zoom: u8,
    language: &str,
) -> Result<Value, DfsError> {
    Ok(json!({
        "keyword": clean_keyword(keyword)?,
        "location_coordinate": format!("{},{},{zoom}z", decimal(point.lat), decimal(point.lng)),
        "language_code": language.trim().to_ascii_lowercase(),
        "depth": MAPS_DEPTH,
        "device": "desktop",
    }))
}

/// The task of one point's Maps result page for the standard queue, named by `tag`.
///
/// # Errors
/// As [`maps_task`].
pub fn queued_maps_task(
    keyword: &str,
    point: &GridPoint,
    zoom: u8,
    language: &str,
    tag: &str,
) -> Result<Value, DfsError> {
    let mut task = maps_task(keyword, point, zoom, language)?;
    task["priority"] = json!(1);
    task["tag"] = json!(tag.chars().take(255).collect::<String>());
    Ok(task)
}

/// The list price of a grid's Maps pages, live or through the standard queue, US dollars.
#[allow(clippy::cast_precision_loss)] // point counts
pub fn maps_estimate(points: usize, queued: bool) -> f64 {
    let page = if queued {
        SERP_STANDARD_PAGE
    } else {
        SERP_LIVE_PAGE
    };
    page * points as f64
}

// --------------------------------------------------------------------------------- Business

/// A business: what identifies it on Google, where it is and how it is rated.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Business {
    /// Its name on Google.
    pub title: String,
    /// Google's id of the place.
    #[serde(default)]
    pub cid: Option<String>,
    /// Google's place id.
    #[serde(default)]
    pub place_id: Option<String>,
    /// Its main category.
    #[serde(default)]
    pub category: Option<String>,
    /// Its address.
    #[serde(default)]
    pub address: Option<String>,
    /// Its website's host, without `www.`.
    #[serde(default)]
    pub domain: Option<String>,
    /// Its website.
    #[serde(default)]
    pub url: Option<String>,
    /// Its phone number.
    #[serde(default)]
    pub phone: Option<String>,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// Average stars.
    #[serde(default)]
    pub rating: Option<f64>,
    /// Reviews counted in the rating.
    #[serde(default)]
    pub votes: Option<u64>,
    /// The owner has claimed the profile.
    #[serde(default)]
    pub is_claimed: Option<bool>,
}

/// Lowercase without accents or Turkish dotless letters, for comparing names.
fn folded(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ı' | 'î' | 'í' => 'i',
            'ş' => 's',
            'ğ' => 'g',
            'ü' | 'û' | 'ú' => 'u',
            'ö' => 'o',
            'ç' => 'c',
            'â' | 'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ê' => 'e',
            other => other,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

impl Business {
    /// Whether `other` is the same business: the same Google id, else the same website, else
    /// the same name.
    pub fn is(&self, other: &Self) -> bool {
        let same = |left: &Option<String>, right: &Option<String>| {
            left.as_ref()
                .zip(right.as_ref())
                .map(|(left, right)| left == right)
        };
        if let Some(found) = same(&self.cid, &other.cid) {
            return found;
        }
        if let Some(found) = same(&self.place_id, &other.place_id) {
            return found;
        }
        let site = |business: &Self| {
            business
                .domain
                .as_deref()
                .map(target_domain)
                .filter(|domain| !domain.is_empty())
        };
        if let (Some(own), Some(theirs)) = (site(self), site(other))
            && own == theirs
        {
            return true;
        }
        !self.title.is_empty() && folded(&self.title) == folded(&other.title)
    }
}

/// A business from an item of a Maps page, a profile or a listing.
fn business_of(item: &Value) -> Option<Business> {
    Some(Business {
        title: text(item, "title")?,
        cid: text(item, "cid"),
        place_id: text(item, "place_id"),
        category: text(item, "category"),
        address: text(item, "address"),
        domain: text(item, "domain").map(|domain| target_domain(&domain)),
        url: text(item, "url"),
        phone: text(item, "phone"),
        lat: item.get("latitude").and_then(number)?,
        lng: item.get("longitude").and_then(number)?,
        rating: item.pointer("/rating/value").and_then(number),
        votes: item.pointer("/rating/votes_count").and_then(count),
        is_claimed: item.get("is_claimed").and_then(Value::as_bool),
    })
}

/// A business on a point's Maps page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapPlace {
    /// Place on the page.
    pub position: u16,
    /// Its name.
    pub title: String,
    /// Google's id of the place.
    #[serde(default)]
    pub cid: Option<String>,
    /// Average stars.
    #[serde(default)]
    pub rating: Option<f64>,
    /// Reviews counted in the rating.
    #[serde(default)]
    pub votes: Option<u64>,
}

/// What a point's Maps page showed: the business's place and the first three.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PointReading {
    /// The business's place; `None` when it is not among the results read.
    pub position: Option<u16>,
    /// The first three businesses.
    pub top: Vec<MapPlace>,
}

/// Reads a point's Maps page for `business`.
pub fn maps_from_result(result: &[Value], business: &Business) -> PointReading {
    let mut reading = PointReading::default();
    let mut places: Vec<(u16, Business)> = items_of(result)
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("maps_search"))
        .filter_map(|item| {
            let position = item
                .get("rank_group")
                .and_then(count)
                .and_then(|n| u16::try_from(n).ok())?;
            Some((position, business_of(item)?))
        })
        .collect();
    places.sort_by_key(|(position, _)| *position);
    reading.position = places
        .iter()
        .find(|(_, place)| business.is(place))
        .map(|(position, _)| *position);
    reading.top = places
        .into_iter()
        .take(TOP)
        .map(|(position, place)| MapPlace {
            position,
            title: place.title,
            cid: place.cid,
            rating: place.rating,
            votes: place.votes,
        })
        .collect();
    reading
}

/// A grid's figures over its points.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GridSummary {
    /// Points read.
    pub points: u32,
    /// Points where the business shows among the first 20.
    pub found: u32,
    /// Points where it shows among the first three (the local pack).
    pub top3: u32,
    /// Average rank where it shows.
    pub arp: Option<f64>,
    /// Average rank over every point, [`NOT_FOUND`] where it does not show.
    pub atrp: Option<f64>,
    /// Share of local voice: the share of points where it is in the first three, percent.
    pub solv: f64,
}

/// The figures of a grid's readings (`None` where the business did not show).
#[allow(clippy::cast_precision_loss)] // point counts
pub fn summarize(positions: &[Option<u16>]) -> GridSummary {
    let found: Vec<f64> = positions.iter().flatten().map(|p| f64::from(*p)).collect();
    let points = positions.len();
    let top3 = positions.iter().flatten().filter(|p| **p <= 3).count();
    GridSummary {
        points: u32::try_from(points).unwrap_or(u32::MAX),
        found: u32::try_from(found.len()).unwrap_or(u32::MAX),
        top3: u32::try_from(top3).unwrap_or(u32::MAX),
        arp: (!found.is_empty()).then(|| tenth(found.iter().sum::<f64>() / found.len() as f64)),
        atrp: (points > 0).then(|| {
            let total: f64 = positions
                .iter()
                .map(|p| f64::from(p.unwrap_or(NOT_FOUND)))
                .sum();
            tenth(total / points as f64)
        }),
        solv: if points == 0 {
            0.0
        } else {
            tenth(top3 as f64 / points as f64 * 100.0)
        },
    }
}

// ---------------------------------------------------------------------------------- Profile

/// The opening hours of a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayHours {
    /// `monday` … `sunday`.
    pub day: String,
    /// `08:00–22:30` spans; empty when closed.
    pub hours: Vec<String>,
}

/// A business's Google profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// The business.
    pub business: Business,
    /// What the business says about itself.
    pub description: Option<String>,
    /// Its other categories.
    pub additional_categories: Vec<String>,
    /// Reviews by stars, `"1"` … `"5"`.
    pub rating_distribution: BTreeMap<String, u64>,
    /// Photos on the profile.
    pub total_photos: Option<u64>,
    /// The profile's main photo.
    pub main_image: Option<String>,
    /// Price level (`₺₺`).
    pub price_level: Option<String>,
    /// Monday first.
    pub hours: Vec<DayHours>,
    /// `opened`, `closed`, `temporarily_closed` or `closed_forever`.
    pub status: Option<String>,
    /// What it offers (`has_takeout` …).
    pub attributes: Vec<String>,
    /// What reviews talk about, most mentioned first.
    pub topics: Vec<(String, u64)>,
    /// The search page, to look at it by hand.
    pub check_url: Option<String>,
}

/// The task of a business's profile: by its name (with its district or city) or by its Google
/// id (`cid:…`).
///
/// # Errors
/// [`DfsError::Config`] for an empty query or an unknown country.
pub fn business_task(query: &str, market: &Market) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keyword"] = json!(clean_keyword(query)?);
    Ok(task)
}

/// The list price of a business's profile, US dollars.
pub const fn business_estimate() -> f64 {
    BUSINESS_INFO_LIVE
}

const DAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

fn clock(value: &Value) -> Option<String> {
    let hour = value.get("hour").and_then(count)?;
    let minute = value.get("minute").and_then(count).unwrap_or_default();
    Some(format!("{hour:02}:{minute:02}"))
}

/// Reads a profile answer.
pub fn profile_from_result(result: &[Value]) -> Option<Profile> {
    let item = items_of(result)
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("google_business_info"))?;
    let strings = |value: Option<&Value>| -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let timetable = item.pointer("/work_time/work_hours/timetable");
    let hours = timetable
        .map(|table| {
            DAYS.iter()
                .map(|day| DayHours {
                    day: (*day).to_owned(),
                    hours: table
                        .get(*day)
                        .and_then(Value::as_array)
                        .map(|spans| {
                            spans
                                .iter()
                                .filter_map(|span| {
                                    Some(format!(
                                        "{}–{}",
                                        clock(span.get("open")?)?,
                                        clock(span.get("close")?)?
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut topics: Vec<(String, u64)> = item
        .get("place_topics")
        .and_then(Value::as_object)
        .map(|topics| {
            topics
                .iter()
                .filter_map(|(name, mentions)| Some((name.clone(), count(mentions)?)))
                .collect()
        })
        .unwrap_or_default();
    topics.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let attributes = item
        .pointer("/attributes/available_attributes")
        .and_then(Value::as_object)
        .map(|groups| {
            groups
                .values()
                .flat_map(|list| strings(Some(list)))
                .collect()
        })
        .unwrap_or_default();
    Some(Profile {
        business: business_of(item)?,
        description: text(item, "description"),
        additional_categories: strings(item.get("additional_categories")),
        rating_distribution: item
            .get("rating_distribution")
            .and_then(Value::as_object)
            .map(|stars| {
                stars
                    .iter()
                    .filter_map(|(star, n)| Some((star.clone(), count(n)?)))
                    .collect()
            })
            .unwrap_or_default(),
        total_photos: item.get("total_photos").and_then(count),
        main_image: text(item, "main_image"),
        price_level: text(item, "price_level"),
        hours,
        status: item
            .pointer("/work_time/work_hours/current_status")
            .and_then(Value::as_str)
            .map(str::to_owned),
        attributes,
        topics,
        check_url: result.first().and_then(|first| text(first, "check_url")),
    })
}

// --------------------------------------------------------------------------------- Listings

/// The task of the businesses of `categories` within `radius_km` of a place, most reviewed
/// first.
pub fn listings_task(
    categories: &[String],
    lat: f64,
    lng: f64,
    radius_km: f64,
    limit: u32,
) -> Value {
    let categories: Vec<&String> = categories
        .iter()
        .filter(|category| !category.trim().is_empty())
        .take(10)
        .collect();
    json!({
        "categories": categories,
        "location_coordinate": format!("{},{},{}", decimal(lat), decimal(lng), radius_km.ceil().max(1.0)),
        "order_by": ["rating.votes_count,desc"],
        "limit": limit.clamp(1, 1000),
    })
}

/// Reads a listings answer: the businesses and how many match in all.
pub fn listings_from_result(result: &[Value]) -> (Vec<Business>, Option<u64>) {
    let businesses = items_of(result).iter().filter_map(business_of).collect();
    let total = result
        .first()
        .and_then(|first| first.get("total_count"))
        .and_then(count);
    (businesses, total)
}

/// The list price of a listings search returning `rows` businesses, US dollars.
pub fn listings_estimate(rows: u32) -> f64 {
    LISTINGS_TASK + LISTINGS_ROW * f64::from(rows)
}

// ---------------------------------------------------------------------------------- Reviews

/// A Google review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Review {
    /// Google's id of the review.
    pub review_id: String,
    /// 1–5 stars.
    pub rating: Option<u8>,
    /// What the reviewer wrote.
    pub text: Option<String>,
    /// The reviewer's name.
    pub author: Option<String>,
    /// The reviewer's profile.
    pub author_url: Option<String>,
    /// The review on Google.
    pub url: Option<String>,
    /// RFC 3339.
    pub published_at: Option<String>,
    /// The business's answer.
    pub owner_answer: Option<String>,
    /// RFC 3339.
    pub answered_at: Option<String>,
    /// Photos with it.
    pub photos: u32,
    /// The reviewer is a Local Guide.
    pub local_guide: bool,
}

/// A page of reviews with the business's rating then.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReviewPage {
    /// Newest first.
    pub reviews: Vec<Review>,
    /// The business's average stars.
    pub rating: Option<f64>,
    /// Its reviews in all.
    pub votes: Option<u64>,
}

/// Reviews read in steps of ten, as DataForSEO charges them.
pub fn review_depth(depth: u32) -> u32 {
    depth.clamp(10, MAX_REVIEWS).div_ceil(10) * 10
}

/// The task of a business's newest reviews for the standard queue, named by `tag`.
///
/// # Errors
/// [`DfsError::Config`] for a business without a Google id or name, or an unknown country.
pub fn reviews_task(
    business: &Business,
    market: &Market,
    depth: u32,
    tag: &str,
) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    if let Some(cid) = &business.cid {
        task["cid"] = json!(cid);
    } else if let Some(place_id) = &business.place_id {
        task["place_id"] = json!(place_id);
    } else {
        task["keyword"] = json!(clean_keyword(&business.title)?);
    }
    task["depth"] = json!(review_depth(depth));
    task["sort_by"] = json!("newest");
    task["priority"] = json!(1);
    task["tag"] = json!(tag.chars().take(255).collect::<String>());
    Ok(task)
}

/// The list price of `depth` reviews, US dollars.
pub fn reviews_estimate(depth: u32) -> f64 {
    REVIEWS_TEN * f64::from(review_depth(depth) / 10)
}

/// Reads a reviews answer.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // stars clamped to 1–5
pub fn reviews_from_result(result: &[Value]) -> ReviewPage {
    let first = result.first().unwrap_or(&Value::Null);
    ReviewPage {
        reviews: items_of(result)
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("google_review"))
            .filter_map(|item| {
                Some(Review {
                    review_id: text(item, "review_id")?,
                    rating: item
                        .pointer("/rating/value")
                        .and_then(number)
                        .map(|stars| stars.round().clamp(1.0, 5.0) as u8),
                    text: text(item, "review_text"),
                    author: text(item, "profile_name"),
                    author_url: text(item, "profile_url"),
                    url: text(item, "review_url"),
                    published_at: text(item, "timestamp").and_then(|at| rfc3339(&at)),
                    owner_answer: text(item, "owner_answer"),
                    answered_at: text(item, "owner_timestamp").and_then(|at| rfc3339(&at)),
                    photos: item
                        .get("images")
                        .and_then(Value::as_array)
                        .map_or(0, |images| u32::try_from(images.len()).unwrap_or(u32::MAX)),
                    local_guide: item.get("local_guide").and_then(Value::as_bool) == Some(true),
                })
            })
            .collect(),
        rating: first.pointer("/rating/value").and_then(number),
        votes: first
            .get("reviews_count")
            .and_then(count)
            .or_else(|| first.pointer("/rating/votes_count").and_then(count)),
    }
}
