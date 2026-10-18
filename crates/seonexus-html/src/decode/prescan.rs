//! Bounded byte prescan: comments, tag attributes and pragma-gated HTML declarations.
//! This is the HTML encoding prescan, not DOM construction or locale-based autodetection.

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1252, X_USER_DEFINED};

pub(super) fn encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.starts_with(b"<\0?\0x\0") {
        return Some(UTF_16LE);
    }
    if bytes.starts_with(b"\0<\0?\0x") {
        return Some(UTF_16BE);
    }
    scan_meta(bytes).or_else(|| xml_encoding(bytes))
}

fn scan_meta(bytes: &[u8]) -> Option<&'static Encoding> {
    let mut position = 0;
    while position < bytes.len() {
        let tail = &bytes[position..];
        if tail.starts_with(b"<!--") {
            // The closing dashes may overlap the opening sequence (<!-->).
            position += 2 + tail[2..].windows(3).position(|part| part == b"-->")? + 3;
            continue;
        }
        if tail.len() > 5
            && tail[..5].eq_ignore_ascii_case(b"<meta")
            && (space(tail[5]) || tail[5] == b'/')
        {
            position += 5;
            let attrs = attributes(bytes, &mut position)?;
            if let Some(encoding) = meta_encoding(&attrs) {
                return Some(encoding);
            }
        } else if tail.first() == Some(&b'<') {
            let name_start = if tail.get(1) == Some(&b'/') { 2 } else { 1 };
            if tail.get(name_start).is_some_and(u8::is_ascii_alphabetic) {
                position += name_start;
                while !bytes
                    .get(position)
                    .is_some_and(|byte| space(*byte) || *byte == b'>')
                {
                    bytes.get(position)?;
                    position += 1;
                }
                attributes(bytes, &mut position)?;
            } else if tail
                .get(1)
                .is_some_and(|byte| matches!(byte, b'!' | b'/' | b'?'))
            {
                position += tail.iter().position(|&byte| byte == b'>')?;
            }
        }
        position += 1;
    }
    None
}

type Attribute<'a> = (&'a [u8], &'a [u8]);

fn space(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn attributes<'a>(bytes: &'a [u8], position: &mut usize) -> Option<Vec<Attribute<'a>>> {
    let mut result: Vec<Attribute<'a>> = Vec::new();
    loop {
        while bytes
            .get(*position)
            .is_some_and(|byte| space(*byte) || *byte == b'/')
        {
            *position += 1;
        }
        if *bytes.get(*position)? == b'>' {
            return Some(result);
        }
        let attribute = attribute(bytes, position)?;
        if !result
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(attribute.0))
        {
            result.push(attribute);
        }
    }
}

fn attribute<'a>(bytes: &'a [u8], position: &mut usize) -> Option<Attribute<'a>> {
    let start = *position;
    loop {
        let byte = *bytes.get(*position)?;
        if space(byte) || matches!(byte, b'/' | b'>') || (byte == b'=' && *position > start) {
            break;
        }
        *position += 1;
    }
    let name = &bytes[start..*position];
    while bytes.get(*position).is_some_and(|byte| space(*byte)) {
        *position += 1;
    }
    if *bytes.get(*position)? != b'=' {
        return Some((name, &[]));
    }
    *position += 1;
    while bytes.get(*position).is_some_and(|byte| space(*byte)) {
        *position += 1;
    }
    let first = *bytes.get(*position)?;
    if matches!(first, b'\'' | b'"') {
        *position += 1;
        let start = *position;
        while *bytes.get(*position)? != first {
            *position += 1;
        }
        let value = &bytes[start..*position];
        *position += 1;
        Some((name, value))
    } else {
        let start = *position;
        while !space(*bytes.get(*position)?) && bytes[*position] != b'>' {
            *position += 1;
        }
        Some((name, &bytes[start..*position]))
    }
}

fn meta_encoding(attributes: &[Attribute<'_>]) -> Option<&'static Encoding> {
    let mut pragma = false;
    let mut need_pragma = None;
    // Some(None) distinguishes an unsupported charset attribute from no candidate yet.
    let mut charset = None;
    for &(name, value) in attributes {
        if name.eq_ignore_ascii_case(b"http-equiv") {
            pragma = value.eq_ignore_ascii_case(b"content-type");
        } else if name.eq_ignore_ascii_case(b"charset") {
            charset = Some(Encoding::for_label(value));
            need_pragma = Some(false);
        } else if name.eq_ignore_ascii_case(b"content")
            && charset.is_none()
            && let Some(encoding) = content_encoding(value)
        {
            charset = Some(Some(encoding));
            need_pragma = Some(true);
        }
    }
    if need_pragma? && !pragma {
        return None;
    }
    charset.flatten().map(|encoding| {
        if encoding == UTF_16LE || encoding == UTF_16BE {
            UTF_8
        } else if encoding == X_USER_DEFINED {
            WINDOWS_1252
        } else {
            encoding
        }
    })
}

fn content_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    let mut position = 0;
    loop {
        let offset = bytes[position..]
            .windows(7)
            .position(|part| part.eq_ignore_ascii_case(b"charset"))?;
        position += offset + 7;
        while bytes.get(position).is_some_and(|byte| space(*byte)) {
            position += 1;
        }
        if *bytes.get(position)? != b'=' {
            continue;
        }
        position += 1;
        while bytes.get(position).is_some_and(|byte| space(*byte)) {
            position += 1;
        }
        let first = *bytes.get(position)?;
        let label = if matches!(first, b'\'' | b'"') {
            position += 1;
            let end = bytes[position..].iter().position(|&byte| byte == first)?;
            &bytes[position..position + end]
        } else {
            let end = bytes[position..]
                .iter()
                .position(|&byte| space(byte) || byte == b';')
                .unwrap_or(bytes.len() - position);
            &bytes[position..position + end]
        };
        return Encoding::for_label(label);
    }
}

fn xml_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    if !bytes.starts_with(b"<?xml") {
        return None;
    }
    let end = bytes.iter().position(|&byte| byte == b'>')?;
    let declaration = &bytes[..end];
    let mut position = declaration
        .windows(8)
        .position(|part| part == b"encoding")?
        + 8;
    while declaration.get(position).is_some_and(|&byte| byte <= b' ') {
        position += 1;
    }
    if *declaration.get(position)? != b'=' {
        return None;
    }
    position += 1;
    while declaration.get(position).is_some_and(|&byte| byte <= b' ') {
        position += 1;
    }
    let quote = *declaration.get(position)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    position += 1;
    let end = declaration[position..]
        .iter()
        .position(|&byte| byte == quote)?
        + position;
    let label = &declaration[position..end];
    if label.iter().any(|&byte| byte <= b' ') {
        return None;
    }
    Encoding::for_label(label).map(|encoding| {
        if encoding == UTF_16LE || encoding == UTF_16BE {
            UTF_8
        } else {
            encoding
        }
    })
}
