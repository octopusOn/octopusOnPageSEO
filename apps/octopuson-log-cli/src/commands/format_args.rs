use clap::Args;
use octopuson_log_app::FormatChoice;

/// Log format selection shared by commands that parse logs.
#[derive(Debug, Clone, Args)]
pub(crate) struct FormatArgs {
    /// Built-in format id (traefik-clf, traefik-json, combined, common) or `auto`.
    #[arg(short, long, default_value = "auto")]
    format: String,

    /// Custom Nginx `log_format` string or pasted directive.
    #[arg(long, value_name = "FORMAT", conflicts_with = "apache_format")]
    nginx_format: Option<String>,

    /// Custom Apache `LogFormat` string or pasted directive.
    #[arg(long, value_name = "FORMAT")]
    apache_format: Option<String>,
}

impl FormatArgs {
    pub(crate) fn choice(&self) -> FormatChoice {
        match (&self.nginx_format, &self.apache_format) {
            (Some(nginx), _) => FormatChoice::Nginx(nginx.clone()),
            (None, Some(apache)) => FormatChoice::Apache(apache.clone()),
            (None, None) if self.format == "auto" => FormatChoice::Auto,
            (None, None) => FormatChoice::Builtin(self.format.clone()),
        }
    }
}
