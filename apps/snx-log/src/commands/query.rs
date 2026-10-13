use std::io::{self, Write};

use clap::Args;
use miette::{Diagnostic, SourceSpan};
use seonexus_log_app::Workspace;
use seonexus_log_app::query::{
    ExportRequest, Page, QueryError, QueryOutcome, QueryRequest, QuerySpec, export_query, run_query,
};
use seonexus_log_app::report::{CsvWriter, JsonWriter};

use super::output::{ignore_broken_pipe, write_json};
use super::results::ResultArgs;
use super::table::TextTable;

/// Reference shown by `snx-log query --help`.
const QUERY_HELP: &str = "\
QUERY LANGUAGE
  Conditions are written field:value and combined with spaces (AND), OR, NOT / -, and ( ).
  Quote the whole query in the shell when it contains >, <, ( or ).

  Fields      date ip method url path directory params status bytes response_ms ua
              referer host protocol bot bot_category verification content_type
              referer_host referer_type
  Values      status:404  status:301,302  status:4xx  bytes:>10000  response_ms:500..2000
              url:/blog/*  url:~\"^/(en|de)/\"  date:2026-10-01..2026-10-07  date:last30d
              params:true  ua:*Firefox*  bot:googlebot*  bot_category:ai-*  -bot:*
              verification:verified|spoofed|unverified  content_type:html|image|js|css|...
              referer_type:direct|internal|search|ai|social|email|other
  Directives  group:url,status,bot,content_type,referer_type,...
              metric:hits,bytes,avg_bytes,avg_ms,p95_ms,max_ms,unique_urls,unique_ips,
                     first_seen,last_seen,days_since_last,last_status,status_codes,
                     unique_statuses,inconsistent,errors,hits_1xx..hits_5xx,co2_mg,
                     bot_hits,search_hits,ai_hits,googlebot_hits,googlebot_smartphone_hits,
                     googlebot_desktop_hits,bingbot_hits,yandex_hits
              sort:hits:desc   limit:500

EXAMPLES
  snx-log query acme.com 'status:5xx group:url metric:hits,last_seen'
  snx-log query acme.com --template googlebot-daily 'date:last30d'
  snx-log query acme.com --template client-errors -o 404s.xlsx
  snx-log saved add \"Googlebot 404\" 'bot:googlebot* verification:verified status:404 group:url'
  snx-log query acme.com --saved \"Googlebot 404\" 'date:last7d'
  snx-log query acme.com 'bot:googlebot* verification:verified -status:200 date:last7d'
  snx-log query acme.com 'bot_category:ai-* group:bot,day'
  snx-log query acme.com 'referer_type:ai -bot:* group:day,referer_host'";

#[derive(Debug, Args)]
#[command(after_long_help = QUERY_HELP)]
pub(crate) struct QueryArgs {
    /// Site to query.
    site: String,

    /// Query text; several words are joined with spaces. With --template it refines the
    /// template (extra filters, or a new group/metric/sort/limit).
    #[arg(value_name = "QUERY")]
    text: Vec<String>,

    /// Start from a built-in report template (see `snx-log templates`).
    #[arg(short, long, value_name = "ID", conflicts_with = "saved")]
    template: Option<String>,

    /// Start from a saved query (see `snx-log saved list`).
    #[arg(short, long, value_name = "NAME")]
    saved: Option<String>,

    #[command(flatten)]
    output: ResultArgs,
}

impl QueryArgs {
    fn spec(&self) -> QuerySpec {
        let text = self.text.join(" ");
        match (&self.template, &self.saved) {
            (Some(id), _) => QuerySpec::Template {
                id: id.clone(),
                refine: Some(text),
            },
            (None, Some(name)) => QuerySpec::Saved {
                name: name.clone(),
                refine: Some(text),
            },
            (None, None) => QuerySpec::text(text),
        }
    }
}

pub(crate) fn run(workspace: &Workspace, args: &QueryArgs) -> miette::Result<()> {
    let spec = args.spec();
    if let Some(path) = &args.output.output {
        let request = ExportRequest {
            site: args.site.clone(),
            spec,
            path: path.clone(),
            format: None,
        };
        let report = export_query(workspace, &request).map_err(diagnose)?;
        tracing::info!(
            rows = report.outcome.rows,
            source = report.outcome.source,
            elapsed_ms = report.outcome.elapsed_ms,
            "exported to {}",
            report.path
        );
        return Ok(());
    }
    let request = |page| QueryRequest {
        site: args.site.clone(),
        spec: spec.clone(),
        page,
    };
    let stdout = io::stdout();
    if args.output.csv || args.output.json {
        let mut out = stdout.lock();
        let outcome = if args.output.csv {
            let mut sink = CsvWriter::new(&mut out);
            run_query(workspace, &request(None), &mut sink)
        } else {
            let mut sink = JsonWriter::new(&mut out);
            run_query(workspace, &request(None), &mut sink)
        };
        return match outcome {
            Ok(outcome) => {
                log_outcome(&outcome);
                Ok(())
            }
            Err(
                QueryError::Store(seonexus_log_app::StoreError::Output(error))
                | QueryError::Output(error),
            ) => ignore_broken_pipe(Err(error)),
            Err(error) => Err(diagnose(error)),
        };
    }

    // Ask for one extra row to know whether more results exist.
    let page = Page {
        offset: args.output.offset,
        size: args.output.rows.saturating_add(1),
    };
    let mut table = TextTable::default();
    let outcome = run_query(workspace, &request(Some(page)), &mut table).map_err(diagnose)?;
    let shown = usize::try_from(args.output.rows).unwrap_or(usize::MAX);
    let mut out = stdout.lock();
    let result = table.write(&mut out, shown).and_then(|()| {
        footer(
            &mut out,
            &outcome,
            args.output.offset,
            table.len().min(shown),
            table.len() > shown,
        )
    });
    ignore_broken_pipe(result)
}

fn footer(
    out: &mut dyn Write,
    outcome: &QueryOutcome,
    offset: u64,
    shown: usize,
    more: bool,
) -> io::Result<()> {
    writeln!(out)?;
    if shown == 0 {
        writeln!(out, "No matching rows.")?;
    } else {
        let first = offset + 1;
        let last = offset + shown as u64;
        let more = if more {
            " (more available: --offset, --rows, --csv or -o FILE)"
        } else {
            ""
        };
        writeln!(out, "Rows {first}–{last}{more}")?;
    }
    writeln!(
        out,
        "Query  {}\nSource {} · {} ms",
        outcome.query, outcome.source, outcome.elapsed_ms
    )
}

fn log_outcome(outcome: &QueryOutcome) {
    tracing::debug!(
        rows = outcome.rows,
        source = outcome.source,
        elapsed_ms = outcome.elapsed_ms,
        query = %outcome.query,
        "query finished"
    );
}

/// A query syntax error rendered with the offending part of the query underlined.
#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(
    code(snx_log::query::syntax),
    help("run `snx-log query --help` for the query language reference")
)]
struct SyntaxError {
    message: String,
    #[source_code]
    query: String,
    #[label("here")]
    span: SourceSpan,
}

fn diagnose(error: QueryError) -> miette::Report {
    match error {
        QueryError::Parse { text, source } => miette::Report::new(SyntaxError {
            message: source.to_string(),
            span: source.span().into(),
            query: text,
        }),
        other => miette::Report::from_err(other),
    }
}

/// `snx-log templates`: lists built-in reports.
pub(crate) fn list_templates(json: bool) -> miette::Result<()> {
    let templates = seonexus_log_app::query::templates();
    if json {
        return write_json(&templates);
    }
    super::output::write_text(|out| {
        for template in &templates {
            writeln!(out, "{:<18} {}", template.id, template.title)?;
            writeln!(out, "{:<18} {}", "", template.description)?;
            writeln!(out, "{:<18} {}\n", "", template.query)?;
        }
        Ok(())
    })
}
