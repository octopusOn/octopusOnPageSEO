fn links(html: &str) -> Vec<super::super::Link> {
    super::super::super::parse_page(html, &url::Url::parse("https://example.com/").unwrap(), &[])
        .links
}

#[test]
fn names_svg_links_without_treating_graphics_alone_as_descriptive_text() {
    let rows = links(
        r"<a href='/label' aria-label='WhatsApp'><svg aria-hidden='true'><path d=''/></svg></a>
        <a href='/title'><svg><title>Support</title><path d=''/></svg></a>
        <a href='/bare'><svg><path d=''/></svg></a>
        <a href='/decorative'><svg aria-hidden='true'><title>Hidden name</title></svg></a>",
    );
    assert_eq!(
        rows.iter().map(|r| r.anchor.as_str()).collect::<Vec<_>>(),
        ["WhatsApp", "Support", "", ""]
    );
    assert!(rows.iter().all(|r| r.name.as_ref().unwrap().has_svg));
    assert_eq!(rows[0].name.as_ref().unwrap().visible_text, "");
    assert_eq!(rows[0].name.as_ref().unwrap().source, "aria-label");
}

#[test]
fn respects_ordered_label_references_and_safe_fallbacks_without_following_cycles() {
    let rows = links(
        r"<span id='second' hidden>support</span><span id='first'>Contact</span>
        <a href='/ids' aria-labelledby='first second' aria-label='Fallback'>Visible</a>
        <a href='/missing' aria-labelledby='absent' aria-label='  WhatsApp  '><svg/></a>
        <a href='/cycle' id='cycle' aria-labelledby='cycle'>Cycle content</a>
        <a href='/title' title='Download brochure'><svg aria-hidden='true'/></a>
        <a href='/svg' aria-labelledby='svg-name'><svg><title id='svg-name'>SVG label</title></svg></a>",
    );
    assert_eq!(
        rows.iter().map(|r| r.anchor.as_str()).collect::<Vec<_>>(),
        [
            "Contact support",
            "WhatsApp",
            "Cycle content",
            "Download brochure",
            "SVG label"
        ]
    );
    assert_eq!(rows[0].name.as_ref().unwrap().visible_text, "Visible");
    assert_eq!(rows[3].name.as_ref().unwrap().source, "title");
}

#[test]
fn uses_all_image_alts_and_excludes_inert_or_hidden_metadata_from_names() {
    let rows = links(
        r"<a href='/images'><img alt=''><img alt='Company logo'></a>
        <a href='/hidden'><span hidden>Hidden</span><span aria-hidden='true'>Decoration</span><span style='display:none'>Invisible</span><template>Inert</template><svg><desc>Not a name</desc></svg></a>
        <a href='/mixed'>Visit <img alt='Nexus'> now</a>
        <area href='/map' alt='Office location'>",
    );
    assert_eq!(
        rows.iter().map(|r| r.anchor.as_str()).collect::<Vec<_>>(),
        ["Company logo", "", "Visit Nexus now", "Office location"]
    );
    assert!(rows[0].name.as_ref().unwrap().has_image);
}
