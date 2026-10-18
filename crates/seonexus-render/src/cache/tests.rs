use super::*;
use serde_json::json;

fn response() -> Value {
    json!({"url":"https://example.com/app-abc.js","status":200,"headers":{
        "Content-Type":"application/javascript; charset=utf-8",
        "Cache-Control":"public, max-age=3600, immutable",
        "Date":OffsetDateTime::now_utc().format(&Rfc2822).unwrap(),
        "Content-Encoding":"gzip", "Content-Length":"22", "Vary":"Accept-Encoding"
    }})
}

#[test]
fn intercepted_headers_keep_the_same_eligibility_and_reject_ambiguous_duplicates() {
    let response = response();
    let headers: Vec<_> = response["headers"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect();
    let mut params = json!({"responseStatusCode":200,"request":{"url":response["url"]},"responseHeaders":headers});
    assert!(Response::intercepted(&params).is_some());
    params["responseHeaders"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"Cache-Control","value":"private"}));
    assert!(Response::intercepted(&params).is_none());
    params["responseHeaders"].as_array_mut().unwrap().pop();
    params["responseHeaders"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"Set-Cookie","value":"session=personal"}));
    assert!(Response::intercepted(&params).is_none());
    params["responseHeaders"].as_array_mut().unwrap().pop();
    params["responseStatusCode"] = json!(503);
    assert!(Response::intercepted(&params).is_none());
}

#[test]
fn only_fresh_explicitly_public_immutable_utf8_assets_are_cached() {
    let resource = Response::new(&response())
        .unwrap()
        .with_body(b"window.ok=true;".to_vec())
        .unwrap();
    assert!(
        !resource
            .headers
            .iter()
            .any(|(n, _)| matches!(n.as_str(), "content-length" | "content-encoding"))
    );
    let cache = ResourceCache::default();
    cache.insert(resource);
    assert_eq!(
        cache.get("https://example.com/app-abc.js").unwrap().body,
        b"window.ok=true;"
    );
    for (name, value) in [
        ("Cache-Control", "public, max-age=3600"),
        ("Cache-Control", "private, max-age=3600, immutable"),
        ("Cache-Control", "public, no-store, max-age=3600, immutable"),
        (
            "Cache-Control",
            "public, no-cache=\"etag\", max-age=3600, immutable",
        ),
        ("Cache-Control", "public, max-age=0, immutable"),
        (
            "Cache-Control",
            "public, max-age=3600, max-age=20, immutable",
        ),
        ("Cache-Control", "public, max-age=invalid, immutable"),
        ("Vary", "Cookie"),
        ("Vary", "Origin"),
        ("Vary", "*"),
        ("Set-Cookie", "session=personal"),
        ("Age", "3600"),
        ("Age", "invalid"),
        ("Date", "Tue, 01 Jan 2019 00:00:00 GMT"),
        ("Content-Type", "application/javascript; charset=iso-8859-1"),
        ("Content-Type", "text/html"),
    ] {
        let mut value_response = response();
        value_response["headers"][name] = json!(value);
        assert!(Response::new(&value_response).is_none(), "{name}: {value}");
    }
    for status in [204, 301, 304, 404, 500] {
        let mut value = response();
        value["status"] = json!(status);
        assert!(Response::new(&value).is_none());
    }
    assert!(
        Response::new(&response())
            .unwrap()
            .with_body(vec![b'a'; ENTRY_BYTES + 1])
            .is_none()
    );
    assert!(
        Response::new(&response())
            .unwrap()
            .with_body(vec![255])
            .is_none()
    );
    assert!(
        Response::new(&response())
            .unwrap()
            .with_body(b"@charset \"iso-8859-1\"; body{}".to_vec())
            .is_none()
    );
}

#[test]
fn request_identity_excludes_navigation_api_cross_origin_credentials_and_revalidation() {
    let request = json!({"resourceType":"Script","request":{"url":"https://example.com/app-abc.js","method":"GET","headers":{"Accept":"*/*"}}});
    assert!(reusable_request(&request, "https://example.com/page"));
    for kind in ["Document", "XHR", "Fetch", "Image", "Font"] {
        let mut value = request.clone();
        value["resourceType"] = json!(kind);
        assert!(!reusable_request(&value, "https://example.com/page"));
    }
    for name in [
        "Cookie",
        "Authorization",
        "Range",
        "Cache-Control",
        "Pragma",
    ] {
        let mut value = request.clone();
        value["request"]["headers"][name] = json!("sensitive");
        assert!(!reusable_request(&value, "https://example.com/page"));
    }
    let mut value = request.clone();
    value["request"]["method"] = json!("POST");
    assert!(!reusable_request(&value, "https://example.com/page"));
    for origin in [
        "http://example.com/",
        "https://example.com:444/",
        "https://other.example/",
    ] {
        assert!(!reusable_request(&request, origin));
    }
    let mut value = request.clone();
    value["request"]["url"] = json!("https://user:password@example.com/app.js");
    assert!(!reusable_request(&value, "https://example.com/"));
}

#[test]
fn cache_is_bounded_expiring_and_uses_least_recently_used_eviction() {
    let cache = ResourceCache::default();
    for id in 0..MAX_ENTRIES {
        let mut resource = Response::new(&response())
            .unwrap()
            .with_body(vec![b'a'; 1])
            .unwrap();
        resource.url = id.to_string();
        cache.insert(resource);
    }
    cache.get("0").unwrap();
    let mut resource = Response::new(&response())
        .unwrap()
        .with_body(vec![b'a'; 1])
        .unwrap();
    resource.url = "new".into();
    cache.insert(resource);
    assert!(cache.get("1").is_none());
    assert!(cache.get("0").is_some());
    let mut resource = Response::new(&response())
        .unwrap()
        .with_body(vec![b'a'; 1])
        .unwrap();
    resource.url = "expired".into();
    resource.expires = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    cache.insert(resource);
    assert!(cache.get("expired").is_none());
    for id in 0..40 {
        let mut resource = Response::new(&response())
            .unwrap()
            .with_body(vec![b'a'; ENTRY_BYTES])
            .unwrap();
        resource.url = format!("big{id}");
        cache.insert(resource);
    }
    let entries = cache.0.lock().unwrap();
    assert!(entries.bytes <= CACHE_BYTES);
    assert!(entries.items.len() < 40);
}
