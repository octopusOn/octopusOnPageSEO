use std::io::{self, Write};

use seonexus_log_app::ParseSummary;

/// Renders the parse details shared by `inspect` and `import`.
pub(super) fn render_summary(out: &mut dyn Write, summary: &ParseSummary) -> io::Result<()> {
    writeln!(
        out,
        "Lines        {} total, {} parsed, {} skipped",
        summary.total_lines,
        summary.parsed_lines,
        summary.skipped_lines()
    )?;
    if summary.filtered_lines > 0 {
        writeln!(
            out,
            "Filtered     {} parsed lines not stored (site settings)",
            summary.filtered_lines
        )?;
    }
    if let (Some(first), Some(last)) = (&summary.first_request, &summary.last_request) {
        writeln!(out, "Time range   {first} → {last}")?;
    }
    render_counts(out, "Status", summary.status_classes.iter())?;
    render_counts(out, "Skipped", summary.skipped_by_reason.iter())?;

    if summary.streams.len() > 1 {
        writeln!(out, "Streams")?;
        for stream in &summary.streams {
            writeln!(out, "  {:>10} lines  {}", stream.lines, stream.name)?;
        }
    }
    if !summary.error_samples.is_empty() {
        writeln!(out, "Skipped line examples")?;
        for sample in &summary.error_samples {
            writeln!(
                out,
                "  {}:{}  {}",
                sample.stream, sample.line_number, sample.reason
            )?;
            writeln!(out, "    {}", sample.excerpt)?;
        }
    }
    Ok(())
}

fn render_counts<'a>(
    out: &mut dyn Write,
    label: &str,
    counts: impl Iterator<Item = (&'a String, &'a u64)>,
) -> io::Result<()> {
    let parts: Vec<String> = counts
        .map(|(key, count)| format!("{key}: {count}"))
        .collect();
    if parts.is_empty() {
        return Ok(());
    }
    writeln!(out, "{label:<12} {}", parts.join(", "))
}
