use super::*;

fn parse(html: &str) -> PageFacts {
    let url = Url::parse("https://example.com/blog/post?x=1").unwrap();
    parse_page(html, &url, &["googlebot"])
}

#[test]
fn head_elements() {
    let facts = parse(
        r#"<!doctype html><html lang="tr-TR"><head>
        <meta charset="utf-8">
        <title>  İlk
          başlık </title>
        <meta name="description" content=" Açıklama ">
        <meta name="ROBOTS" content="noindex, follow">
        <meta name="googlebot" content="nosnippet">
        <meta name="bingbot" content="nofollow">
        <meta name="viewport" content="width=device-width">
        <link rel="canonical" href="/blog/post">
        <link rel="alternate" hreflang="en" href="https://example.com/en/post">
        <link rel="alternate" hreflang="x-default" href="/">
        <link rel="next" href="?page=2"><link rel="prev" href="?page=0">
        <link rel="amphtml" href="/amp/post">
        <meta property="og:title" content="OG">
        <meta property="og:image" content="/img/og.png">
        <meta name="twitter:card" content="summary">
        </head><body><svg><title>icon</title></svg></body></html>"#,
    );
    assert_eq!(facts.titles, ["İlk başlık"]);
    assert_eq!(facts.title(), Some("İlk başlık"));
    assert_eq!(facts.meta_description(), Some("Açıklama"));
    assert_eq!(facts.lang.as_deref(), Some("tr-TR"));
    assert_eq!(facts.charset.as_deref(), Some("utf-8"));
    assert_eq!(facts.meta_robots, ["noindex, follow", "nosnippet"]);
    assert!(facts.directives.noindex && facts.directives.nosnippet);
    assert!(
        !facts.directives.nofollow,
        "bingbot rules do not apply to googlebot"
    );
    assert_eq!(facts.canonicals, ["https://example.com/blog/post"]);
    assert_eq!(
        facts.hreflang,
        [
            Hreflang {
                lang: "en".into(),
                url: "https://example.com/en/post".into()
            },
            Hreflang {
                lang: "x-default".into(),
                url: "https://example.com/".into()
            },
        ]
    );
    assert_eq!(
        facts.rel_next.as_deref(),
        Some("https://example.com/blog/post?page=2")
    );
    assert_eq!(
        facts.rel_prev.as_deref(),
        Some("https://example.com/blog/post?page=0")
    );
    assert_eq!(
        facts.amp_html.as_deref(),
        Some("https://example.com/amp/post")
    );
    assert_eq!(facts.social.og_title.as_deref(), Some("OG"));
    assert_eq!(
        facts.social.og_image.as_deref(),
        Some("https://example.com/img/og.png")
    );
    assert_eq!(facts.social.twitter_card.as_deref(), Some("summary"));
    assert_eq!(facts.viewport.as_deref(), Some("width=device-width"));
}

#[test]
fn canonical_requires_a_non_inert_head_element() {
    let facts = parse(
        "<html><head><link rel='canonical' href='/valid'>\
         <template><link rel='canonical' href='/inert'></template></head>\
         <body><link rel='canonical' href='/body'>\
         <link rel='stylesheet' href='/body.css'></body></html>",
    );
    assert_eq!(facts.canonicals, ["https://example.com/valid"]);
    assert!(
        facts
            .links
            .iter()
            .any(|link| link.url.ends_with("/body.css"))
    );
    for html in [
        "<html><head></head><body><link rel='canonical' href='/body'></body></html>",
        "<html><head><div>Closes the head</div><link rel='canonical' href='/misplaced'></head></html>",
    ] {
        assert_eq!(parse(html).canonicals, Vec::<String>::new());
    }
}

#[test]
fn canonical_ignores_alternate_version_attributes_even_when_empty() {
    for attribute in ["hreflang", "lang", "media", "type"] {
        for value in ["", "en"] {
            let facts = parse(&format!(
                "<head><link rel='canonical alternate' {attribute}='{value}' href='/invalid'>\
                 <link rel='canonical' href='/valid'></head>"
            ));
            assert_eq!(
                facts.canonicals,
                ["https://example.com/valid"],
                "{attribute}={value}"
            );
            if attribute == "hreflang" {
                assert_eq!(facts.hreflang[0].url, "https://example.com/invalid");
            }
        }
    }
}

#[test]
fn canonical_counts_elements_once_and_preserves_separate_declarations() {
    let facts = parse(
        "<head><base href='https://example.com/base/'>\
        <link rel='CANONICAL canonical' href='one'>\
        <link rel='canonical' href='two'></head>",
    );
    assert_eq!(
        facts.canonicals,
        [
            "https://example.com/base/one",
            "https://example.com/base/two"
        ]
    );
}

#[test]
fn hreflang_eligibility_requires_an_active_direct_head_element() {
    let facts = parse(
        "<html><head><link rel='alternate' hreflang='en' href='/head'>\
        <template><link rel='alternate' hreflang='fr' href='/template'></template></head>\
        <body><link rel='alternate' hreflang='de' href='/body'></body></html>",
    );
    assert_eq!(
        facts.hreflang,
        [Hreflang {
            lang: "en".into(),
            url: "https://example.com/head".into()
        }]
    );
    let malformed = parse(
        "<html><head><div>Closes head</div><link rel='alternate' hreflang='en' href='/misplaced'></head></html>",
    );
    assert_eq!(malformed.hreflang, [] as [Hreflang; 0]);
}

#[test]
fn hreflang_eligibility_rejects_media_qualified_alternates_including_empty_values() {
    for media in ["", "all", "screen", "(max-width: 640px)"] {
        let facts = parse(&format!(
            "<head><link rel='alternate' hreflang='en' media='{media}' href='/qualified'>\
            <link rel='alternate' hreflang='fr' href='/valid'></head>"
        ));
        assert_eq!(
            facts.hreflang,
            [Hreflang {
                lang: "fr".into(),
                url: "https://example.com/valid".into()
            }],
            "media={media:?}"
        );
    }
}

#[test]
fn hreflang_eligibility_does_not_remove_independent_stylesheet_references() {
    let facts = parse(
        "<html><head><link rel='alternate stylesheet' hreflang='en' media='screen' href='http://example.com/head.css'></head>\
        <body><link rel='alternate stylesheet' hreflang='fr' href='/body.css'></body></html>",
    );
    assert_eq!(facts.hreflang, [] as [Hreflang; 0]);
    let resources: Vec<_> = facts
        .links
        .iter()
        .filter(|link| link.kind == LinkKind::Stylesheet)
        .map(|link| link.url.as_str())
        .collect();
    assert_eq!(
        resources,
        [
            "http://example.com/head.css",
            "https://example.com/body.css"
        ]
    );
    assert_eq!(facts.mixed_content, ["http://example.com/head.css"]);
}

#[test]
fn hreflang_eligibility_counts_elements_once_and_preserves_raw_values() {
    let facts = parse(
        "<head><base href='/languages/'>\
        <link rel='ALTERNATE alternate' hreflang=' en-GB ' href='english'>\
        <link rel='alternate' hreflang='xx_BAD' href='invalid-code'>\
        <link rel='alternate' hreflang='en-GB' href='english'></head>",
    );
    assert_eq!(
        facts.hreflang,
        [
            Hreflang {
                lang: "en-GB".into(),
                url: "https://example.com/languages/english".into()
            },
            Hreflang {
                lang: "xx_BAD".into(),
                url: "https://example.com/languages/invalid-code".into()
            },
            Hreflang {
                lang: "en-GB".into(),
                url: "https://example.com/languages/english".into()
            },
        ]
    );
}

#[test]
fn multiple_robots_meta_tags_preserve_the_strictest_matching_snippet_limit() {
    for tags in [
        "<meta name='robots' content='max-snippet:0'><meta name='robots' content='max-snippet:-1'>",
        "<meta name='robots' content='max-snippet:-1'><meta name='robots' content='max-snippet:0'>",
        "<meta name='googlebot' content='max-snippet:0'><meta name='robots' content='max-snippet:150'>",
    ] {
        assert_eq!(
            parse(&format!("<head>{tags}</head>"))
                .directives
                .max_snippet,
            Some(0)
        );
    }
    assert_eq!(parse("<head><meta name='robots' content='max-snippet:50'><meta name='bingbot' content='max-snippet:0'></head>").directives.max_snippet, Some(50));
}

#[test]
fn template_metadata_does_not_override_active_page_signals() {
    let facts = parse(
        "<head><template><base href='https://wrong.example/'>\
        <title>Unused title</title><meta name='description' content='Unused description'>\
        <meta name='robots' content='noindex,nofollow,max-snippet:0'>\
        <meta name='viewport' content='unused'><meta charset='windows-1252'>\
        <meta http-equiv='refresh' content='0;url=/unused'></template>\
        <base href='/active/'><title>Actual title</title>\
        <meta name='description' content='Actual description'>\
        <meta name='robots' content='max-snippet:50'></head>\
        <body><a href='live'>Live link</a></body>",
    );
    assert_eq!(
        facts.base_href.as_deref(),
        Some("https://example.com/active/")
    );
    assert_eq!(facts.titles, ["Actual title"]);
    assert_eq!(facts.meta_descriptions, ["Actual description"]);
    assert!(!facts.directives.noindex && !facts.directives.nofollow);
    assert_eq!(facts.directives.max_snippet, Some(50));
    assert_eq!(facts.viewport, None);
    assert_eq!(facts.charset, None);
    assert_eq!(facts.meta_refresh, None);
    assert_eq!(
        facts.hyperlinks().next().unwrap().url,
        "https://example.com/active/live"
    );
    assert!(
        parse("<body><meta name='robots' content='noindex'></body>")
            .directives
            .noindex
    );
}

#[test]
fn template_links_resources_and_text_do_not_leak_into_active_facts() {
    let facts = parse(
        "<body><template><a href='/unused-link'>Unused</a>\
        <area href='/unused-area'><img src='http://example.com/unused.png'>\
        <script src='http://example.com/unused.js'></script>\
        <link rel='stylesheet' href='http://example.com/unused.css'>\
        <iframe src='http://example.com/unused-frame'></iframe>\
        <video src='http://example.com/unused-video'></video>\
        <form action='http://example.com/unused-form'></form>\
        <template><h2>Unused nested heading</h2></template></template>\
        <h1>Real <template>Unused heading text</template>Heading</h1>\
        <a href='/live'>Actual <template>Unused anchor text</template>link</a>\
        <a href='/image-link'><template><img src='/unused-alt.png' alt='Unused alt'></template>\
        <img src='/real.png' alt='Actual alt'></a></body>",
    );
    assert_eq!(
        facts
            .headings
            .iter()
            .map(|heading| heading.text.as_str())
            .collect::<Vec<_>>(),
        ["Real Heading"]
    );
    assert_eq!(
        facts
            .hyperlinks()
            .map(|link| link.anchor.as_str())
            .collect::<Vec<_>>(),
        ["Actual link", "Actual alt"]
    );
    assert_eq!(
        facts
            .links
            .iter()
            .map(|link| link.url.as_str())
            .collect::<Vec<_>>(),
        [
            "https://example.com/live",
            "https://example.com/image-link",
            "https://example.com/real.png"
        ]
    );
    assert_eq!(facts.images.len(), 1);
    assert_eq!(facts.insecure_forms, 0);
    assert_eq!(facts.mixed_content, Vec::<String>::new());
}

#[test]
fn template_structured_data_is_excluded_from_document_analysis() {
    let facts = parse(
        r#"<body><template><script type='application/ld+json'>invalid JSON</script>
        <div itemscope itemtype='https://schema.org/Product'></div>
        <template><div typeof='schema:Recipe'></div></template></template>
        <script type='application/ld+json'>{"@type":"Article"}</script></body>"#,
    );
    assert_eq!(facts.structured_data.len(), 1);
    assert_eq!(facts.structured_data[0].types, ["Article"]);
    assert!(facts.structured_data[0].valid);
}

#[test]
fn links_resolve_against_base_and_skip_other_schemes() {
    let facts = parse(
        r##"<head><base href="https://cdn.example.com/root/"></head><body>
        <a href="page#section">Sayfa  <b>bir</b></a>
        <a href="https://other.org/" rel="nofollow ugc">Dış</a>
        <a href="/sponsor" rel="sponsored"><img src="logo.png" alt=" Logo "></a>
        <a href="mailto:a@b.c">mail</a><a href="javascript:void(0)">js</a>
        <a href="#top">top</a><a href="">empty</a><a href="tel:123">tel</a>
        <map><area href="/area" alt="Bölge"></map>
        </body>"##,
    );
    assert_eq!(
        facts.base_href.as_deref(),
        Some("https://cdn.example.com/root/")
    );
    let links: Vec<_> = facts.hyperlinks().collect();
    assert_eq!(links.len(), 6);
    assert_eq!(links[0].url, "https://cdn.example.com/root/page");
    assert_eq!(links[0].anchor, "Sayfa bir");
    assert!(links[1].nofollow && links[1].ugc && !links[1].sponsored);
    assert_eq!(
        links[2].anchor, "Logo",
        "an image link's text is the image alt"
    );
    assert!(links[2].sponsored);
    assert_eq!(
        (links[5].kind, links[5].anchor.as_str()),
        (LinkKind::Area, "Bölge")
    );
    assert_eq!(links[3].url, "https://cdn.example.com/root/");
    assert_eq!(links[4].url, "https://cdn.example.com/root/");
}

#[test]
fn rejected_base_schemes_fall_back_without_losing_relative_seo_references() {
    for href in [
        "data:text/html,unused",
        "JaVaScRiPt:alert(1)",
        "  data:,unused  ",
    ] {
        let facts = parse(&format!(
            "<head><base href='{href}'><base href='/wrong/'>\
            <link rel='canonical' href='post'><link rel='alternate' hreflang='en' href='en'>\
            <link rel='stylesheet' href='style.css'></head><body><a href='child'>Child</a>\
            <img src='image.png'><script src='script.js'></script></body>"
        ));
        assert_eq!(
            facts.canonicals,
            ["https://example.com/blog/post"],
            "{href}"
        );
        assert_eq!(facts.hreflang[0].url, "https://example.com/blog/en");
        let targets: Vec<_> = facts.links.iter().map(|link| link.url.as_str()).collect();
        for target in ["child", "image.png", "script.js", "style.css"] {
            assert!(
                targets.contains(&format!("https://example.com/blog/{target}").as_str()),
                "{href}: {targets:?}"
            );
        }
        assert!(
            facts
                .base_href
                .as_deref()
                .is_some_and(|base| base.starts_with("data:") || base.starts_with("javascript:")),
            "preserve the resolved declaration"
        );
    }
}

#[test]
fn only_the_first_active_base_with_href_controls_resolution_even_when_rejected() {
    for href in ["http://[", "", "data:,unused"] {
        let facts = parse(&format!(
            "<head><base target='_blank'><template><base href='/inert/'></template>\
            <base href='{href}'><base href='/wrong/'></head><body><a href='child'>Child</a></body>"
        ));
        assert_eq!(
            facts.hyperlinks().next().unwrap().url,
            "https://example.com/blog/child",
            "{href}"
        );
    }
    let facts = parse(
        "<base href='ftp://example.com/files/'><a href='child'>FTP</a><a href='https://example.com/web'>Web</a>",
    );
    assert_eq!(
        facts
            .hyperlinks()
            .map(|link| link.url.as_str())
            .collect::<Vec<_>>(),
        ["https://example.com/web"],
        "other non-HTTP bases are not fallback candidates"
    );
}

#[test]
fn fragment_and_empty_hyperlinks_can_target_a_different_base_document() {
    let facts = parse(
        "<head><base href='/target?part=2&amp;token=a%23b#base'></head><body>\
        <a href='#one' rel='nofollow ugc'>One</a><a href='  '>Empty</a>\
        <map><area href=' #two ' alt='Area' rel='sponsored'></map></body>",
    );
    let links: Vec<_> = facts.hyperlinks().collect();
    assert_eq!(links.len(), 3);
    assert!(
        links
            .iter()
            .all(|link| link.url == "https://example.com/target?part=2&token=a%23b")
    );
    assert!(links[0].nofollow && links[0].ugc);
    assert_eq!(links[1].anchor, "Empty");
    assert!(links[2].sponsored && links[2].kind == LinkKind::Area);
    let external =
        parse("<base href='https://other.example/page#base'><a href='#section'>External</a>");
    assert_eq!(
        external.hyperlinks().next().unwrap().url,
        "https://other.example/page"
    );
}

#[test]
fn same_document_fragments_and_empty_links_do_not_create_self_inlinks() {
    for base in [
        "",
        "<base href='/blog/post?x=1#base'>",
        "<base href='https://example.com:443/blog/post?x=1'>",
        "<base href='javascript:unused'>",
    ] {
        let facts = parse(&format!(
            "<head>{base}</head><body><a href='#one'>One</a><a href=' '>Empty</a><map><area href='#two'></map></body>"
        ));
        assert_eq!(facts.hyperlinks().count(), 0, "{base}");
    }
    let facts = parse("<base href='/blog/post?part=2'><a href='#section'>Query document</a>");
    assert_eq!(
        facts.hyperlinks().next().unwrap().url,
        "https://example.com/blog/post?part=2"
    );
}

#[test]
fn headings_keep_document_order() {
    let facts = parse("<h2>B</h2><h1> A </h1><h1></h1><h3>C</h3>");
    let outline: Vec<_> = facts
        .headings
        .iter()
        .map(|h| (h.level, h.text.as_str()))
        .collect();
    assert_eq!(outline, [(2, "B"), (1, "A"), (1, ""), (3, "C")]);
    assert_eq!(facts.headings_of(1).collect::<Vec<_>>(), ["A", ""]);
}

#[test]
fn images_and_resources() {
    let facts = parse(
        r#"<head><link rel="stylesheet" href="http://example.com/a.css">
        <script src="/app.js"></script></head><body>
        <img src="/a.jpg" alt="A" width="10" height="10" loading="lazy">
        <img src="data:image/png;base64,xx" data-src="/lazy.jpg">
        <img src="http://example.com/b.jpg">
        <iframe src="https://video.example/embed"></iframe>
        <video src="/v.mp4"></video>
        <form action="http://example.com/login"></form><form action="/search"></form>
        </body>"#,
    );
    assert_eq!(facts.images.len(), 3);
    let first = &facts.images[0];
    assert_eq!(first.alt.as_deref(), Some("A"));
    assert!(first.has_dimensions && first.lazy);
    assert_eq!(facts.images[1].url, "https://example.com/lazy.jpg");
    assert_eq!(facts.images[1].alt, None);
    let kinds: Vec<_> = facts.links.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        [
            LinkKind::Stylesheet,
            LinkKind::Image,
            LinkKind::Image,
            LinkKind::Image,
            LinkKind::Script,
            LinkKind::Iframe,
            LinkKind::Media,
        ]
    );
    assert_eq!(
        facts.mixed_content,
        ["http://example.com/a.css", "http://example.com/b.jpg"]
    );
    assert_eq!(facts.insecure_forms, 1);
}

#[test]
fn responsive_images_include_valid_candidates_and_preserve_url_commas() {
    let facts = parse(
        "<base href='/assets/'><img src='small.jpg' alt=' Hero ' width='10' height='20' loading='LAZY' \
         srcset='small.jpg 1x, large.jpg 2x, crop,a.jpg 3x, \
         /_next/image?url=%2Fhero.jpg&amp;w=1280&amp;q=75 1280w, \
         data:image/png;base64,AAAA 4x, bad.jpg 0w, rejected.jpg 1x 2x'>",
    );
    let urls: Vec<_> = facts
        .images
        .iter()
        .map(|image| image.url.as_str())
        .collect();
    assert_eq!(
        urls,
        [
            "https://example.com/assets/small.jpg",
            "https://example.com/assets/large.jpg",
            "https://example.com/assets/crop,a.jpg",
            "https://example.com/_next/image?url=%2Fhero.jpg&w=1280&q=75",
        ]
    );
    assert!(
        facts
            .images
            .iter()
            .all(|image| image.alt.as_deref() == Some("Hero")
                && image.has_dimensions
                && image.lazy)
    );
    assert_eq!(facts.links.len(), 4);
    assert!(
        facts
            .links
            .iter()
            .all(|link| link.kind == LinkKind::Image && link.anchor == "Hero")
    );
}

#[test]
fn picture_sources_belong_to_their_following_direct_image() {
    let facts = parse(
        "<source srcset='/orphan.png 1x'><picture>\
         <source srcset='/wide.avif 1x, /large.avif 2x' media='(min-width: 900px)' type='image/avif' width='40' height='20'>\
         <source srcset='http://cdn.example/narrow.webp 1x' height='10'>\
         <template><source srcset='/inert.webp 1x'><img src='/inert.png'></template>\
         <img src='/fallback.png' width='20' height='10'>\
         <source srcset='/after.webp 1x'></picture>\
         <img src='/fallback.png' alt='Different occurrence'>",
    );
    let urls: Vec<_> = facts
        .images
        .iter()
        .map(|image| image.url.as_str())
        .collect();
    assert_eq!(
        urls,
        [
            "https://example.com/fallback.png",
            "https://example.com/wide.avif",
            "https://example.com/large.avif",
            "http://cdn.example/narrow.webp",
            "https://example.com/fallback.png"
        ]
    );
    assert!(facts.images[1].has_dimensions && facts.images[2].has_dimensions);
    assert!(!facts.images[3].has_dimensions);
    assert_eq!(facts.images[0].alt, None);
    assert_eq!(facts.images[4].alt.as_deref(), Some("Different occurrence"));
    assert_eq!(facts.mixed_content, ["http://cdn.example/narrow.webp"]);
}

#[test]
fn responsive_images_without_src_and_repeated_urls_keep_owner_attributes() {
    let facts = parse(
        "<picture><source srcset='/shared.jpg 2x' width='20'>\
         <img srcset='/shared.jpg 1x, /unique.jpg 2x' width='20' height='10' alt=''></picture>\
         <img src='data:image/png;base64,AAAA' data-src='/lazy.jpg' srcset='/lazy-large.jpg 2x' loading='lazy'>",
    );
    assert_eq!(facts.images.len(), 4);
    assert_eq!(facts.images[0].url, "https://example.com/shared.jpg");
    assert_eq!(facts.images[0].alt.as_deref(), Some(""));
    assert!(!facts.images[0].has_dimensions);
    assert!(facts.images[1].has_dimensions);
    assert!(facts.images[2].lazy && facts.images[3].lazy);
}

#[test]
fn structured_data_blocks() {
    let facts = parse(
        r#"<script type="application/ld+json">
          {"@context":"https://schema.org","@graph":[{"@type":"Organization"},{"@type":["WebPage","FAQPage"]}]}
        </script>
        <script type="application/ld+json">{ broken </script>
        <div itemscope itemtype="https://schema.org/Product">
          <div itemprop="offers" itemscope itemtype="https://schema.org/Offer"></div>
        </div>
        <div vocab="https://schema.org/" typeof="BreadcrumbList"></div>"#,
    );
    let blocks: Vec<_> = facts
        .structured_data
        .iter()
        .map(|b| (b.format, b.types.join(","), b.valid))
        .collect();
    assert_eq!(
        blocks,
        [
            (
                StructuredDataFormat::JsonLd,
                "Organization,WebPage,FAQPage".to_owned(),
                true
            ),
            (StructuredDataFormat::JsonLd, String::new(), false),
            (StructuredDataFormat::Microdata, "Product".to_owned(), true),
            (
                StructuredDataFormat::Rdfa,
                "BreadcrumbList".to_owned(),
                true
            ),
        ]
    );
}

#[test]
fn visible_text_skips_hidden_elements() {
    let facts = parse(
        "<head><title>Not counted</title><style>p{}</style></head><body>\
         <p>Bir iki<b>üç</b></p><div>dört</div><script>var x = 1;</script>\
         <noscript>hidden</noscript><template>t</template></body>",
    );
    // "iki<b>üç</b>" is one word, the div starts a new one.
    assert_eq!(facts.word_count, 3);
    let same = parse("<p>Bir   ikiüç</p><p>dört</p>");
    assert_eq!(facts.text_hash, same.text_hash);
    assert_ne!(facts.text_hash, parse("<p>başka</p>").text_hash);
    assert_eq!(parse("<body></body>").text_hash, 0);
}

#[test]
fn meta_refresh_and_http_equiv_charset() {
    let facts = parse(
        r#"<meta http-equiv="Content-Type" content="text/html; charset=windows-1254">
        <meta http-equiv="refresh" content="5; URL='/yeni'">"#,
    );
    assert_eq!(facts.charset.as_deref(), Some("windows-1254"));
    assert_eq!(
        facts.meta_refresh,
        Some(MetaRefresh {
            delay_seconds: 5,
            url: Some("https://example.com/yeni".into())
        })
    );
    let reload = parse(r#"<meta http-equiv="refresh" content="30">"#);
    assert_eq!(
        reload.meta_refresh,
        Some(MetaRefresh {
            delay_seconds: 30,
            url: None
        })
    );
}
