//! URL discovery using the HTML srcset tokenizer and descriptor validity rules.
//! Commas inside URLs or parenthesized descriptors are not candidate separators.

fn space(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

pub(super) fn urls(input: &str) -> Vec<&str> {
    let bytes = input.as_bytes();
    let mut position = 0;
    let mut result = Vec::new();
    while position < bytes.len() {
        while position < bytes.len() && (space(bytes[position]) || bytes[position] == b',') {
            position += 1;
        }
        let start = position;
        while position < bytes.len() && !space(bytes[position]) {
            position += 1;
        }
        let url = &input[start..position];
        if url.is_empty() {
            break;
        }
        if url.ends_with(',') {
            result.push(url.trim_end_matches(','));
            continue;
        }
        let start = position;
        let mut parentheses = false;
        while position < bytes.len() {
            match bytes[position] {
                b',' if !parentheses => break,
                b'(' => parentheses = true,
                b')' => parentheses = false,
                _ => {}
            }
            position += 1;
        }
        if valid_descriptors(&input[start..position]) {
            result.push(url);
        }
        // Consume the delimiter; EOF is harmless here.
        position += 1;
    }
    result
}

fn valid_descriptors(input: &str) -> bool {
    let (mut width, mut density, mut height) = (false, false, false);
    for descriptor in input
        .split(|ch: char| ch.is_ascii() && space(ch as u8))
        .filter(|part| !part.is_empty())
    {
        if let Some(value) = descriptor.strip_suffix('w') {
            if width || density || !positive_integer(value) {
                return false;
            }
            width = true;
        } else if let Some(value) = descriptor.strip_suffix('x') {
            if width || density || height || !nonnegative_float(value) {
                return false;
            }
            density = true;
        } else if let Some(value) = descriptor.strip_suffix('h') {
            if height || density || !positive_integer(value) {
                return false;
            }
            height = true;
        } else {
            return false;
        }
    }
    !height || width
}

fn digits(input: &str) -> bool {
    !input.is_empty() && input.bytes().all(|byte| byte.is_ascii_digit())
}

fn positive_integer(input: &str) -> bool {
    digits(input) && input.bytes().any(|byte| byte != b'0')
}

fn nonnegative_float(input: &str) -> bool {
    // Rust accepts +1, NaN and 1.; HTML's valid floating-point syntax does not.
    let unsigned = input.strip_prefix('-').unwrap_or(input);
    let mantissa = if let Some((mantissa, exponent)) = unsigned.split_once(['e', 'E']) {
        if !digits(exponent.strip_prefix(['-', '+']).unwrap_or(exponent)) {
            return false;
        }
        mantissa
    } else {
        unsigned
    };
    let valid = if let Some((integer, fraction)) = mantissa.split_once('.') {
        (integer.is_empty() || digits(integer)) && digits(fraction)
    } else {
        digits(mantissa)
    };
    valid
        && input
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commas_data_urls_and_parentheses_follow_html_tokenization() {
        assert_eq!(
            urls(
                ",, a,b.png 1x, data:image/png;base64,AAAA 2x, c.png, d.png,, e.png bad(a,b), good.png 2x"
            ),
            [
                "a,b.png",
                "data:image/png;base64,AAAA",
                "c.png",
                "d.png",
                "good.png"
            ]
        );
        assert_eq!(urls("a.png,b.png"), ["a.png,b.png"]);
        assert_eq!(urls("a.png bad(a,b, other.png 2x"), Vec::<&str>::new());
    }

    #[test]
    fn descriptors_reject_conflicts_and_recover_at_the_next_candidate() {
        for descriptor in [
            "0w",
            "-1w",
            "+1w",
            "1.0w",
            "1w 2w",
            "1x 2x",
            "1w 2x",
            "1h",
            "2h 3h 1w",
            "1h 2x",
            "1w 1q",
            "1X",
            "1x\u{b}2x",
        ] {
            assert_eq!(
                urls(&format!("bad.png {descriptor}, good.png 2x")),
                ["good.png"],
                "{descriptor}"
            );
        }
        assert_eq!(
            urls("a.png 1h 2w, b.png 9999999999999999999999999999w, c.png\t2x, d.png\u{c}4x"),
            ["a.png", "b.png", "c.png", "d.png"]
        );
    }

    #[test]
    fn density_uses_html_floating_point_syntax_and_finite_nonnegative_values() {
        for density in ["0", "-0", ".5", "1.25", "1e2", "1E+2", "1e-2", "2"] {
            assert_eq!(
                urls(&format!("image.png {density}x")),
                ["image.png"],
                "{density}"
            );
        }
        for density in [
            "-1", "+1", "1.", ".", "NaN", "inf", "1e309", "1e", "1e+", "1e2e3", "1..2",
        ] {
            assert_eq!(
                urls(&format!("image.png {density}x")),
                Vec::<&str>::new(),
                "{density}"
            );
        }
    }
}
