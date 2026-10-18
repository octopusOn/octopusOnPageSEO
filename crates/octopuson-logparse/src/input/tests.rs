use std::io::Write;

use super::*;

const CONTENT: &str = "line one\nline two\n";

fn read_all(path: &Path) -> Vec<(String, Compression, String)> {
    let mut streams = Vec::new();
    for_each_stream::<InputError, _>(path, |info, reader| {
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .map_err(|source| InputError::Io {
                path: path.to_owned(),
                source,
            })?;
        streams.push((info.name.clone(), info.compression, text));
        Ok(ControlFlow::Continue(()))
    })
    .unwrap();
    streams
}

fn write_temp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn sniff_magic_bytes() {
    assert_eq!(Compression::sniff(&[0x1f, 0x8b, 8]), Compression::Gzip);
    assert_eq!(
        Compression::sniff(&[0x28, 0xb5, 0x2f, 0xfd]),
        Compression::Zstd
    );
    assert_eq!(Compression::sniff(b"BZh9"), Compression::Bzip2);
    assert_eq!(Compression::sniff(b"PK\x03\x04"), Compression::Zip);
    assert_eq!(Compression::sniff(b"1.2.3.4 -"), Compression::None);
    assert_eq!(Compression::sniff(b""), Compression::None);
}

#[test]
fn reads_plain_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp(&dir, "access.log", CONTENT.as_bytes());
    let streams = read_all(&path);
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].1, Compression::None);
    assert_eq!(streams[0].2, CONTENT);
}

#[test]
fn reads_multi_member_gzip_regardless_of_extension() {
    let mut bytes = Vec::new();
    for part in ["line one\n", "line two\n"] {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(part.as_bytes()).unwrap();
        bytes.extend(encoder.finish().unwrap());
    }
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp(&dir, "access.log.1", &bytes);
    let streams = read_all(&path);
    assert_eq!(streams[0].1, Compression::Gzip);
    assert_eq!(streams[0].2, CONTENT);
}

#[test]
fn reads_zstd() {
    let bytes = zstd::encode_all(CONTENT.as_bytes(), 1).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let streams = read_all(&write_temp(&dir, "a.zst", &bytes));
    assert_eq!(streams[0].1, Compression::Zstd);
    assert_eq!(streams[0].2, CONTENT);
}

#[test]
fn reads_bzip2() {
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    encoder.write_all(CONTENT.as_bytes()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let streams = read_all(&write_temp(&dir, "a.bz2", &encoder.finish().unwrap()));
    assert_eq!(streams[0].1, Compression::Bzip2);
    assert_eq!(streams[0].2, CONTENT);
}

#[test]
fn reads_every_zip_entry_and_skips_directories() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    writer.add_directory("logs/", options).unwrap();
    for (name, body) in [("logs/a.log", "a\n"), ("logs/b.log", "b\n")] {
        writer.start_file(name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();

    let dir = tempfile::tempdir().unwrap();
    let path = write_temp(&dir, "logs.zip", &bytes);
    let streams = read_all(&path);
    let names: Vec<_> = streams
        .iter()
        .map(|s| s.0.rsplit('!').next().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["/logs/a.log", "/logs/b.log"]);
    assert_eq!(streams[1].2, "b\n");
}

#[test]
fn break_stops_after_first_stream() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for name in ["a.log", "b.log"] {
        writer.start_file(name, options).unwrap();
        writer.write_all(b"x\n").unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp(&dir, "two.zip", &writer.finish().unwrap().into_inner());

    let mut visited = 0;
    for_each_stream::<InputError, _>(&path, |_, _| {
        visited += 1;
        Ok(ControlFlow::Break(()))
    })
    .unwrap();
    assert_eq!(visited, 1);
}

#[test]
fn missing_file_reports_path() {
    let error = for_each_stream::<InputError, _>(Path::new("/definitely/missing.log"), |_, _| {
        Ok(ControlFlow::Continue(()))
    })
    .unwrap_err();
    assert!(error.to_string().contains("/definitely/missing.log"));
}
