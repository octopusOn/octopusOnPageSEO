//! Link texts that say nothing about the target page.

/// Generic anchor texts in English, Turkish, German, Spanish and French (lowercase).
pub const GENERIC_ANCHORS: &[&str] = &[
    // English
    "click here",
    "click",
    "here",
    "read more",
    "more",
    "learn more",
    "see more",
    "find out more",
    "more info",
    "more information",
    "continue",
    "continue reading",
    "details",
    "this",
    "this page",
    "link",
    "this link",
    "go",
    "website",
    // Turkish
    "tıklayın",
    "tıkla",
    "buraya tıklayın",
    "buraya tıkla",
    "burada",
    "buraya",
    "devamı",
    "devamını oku",
    "devamını okuyun",
    "devamı için tıklayın",
    "daha fazla",
    "daha fazlası",
    "detaylar",
    "detay",
    "detaylı bilgi",
    "incele",
    "tümü",
    "bağlantı",
    "bu sayfa",
    // German
    "hier",
    "hier klicken",
    "mehr",
    "weiterlesen",
    "mehr erfahren",
    "mehr lesen",
    // Spanish
    "aquí",
    "haga clic aquí",
    "clic aquí",
    "leer más",
    "más",
    "más información",
    "ver más",
    // French
    "ici",
    "cliquez ici",
    "lire la suite",
    "en savoir plus",
    "plus",
];

/// Whether `text` is a generic anchor text, ignoring case (Turkish dotted and dotless i
/// alike), surrounding punctuation and arrows.
pub fn is_generic_anchor(text: &str) -> bool {
    let folded = fold(text);
    !folded.is_empty() && GENERIC_ANCHORS.iter().any(|anchor| fold(anchor) == folded)
}

/// Lowercase words separated by single spaces, `ı` and `i` treated alike.
fn fold(text: &str) -> String {
    let trimmed = text.trim_matches(|c: char| !c.is_alphanumeric());
    let lower: String = trimmed
        .to_lowercase()
        .chars()
        .filter(|&c| c != '\u{307}')
        .map(|c| if c == 'ı' { 'i' } else { c })
        .collect();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_generic_texts() {
        assert!(is_generic_anchor("Read more »"));
        assert!(is_generic_anchor("  Devamını  oku… "));
        assert!(is_generic_anchor("BURAYA TIKLAYIN"));
        assert!(!is_generic_anchor("Server log analysis guide"));
        assert!(!is_generic_anchor(""));
        assert!(is_generic_anchor("İNCELE"));
        for anchor in GENERIC_ANCHORS {
            assert_eq!(anchor.trim(), *anchor);
            assert_eq!(anchor.to_lowercase(), *anchor, "list entries are lowercase");
        }
    }
}
