//! Local search read from data shaped like DataForSEO's documented examples.

use serde_json::Value;

use crate::Market;
use crate::local::*;

fn result(name: &str) -> Vec<Value> {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let answer: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn ornek() -> Business {
    Business {
        title: "Örnek Kahve Kadıköy".to_owned(),
        cid: Some("222222222222222222".to_owned()),
        lat: 40.9901,
        lng: 29.0291,
        ..Business::default()
    }
}

#[test]
fn a_grid_spreads_evenly_around_the_business() {
    let points = grid(40.9901, 29.0291, 5, 2.0).unwrap();
    assert_eq!(points.len(), 25);
    // Row by row from the north; the centre is the business.
    let centre = points[12];
    assert_eq!((centre.row, centre.col), (2, 2));
    assert!((centre.lat - 40.9901).abs() < 1e-9 && (centre.lng - 29.0291).abs() < 1e-9);
    let (north, south) = (points[2], points[22]);
    assert!(north.lat > centre.lat && south.lat < centre.lat);
    // Two kilometres north and east: about 0.018° of latitude, more of longitude at 41°N.
    assert!((north.lat - centre.lat - 2.0 / 111.32).abs() < 1e-6);
    let east = points[14];
    let degrees = east.lng - centre.lng;
    assert!(
        degrees > 2.0 / 111.32
            && (degrees * 111.32 * 40.9901_f64.to_radians().cos() - 2.0).abs() < 1e-4
    );
    assert!(grid(40.0, 29.0, 4, 2.0).is_err());
    assert!(grid(40.0, 29.0, 5, 0.1).is_err());
    assert!(grid(89.0, 29.0, 5, 2.0).is_err());
    assert_eq!(zoom_for(0.5), 16);
    assert_eq!(zoom_for(3.0), 15);
    assert_eq!(zoom_for(10.0), 13);
}

#[test]
fn a_point_is_asked_from_its_coordinates() {
    let point = GridPoint {
        row: 0,
        col: 0,
        lat: 41.012_345_678,
        lng: 28.98,
    };
    let task = maps_task("  kahve   kadıköy ", &point, 15, "TR").unwrap();
    assert_eq!(task["keyword"], "kahve kadıköy");
    assert_eq!(task["location_coordinate"], "41.0123457,28.98,15z");
    assert_eq!(task["language_code"], "tr");
    assert_eq!(task["depth"], 20);
    let queued = queued_maps_task("kahve", &point, 15, "tr", "k|0|3").unwrap();
    assert_eq!(queued["priority"], 1);
    assert_eq!(queued["tag"], "k|0|3");
    assert!(maps_task(" ", &point, 15, "tr").is_err());
    // Forty-nine points: about three cents through the queue.
    assert!((maps_estimate(49, true) - 0.0294).abs() < 1e-9);
    assert!((maps_estimate(49, false) - 0.098).abs() < 1e-9);
}

#[test]
fn a_maps_page_finds_the_business_and_the_first_three() {
    let reading = maps_from_result(&result("maps_grid"), &ornek());
    assert_eq!(reading.position, Some(2));
    let titles: Vec<&str> = reading
        .top
        .iter()
        .map(|place| place.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Rakip Kahve", "Örnek Kahve Kadıköy", "Köşe Kahvecisi"]
    );
    assert_eq!(reading.top[0].votes, Some(1820));
    // Without its Google id, the website or the name finds it.
    let by_site = Business {
        cid: None,
        domain: Some("ornekkahve.com".to_owned()),
        ..ornek()
    };
    assert_eq!(
        maps_from_result(&result("maps_grid"), &by_site).position,
        Some(2)
    );
    let by_name = Business {
        cid: None,
        title: "ÖRNEK KAHVE kadıköy".to_owned(),
        ..ornek()
    };
    assert_eq!(
        maps_from_result(&result("maps_grid"), &by_name).position,
        Some(2)
    );
    let other = Business {
        cid: Some("999".to_owned()),
        ..ornek()
    };
    assert_eq!(
        maps_from_result(&result("maps_grid"), &other).position,
        None
    );
}

#[test]
fn a_grid_gives_its_average_rank_and_local_share() {
    let summary = summarize(&[Some(1), Some(2), Some(5), None]);
    assert_eq!(summary.points, 4);
    assert_eq!(summary.found, 3);
    assert_eq!(summary.top3, 2);
    assert_eq!(summary.arp, Some(2.7));
    // (1 + 2 + 5 + 21) / 4
    assert_eq!(summary.atrp, Some(7.3));
    assert_eq!(summary.solv, 50.0);
    assert_eq!(summarize(&[]), GridSummary::default());
    assert_eq!(summarize(&[None]).arp, None);
}

#[test]
fn a_profile_reads_hours_ratings_and_topics() {
    let profile = profile_from_result(&result("business_info")).unwrap();
    assert_eq!(profile.business.title, "Örnek Kahve Kadıköy");
    assert_eq!(profile.business.cid.as_deref(), Some("222222222222222222"));
    assert_eq!(profile.business.domain.as_deref(), Some("ornekkahve.com"));
    assert_eq!(profile.business.rating, Some(4.5));
    assert_eq!(profile.business.votes, Some(640));
    assert_eq!(profile.rating_distribution["5"], 450);
    assert_eq!(profile.hours[0].day, "monday");
    assert_eq!(profile.hours[0].hours, ["08:00–22:30"]);
    assert_eq!(profile.hours[5].hours.len(), 0, "closed on Saturday");
    assert_eq!(profile.hours[6].hours, ["09:00–22:00"]);
    assert_eq!(profile.status.as_deref(), Some("opened"));
    assert_eq!(profile.topics[0], ("filtre kahve".to_owned(), 41));
    assert!(profile.attributes.contains(&"has_takeout".to_owned()));
    assert_eq!(profile.additional_categories.len(), 2);
    assert_eq!(profile_from_result(&[]), None);

    let task = business_task("cid:222222222222222222", &Market::new("tr", "tr")).unwrap();
    assert_eq!(task["keyword"], "cid:222222222222222222");
    assert_eq!(task["location_code"], 2792);
}

#[test]
fn listings_and_reviews_are_read() {
    let task = listings_task(
        &["Kafe".to_owned(), " ".to_owned()],
        40.9901,
        29.0291,
        2.4,
        50,
    );
    assert_eq!(task["categories"], serde_json::json!(["Kafe"]));
    assert_eq!(task["location_coordinate"], "40.9901,29.0291,3");
    assert_eq!(task["order_by"][0], "rating.votes_count,desc");
    let (businesses, total) = listings_from_result(&result("business_listings"));
    assert_eq!(total, Some(214));
    assert_eq!(businesses.len(), 3);
    assert_eq!(businesses[0].title, "Zincir Coffee");
    assert!(businesses[2].is(&ornek()));
    assert!((listings_estimate(50) - 0.025).abs() < 1e-9);

    let task = reviews_task(&ornek(), &Market::new("tr", "tr"), 15, "reviews|1").unwrap();
    assert_eq!(task["cid"], "222222222222222222");
    assert_eq!(task["depth"], 20);
    assert_eq!(task["sort_by"], "newest");
    assert_eq!(task["priority"], 1);
    let by_name = Business {
        cid: None,
        place_id: None,
        ..ornek()
    };
    let task = reviews_task(&by_name, &Market::new("tr", "tr"), 10, "r").unwrap();
    assert_eq!(task["keyword"], "Örnek Kahve Kadıköy");
    assert!((reviews_estimate(15) - 0.0015).abs() < 1e-9);
    assert_eq!(review_depth(1000), 500);

    let page = reviews_from_result(&result("google_reviews"));
    assert_eq!(page.rating, Some(4.5));
    assert_eq!(page.votes, Some(640));
    assert_eq!(page.reviews.len(), 3);
    let first = &page.reviews[0];
    assert_eq!(first.rating, Some(5));
    assert_eq!(first.author.as_deref(), Some("Ayşe K."));
    assert_eq!(first.published_at.as_deref(), Some("2026-10-21T12:30:00Z"));
    assert_eq!(first.photos, 1);
    assert!(first.local_guide);
    let answered = &page.reviews[1];
    assert_eq!(answered.rating, Some(2));
    assert_eq!(
        answered.answered_at.as_deref(),
        Some("2026-10-17T09:00:00Z")
    );
    assert_eq!(page.reviews[2].text, None);
}
