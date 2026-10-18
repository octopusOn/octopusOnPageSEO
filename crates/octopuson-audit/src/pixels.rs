//! Estimated rendering width of text in the font search results use.

/// Advance widths of ASCII 32–126 in Arial/Helvetica, in thousandths of an em.
const ASCII_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ' '–'/'
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // '0'–'9'
    278, 278, 584, 584, 584, 556, 1015, // ':'–'@'
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // 'A'–'M'
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // 'N'–'Z'
    278, 278, 278, 469, 556, 333, // '['–'`'
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // 'a'–'m'
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // 'n'–'z'
    334, 260, 334, 584, // '{'–'~'
];

/// Width of an average lowercase letter, used for characters not in the table.
const DEFAULT_WIDTH: u16 = 556;

/// Pixels `text` takes in Arial at `font_px` (rounded). Accented Latin letters count as
/// their base letter; East Asian characters as a full em.
pub fn pixel_width(text: &str, font_px: f64) -> u32 {
    let units: u64 = text.chars().map(|c| u64::from(char_width(c))).sum();
    #[allow(clippy::cast_precision_loss, reason = "text widths are far below 2^52")]
    let pixels = units as f64 / 1000.0 * font_px;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded, non-negative and clamped to u32"
    )]
    let rounded = pixels.round().clamp(0.0, f64::from(u32::MAX)) as u32;
    rounded
}

fn char_width(c: char) -> u16 {
    if let Some(index) = (c as usize).checked_sub(32)
        && let Some(&width) = ASCII_WIDTHS.get(index)
    {
        return width;
    }
    if let Some(base) = base_letter(c) {
        return char_width(base);
    }
    match c {
        '\u{a0}' => 278,
        '‘' | '’' | '‚' => 222,
        '“' | '”' | '„' => 333,
        '–' | '€' => 556,
        '•' => 350,
        '©' | '®' => 737,
        'ß' => 611,
        '—'
        | '…'
        | '™'
        | '\u{1100}'..='\u{115f}'
        | '\u{2e80}'..='\u{a4cf}'
        | '\u{ac00}'..='\u{d7a3}'
        | '\u{f900}'..='\u{faff}'
        | '\u{fe30}'..='\u{fe4f}'
        | '\u{ff00}'..='\u{ff60}' => 1000,
        _ => DEFAULT_WIDTH,
    }
}

/// The unaccented letter of common Latin letters with diacritics.
fn base_letter(c: char) -> Option<char> {
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
        'ğ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ı' => 'i',
        'ł' => 'l',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ő' => 'o',
        'ř' => 'r',
        'ş' | 'ś' | 'š' | 'ș' => 's',
        'ţ' | 'ť' | 'ț' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'A',
        'Ç' | 'Ć' | 'Č' => 'C',
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ę' | 'Ě' => 'E',
        'Ğ' => 'G',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'İ' => 'I',
        'Ł' => 'L',
        'Ñ' | 'Ń' | 'Ň' => 'N',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ő' => 'O',
        'Ş' | 'Ś' | 'Š' | 'Ș' => 'S',
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' | 'Ů' | 'Ű' => 'U',
        'Ý' => 'Y',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        _ => return None,
    };
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_like_arial() {
        // "Hello" = 722 + 556 + 222 + 222 + 556 = 2278 thousandths of an em.
        assert_eq!(pixel_width("Hello", 20.0), 46);
        assert_eq!(pixel_width("", 20.0), 0);
        // Turkish letters count as their base letters.
        assert_eq!(pixel_width("ğüşıöç", 20.0), pixel_width("gusioc", 20.0));
        // Wide capitals take more room than narrow lowercase letters.
        assert!(pixel_width("WWWWW", 20.0) > pixel_width("iiiii", 20.0) * 4);
        assert_eq!(pixel_width("日本", 10.0), 20);
    }
}
