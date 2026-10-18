use super::columns::{competition, count, decimal, fold, role, volume};
use super::read::{FileKind, decode, delimited, rows};
use super::*;

#[test]
fn reads_headers_of_common_exports() {
    assert_eq!(fold("Avg. monthly searches"), "avg monthly searches");
    assert_eq!(fold("Ort. Aylık Aramalar"), "ort aylik aramalar");
    assert_eq!(
        fold("  En Çok Yapılan Sorgular "),
        "en cok yapilan sorgular"
    );
    assert_eq!(role("Keyword"), Some(ColumnRole::Keyword));
    assert_eq!(role("Top queries"), Some(ColumnRole::Keyword));
    assert_eq!(role("Anahtar Kelime"), Some(ColumnRole::Keyword));
    assert_eq!(role("Фраза"), Some(ColumnRole::Keyword));
    assert_eq!(role("Avg. monthly searches"), Some(ColumnRole::Volume));
    assert_eq!(role("Search Volume"), Some(ColumnRole::Volume));
    assert_eq!(role("CPC (USD)"), Some(ColumnRole::Cpc));
    assert_eq!(role("KD %"), Some(ColumnRole::Difficulty));
    assert_eq!(role("Keyword Difficulty"), Some(ColumnRole::Difficulty));
    assert_eq!(
        role("Competition (indexed value)"),
        Some(ColumnRole::CompetitionIndex)
    );
    assert_eq!(role("Competitive Density"), Some(ColumnRole::Competition));
    assert_eq!(
        role("Searches: Sep 2025"),
        Some(ColumnRole::Month("2025-09".to_owned()))
    );
    assert_eq!(
        role("Aramalar: Eyl 2025"),
        Some(ColumnRole::Month("2025-09".to_owned()))
    );
    assert_eq!(role("Impressions"), Some(ColumnRole::Impressions));
    assert_eq!(role("Gösterimler"), Some(ColumnRole::Impressions));
    assert_eq!(role("Trend"), None);
}

#[test]
fn reads_numbers_as_tools_write_them() {
    assert_eq!(count("12,100"), Some(12_100));
    assert_eq!(count("1.300"), Some(1_300));
    assert_eq!(count("1 300"), Some(1_300));
    assert_eq!(count("1K"), Some(1_000));
    assert_eq!(count("2,5 B"), Some(2_500));
    assert_eq!(count("1M"), Some(1_000_000));
    assert_eq!(count("n/a"), None);
    assert_eq!(decimal("$1.25"), Some(1.25));
    assert_eq!(decimal("1,25 ₺"), Some(1.25));
    assert_eq!(decimal("1.234,5"), Some(1_234.5));
    assert_eq!(decimal("1,234.5"), Some(1_234.5));
    assert_eq!(decimal("34%"), Some(34.0));
    assert_eq!(decimal("-"), None);
    assert_eq!(volume("1K – 10K"), Some((None, Some(1_000), Some(10_000))));
    assert_eq!(
        volume("1 B – 10 B"),
        Some((None, Some(1_000), Some(10_000)))
    );
    assert_eq!(volume("8,100"), Some((Some(8_100), None, None)));
    assert_eq!(volume("-"), None);
    assert_eq!(competition("0.45"), Some(0.45));
    assert_eq!(competition("45%"), Some(0.45));
    assert_eq!(competition("Yüksek"), Some(0.75));
    assert_eq!(competition("Low"), Some(0.25));
}

#[test]
fn decodes_utf16_and_turkish_single_byte_text() {
    let text = "Keyword\tAvg. monthly searches\ntürk kahvesi\t1000\n";
    let mut le = vec![0xFF, 0xFE];
    le.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(decode(&le), text);
    let without_bom: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(decode(&without_bom), text);
    // "çay" in Windows-1254.
    assert_eq!(decode(&[0xE7, b'a', b'y']), "çay");
}

#[test]
fn splits_quoted_cells_with_the_separator_used_most() {
    let rows = delimited(
        "Keyword;Volume\r\n\"kahve; çekirdek\";\"1.300\"\n\n\"filtre \"\"kahve\"\"\";90\n",
    );
    assert_eq!(
        rows,
        [
            vec!["Keyword", "Volume"],
            vec!["kahve; çekirdek", "1.300"],
            vec!["filtre \"kahve\"", "90"],
        ]
    );
    let lines = rows_of("kahve, çay\n\n  filtre kahve \n", FileKind::Lines);
    assert_eq!(lines, [vec!["kahve, çay"], vec!["filtre kahve"]]);
}

fn rows_of(text: &str, kind: FileKind) -> Vec<Vec<String>> {
    rows(text.as_bytes(), kind).unwrap()
}

#[test]
fn finds_the_header_below_title_lines() {
    let rows = delimited(
        "Keyword Stats 2026-10-18 at 10_00_00\nAll locations\n\
         Keyword\tCurrency\tAvg. monthly searches\tCompetition\tCompetition (indexed value)\tSearches: Aug 2026\tSearches: Sep 2026\n\
         türk kahvesi\tUSD\t1K – 10K\tHigh\t81\t900\t1,100\n",
    );
    let table = Table::new(rows);
    assert_eq!(table.keyword, 0);
    assert_eq!(table.data.len(), 1);
    assert_eq!(table.columns[1].role, None);
    let numbers = table.numbers(&table.data[0]).unwrap();
    assert_eq!(numbers.volume, None);
    assert_eq!(
        (numbers.volume_low, numbers.volume_high),
        (Some(1_000), Some(10_000))
    );
    assert_eq!(numbers.competition, Some(0.81));
    assert_eq!(numbers.monthly.len(), 2);
    assert_eq!(numbers.monthly[1].volume, 1_100);

    // Without a header the first column holds the keywords.
    let plain = Table::new(vec![vec!["kahve".to_owned(), "12".to_owned()]]);
    assert_eq!(plain.columns[0].role, Some(ColumnRole::Keyword));
    assert_eq!(plain.data.len(), 1);
    assert!(plain.numbers(&plain.data[0]).is_none());
}

#[test]
fn reads_the_first_sheet_of_a_workbook() {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.write_string(0, 0, "Keyword").unwrap();
    sheet.write_string(0, 1, "Volume").unwrap();
    sheet.write_string(0, 3, "KD").unwrap();
    sheet.write_string(1, 0, "kahve & süt").unwrap();
    sheet.write_number(1, 1, 2_400.0).unwrap();
    sheet.write_number(1, 3, 37.0).unwrap();
    sheet.write_string(2, 0, "türk kahvesi").unwrap();
    let bytes = workbook.save_to_buffer().unwrap();
    let rows = rows(&bytes, FileKind::Workbook).unwrap();
    assert_eq!(rows[0], ["Keyword", "Volume", "", "KD"]);
    assert_eq!(rows[1], ["kahve & süt", "2400", "", "37"]);
    assert_eq!(rows[2], ["türk kahvesi"]);
}

#[test]
fn reads_the_queries_of_a_search_console_archive() {
    use std::io::Write;

    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive.start_file("Pages.csv", options).unwrap();
        archive
            .write_all(b"Top pages,Clicks\nhttps://a.com/,1\n")
            .unwrap();
        archive.start_file("Queries.csv", options).unwrap();
        archive
            .write_all(
                "Top queries,Clicks,Impressions,CTR,Position\ntürk kahvesi,12,340,3.5%,4.2\n"
                    .as_bytes(),
            )
            .unwrap();
        archive.finish().unwrap();
    }
    let rows = rows(buffer.get_ref(), FileKind::Archive).unwrap();
    assert_eq!(rows[1][0], "türk kahvesi");
}
