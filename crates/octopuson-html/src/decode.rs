//! Response bodies to text, honoring the declared character encoding.

use encoding_rs::{Encoding, UTF_8};

mod prescan;

/// How many leading bytes are searched for a `<meta charset>` declaration (as browsers do).
const PRESCAN_BYTES: usize = 1024;

/// Decodes HTML using a BOM, a supported transport charset, then the first-kilobyte HTML
/// encoding prescan (including XML compatibility declarations), in that order. Without a
/// supported declaration the default remains UTF-8. Invalid sequences become U+FFFD.
pub fn decode_html(body: &[u8], content_type: Option<&str>) -> String {
    if let Some((encoding, bom)) = Encoding::for_bom(body) {
        return encoding
            .decode_without_bom_handling(&body[bom..])
            .0
            .into_owned();
    }
    let encoding = content_type
        .and_then(header_encoding)
        .or_else(|| prescan::encoding(&body[..body.len().min(PRESCAN_BYTES)]))
        .unwrap_or(UTF_8);
    encoding.decode_without_bom_handling(body).0.into_owned()
}

fn header_encoding(content_type: &str) -> Option<&'static Encoding> {
    let bytes = content_type.as_bytes();
    let mut position = content_type.find(';')? + 1;
    while position < bytes.len() {
        while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
            position += 1;
        }
        let start = position;
        while bytes
            .get(position)
            .is_some_and(|byte| !matches!(byte, b'=' | b';'))
        {
            position += 1;
        }
        if bytes.get(position) != Some(&b'=') {
            position += 1;
            continue;
        }
        let name = content_type[start..position].trim();
        position += 1;
        while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
            position += 1;
        }
        let label = if bytes.get(position) == Some(&b'"') {
            position += 1;
            let mut label = Vec::new();
            while let Some(&byte) = bytes.get(position) {
                position += 1;
                if byte == b'"' {
                    break;
                }
                if byte == b'\\' {
                    if let Some(&escaped) = bytes.get(position) {
                        label.push(escaped);
                        position += 1;
                    }
                } else {
                    label.push(byte);
                }
            }
            // Ignore trailing junk after a quoted value, up to the parameter delimiter.
            while bytes.get(position).is_some_and(|byte| *byte != b';') {
                position += 1;
            }
            label
        } else {
            let start = position;
            while bytes.get(position).is_some_and(|byte| *byte != b';') {
                position += 1;
            }
            content_type[start..position].trim().as_bytes().to_vec()
        };
        if name.eq_ignore_ascii_case("charset") {
            return Encoding::for_label(&label);
        }
        position += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_charset_wins_over_utf8_default() {
        // "Ağ" in windows-1254 (Turkish): 'ğ' is 0xF0.
        let body = [b'A', 0xF0];
        assert_eq!(
            decode_html(&body, Some("text/html; charset=windows-1254")),
            "Ağ"
        );
        assert_eq!(decode_html("Ağ".as_bytes(), Some("text/html")), "Ağ");
    }

    #[test]
    fn meta_charset_and_bom_are_honored() {
        let mut body = b"<meta charset=\"iso-8859-9\"><p>".to_vec();
        body.push(0xFE); // 'ş' in ISO-8859-9
        assert!(decode_html(&body, None).ends_with('ş'));
        let bom = [0xEF, 0xBB, 0xBF, b'o', b'k'];
        assert_eq!(decode_html(&bom, Some("text/html; charset=latin1")), "ok");
    }

    #[test]
    fn invalid_bytes_are_replaced() {
        assert_eq!(decode_html(&[b'a', 0xFF, b'b'], None), "a\u{FFFD}b");
    }

    #[test]
    fn comments_and_unrelated_charset_text_do_not_override_a_real_declaration() {
        for decoy in [
            "<!-- charset=utf-8 -->",
            "<div data-note='charset=utf-8'></div>",
            "<script>const config='charset=utf-8';</script>",
            "<meta name='description' content='charset=utf-8'>",
        ] {
            let mut body = format!("{decoy}<META CHARSET = 'windows-1254'><title>").into_bytes();
            body.extend_from_slice(b"\xc7a\xf0r\xfd \xdeirketi</title>");
            assert!(
                decode_html(&body, None).contains("Çağrı Şirketi"),
                "{decoy}"
            );
        }
    }

    #[test]
    fn unsupported_header_and_meta_labels_fall_through_to_a_supported_declaration() {
        let body = b"<meta charset=not-an-encoding><meta charset=windows-1254><title>\xc7a\xf0r\xfd</title>";
        assert!(decode_html(body, Some("text/html; charset=not-an-encoding")).contains("Çağrı"));
        assert!(decode_html(body, None).contains("Çağrı"));
    }

    #[test]
    fn prescan_recognizes_labels_after_non_utf8_bytes() {
        let body = b"<!-- \xfe --><meta charset=windows-1254><title>\xc7a\xf0r\xfd</title>";
        assert!(decode_html(body, None).contains("Çağrı"));
    }

    #[test]
    fn http_equiv_requires_a_content_type_pragma_in_either_attribute_order() {
        for declaration in [
            "<meta HTTP-EQUIV='Content-Type' content=\"text/html; CHARSET = 'windows-1254'\">",
            "<meta content='text/html; charset=windows-1254' http-equiv=content-type>",
            "<meta/charset=windows-1254>",
        ] {
            let mut body = declaration.as_bytes().to_vec();
            body.extend_from_slice(b"<title>\xc7a\xf0r\xfd</title>");
            assert!(decode_html(&body, None).contains("Çağrı"), "{declaration}");
        }
        for prefix in [
            "<meta content='text/html; charset=windows-1254'>",
            "<meta http-equiv=refresh content='charset=windows-1254'>",
            "<metaphor charset=windows-1254>",
            "<div data-note=\"<meta charset=windows-1254>\"></div>",
        ] {
            let body = format!("{prefix}<meta charset=utf-8><title>Çağrı</title>");
            assert!(
                decode_html(body.as_bytes(), None).contains("Çağrı"),
                "{prefix}"
            );
        }
    }

    #[test]
    fn first_duplicate_attribute_and_direct_charset_precedence_are_preserved() {
        for declaration in [
            "<meta charset=utf-8 CHARSET=windows-1254>",
            "<meta charset=invalid charset=windows-1254><meta charset=utf-8>",
            "<meta http-equiv=content-type content='charset=windows-1254' charset=utf-8>",
            "<meta charset=utf-8 content='charset=windows-1254' http-equiv=content-type>",
            "<meta charset=invalid content='charset=windows-1254' http-equiv=content-type><meta charset=utf-8>",
        ] {
            let body = format!("{declaration}<title>Çağrı</title>");
            assert!(
                decode_html(body.as_bytes(), None).contains("Çağrı"),
                "{declaration}"
            );
        }
    }

    #[test]
    fn byte_limit_requires_a_complete_tag_and_never_scans_late_declarations() {
        let declaration = b"<meta charset=windows-1254>";
        let mut exact = vec![b' '; PRESCAN_BYTES - declaration.len()];
        exact.extend_from_slice(declaration);
        exact.extend_from_slice(b"<title>\xc7a\xf0r\xfd</title>");
        assert!(decode_html(&exact, None).contains("Çağrı"));
        for padding in [PRESCAN_BYTES - declaration.len() + 1, PRESCAN_BYTES] {
            let mut late = vec![b' '; padding];
            late.extend_from_slice(declaration);
            late.extend_from_slice("<title>Çağrı</title>".as_bytes());
            assert!(decode_html(&late, None).contains("Çağrı"));
        }
    }

    #[test]
    fn meta_utf16_and_user_defined_encodings_use_html_compatible_decoders() {
        for label in ["utf-16", "utf-16le", "utf-16be"] {
            let body = format!("<meta charset={label}><title>Çağrı</title>");
            assert!(decode_html(body.as_bytes(), None).contains("Çağrı"));
        }
        assert!(
            decode_html(b"<meta charset=x-user-defined><title>\x80</title>", None).contains('€')
        );
    }

    #[test]
    fn quoted_http_parameters_do_not_introduce_fake_charset_values() {
        for header in [
            "text/html; note=\"x;charset=utf-8\"; CHARSET=\"windows-1254\"",
            "text/html; note=\"x\\\";charset=utf-8\"; charset=windows-1254",
            "text/html; charset=\"windows\\-1254\"",
            "text/html; note=unquoted\"text; charset=windows-1254",
        ] {
            assert_eq!(
                decode_html(b"\xc7a\xf0r\xfd", Some(header)),
                "Çağrı",
                "{header}"
            );
        }
        let body = b"<meta charset=windows-1254>\xc7a\xf0r\xfd";
        assert!(
            decode_html(body, Some("text/html; charset=invalid; charset=utf-8")).contains("Çağrı")
        );
    }

    #[test]
    fn xml_compatibility_declarations_and_utf16_signatures_are_supported() {
        assert!(
            decode_html(
                b"<?xml version='1.0' encoding='windows-1254'?><title>\xc7a\xf0r\xfd</title>",
                None
            )
            .contains("Çağrı")
        );
        let text = "<?xml version='1.0'?><title>Çağrı</title>";
        for little_endian in [true, false] {
            let body: Vec<_> = text
                .encode_utf16()
                .flat_map(|unit| {
                    if little_endian {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    }
                })
                .collect();
            assert_eq!(decode_html(&body, None), text);
            let mut bom = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            bom.extend(body);
            assert_eq!(decode_html(&bom, Some("text/html; charset=utf-8")), text);
        }
    }

    #[test]
    fn incomplete_prefixes_never_panic_or_leak_a_partial_declaration() {
        for body in [
            b"<!-- charset=windows-1254 --><meta charset='utf-8'>".as_slice(),
            b"<div title='<meta charset=windows-1254>'><meta charset=utf-8>",
            b"<?xml version='1.0' encoding='utf-8'?><meta charset=utf-8>",
        ] {
            for end in 0..=body.len() {
                let _ = decode_html(&body[..end], None);
            }
        }
        assert_eq!(prescan::encoding(b"<meta charset='windows-1254'"), None);
        assert_eq!(prescan::encoding(b"<!-- <meta charset=windows-1254>"), None);
    }
}
