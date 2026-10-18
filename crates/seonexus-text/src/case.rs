//! Case folding and tokenizing.

use crate::Language;

/// Lowercases with the language's rules: Turkish and Azerbaijani map `I` to `ı` and `İ` to
/// `i`; other languages use Unicode lowercase.
pub fn lower(text: &str, language: Language) -> String {
    if language.dotted_i() {
        let mut out = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                'I' => out.push('ı'),
                'İ' => out.push('i'),
                other => out.extend(other.to_lowercase()),
            }
        }
        out
    } else {
        text.to_lowercase()
    }
}

/// Removes diacritics of Latin letters common in Turkish and European text, after
/// lowercasing: `ç ğ ı ö ş ü â î û é è à ä ß` → `c g i o s u a i u e e a a ss`.
/// Matching on folded text treats `turk kahvesi` and `türk kahvesi` as the same phrase.
pub fn fold(lowered: &str) -> String {
    let mut out = String::with_capacity(lowered.len());
    for ch in lowered.chars() {
        match ch {
            'ç' => out.push('c'),
            'ğ' => out.push('g'),
            'ı' | 'î' | 'í' | 'ì' | 'ï' => out.push('i'),
            'ö' | 'ô' | 'ó' | 'ò' | 'õ' => out.push('o'),
            'ş' => out.push('s'),
            'ü' | 'û' | 'ú' | 'ù' => out.push('u'),
            'â' | 'á' | 'à' | 'ä' | 'ã' | 'å' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'ñ' => out.push('n'),
            'ß' => out.push_str("ss"),
            // Combining dot above left by Unicode lowercasing of İ in non-Turkish text.
            '\u{307}' => {}
            other => out.push(other),
        }
    }
    out
}

/// Words of `text`, lowercased with the language's rules. A word is a run of letters and
/// digits; a suffix after an apostrophe is dropped (`Türkiye'nin` → `türkiye`,
/// `Google’da` → `google`), and hyphenated words stay whole (`e-ticaret`).
pub fn tokens(text: &str, language: Language) -> Vec<String> {
    let lowered = lower(text, language);
    let mut words = Vec::new();
    let mut current = String::new();
    let mut skipping_suffix = false;
    let chars: Vec<char> = lowered.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_alphanumeric() {
            if !skipping_suffix {
                current.push(ch);
            }
            continue;
        }
        let next_is_word = chars
            .get(index + 1)
            .is_some_and(|next| next.is_alphanumeric());
        match ch {
            '\'' | '’'
                if !current.is_empty() && next_is_word && language.drops_apostrophe_suffix() =>
            {
                skipping_suffix = true;
            }
            '-' if !current.is_empty() && next_is_word && !skipping_suffix => current.push('-'),
            _ => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                skipping_suffix = false;
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_with_turkish_rules() {
        assert_eq!(lower("IŞIK İzmir", Language::Turkish), "ışık izmir");
        assert_eq!(lower("IKEA", Language::English), "ikea");
    }

    #[test]
    fn folds_diacritics() {
        assert_eq!(fold("türk kahvesi çeşitleri"), "turk kahvesi cesitleri");
        assert_eq!(fold(&"İstanbul".to_lowercase()), "istanbul");
        assert_eq!(fold("straße café"), "strasse cafe");
    }

    #[test]
    fn splits_words_and_drops_apostrophe_suffixes() {
        assert_eq!(
            tokens(
                "Türkiye'nin en iyi e-ticaret siteleri: Google’da 1. sıra!",
                Language::Turkish
            ),
            [
                "türkiye",
                "en",
                "iyi",
                "e-ticaret",
                "siteleri",
                "google",
                "1",
                "sıra"
            ]
        );
        assert_eq!(
            tokens("don't stop", Language::English),
            ["don", "t", "stop"]
        );
        assert_eq!(tokens("  ", Language::Turkish), Vec::<String>::new());
    }

    proptest::proptest! {
        #[test]
        fn tokens_are_words_and_folding_is_idempotent(text in "\\PC{0,80}") {
            for word in tokens(&text, Language::Turkish) {
                proptest::prop_assert!(!word.is_empty());
                proptest::prop_assert!(!word.chars().any(char::is_whitespace));
            }
            let lowered = lower(&text, Language::Turkish);
            let once = fold(&lowered);
            proptest::prop_assert_eq!(fold(&once), once);
        }
    }
}
