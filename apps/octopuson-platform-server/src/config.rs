//! Settings from the environment.

use std::net::SocketAddr;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_oauth::OAuthClient;

/// What the server needs to run.
#[derive(Clone)]
pub struct Config {
    /// Address to listen on (behind nginx: a loopback port).
    pub bind: SocketAddr,
    /// Where every account's workspace and the server's own database live.
    pub data_dir: PathBuf,
    /// The address people open (`https://app.octopuson.com`), without a trailing slash.
    pub public_url: String,
    /// The account site (`https://my.octopuson.com`).
    pub portal_url: String,
    /// The gateway (`https://api.octopuson.com`).
    pub gateway: String,
    /// Seals the accounts' keys and tokens on disk.
    pub master_key: [u8; 32],
    /// The built web interface (`dist` of the app); a placeholder page without it.
    pub ui_dir: Option<PathBuf>,
    /// Offers `/auth/dev`, a sign-in without the account site, on a loopback address only.
    pub dev_login: bool,
    /// The server's Google OAuth client ("Web application" type) for Search Console and
    /// Analytics; without it Google cannot be connected.
    pub google: Option<OAuthClient>,
    /// SMTP server (`smtps://user:password@host:465`) and sender of report e-mails; without
    /// them reports are not e-mailed.
    pub smtp: Option<(String, String)>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .field("data_dir", &self.data_dir)
            .field("public_url", &self.public_url)
            .field("portal_url", &self.portal_url)
            .field("gateway", &self.gateway)
            .field("ui_dir", &self.ui_dir)
            .field("dev_login", &self.dev_login)
            .field("google", &self.google)
            .field("smtp", &self.smtp.as_ref().map(|(_, from)| from))
            .finish_non_exhaustive()
    }
}

/// Environment variables.
pub mod variables {
    /// Listening address, default `127.0.0.1:8796`.
    pub const BIND: &str = "PLATFORM_BIND";
    /// Data folder, default `./platform-data`.
    pub const DATA_DIR: &str = "PLATFORM_DATA_DIR";
    /// Public address, default `http://localhost:8796`.
    pub const PUBLIC_URL: &str = "PLATFORM_PUBLIC_URL";
    /// Account site, default `https://my.octopuson.com`.
    pub const PORTAL_URL: &str = "PLATFORM_PORTAL_URL";
    /// 32 bytes, base64: seals keys and tokens. Required outside the development sign-in.
    pub const MASTER_KEY: &str = "PLATFORM_MASTER_KEY";
    /// The built interface.
    pub const UI_DIR: &str = "PLATFORM_UI_DIR";
    /// `1` offers the development sign-in on a loopback address.
    pub const DEV_LOGIN: &str = "PLATFORM_DEV_LOGIN";
    /// Client id of the server's Google OAuth client (Web application type).
    pub const GOOGLE_CLIENT_ID: &str = "PLATFORM_GOOGLE_CLIENT_ID";
    /// Its secret.
    pub const GOOGLE_CLIENT_SECRET: &str = "PLATFORM_GOOGLE_CLIENT_SECRET";
    /// SMTP server of report e-mails: `smtps://user:password@host:465`, or
    /// `smtp://user:password@host:587?tls=required` for STARTTLS.
    pub const SMTP_URL: &str = "PLATFORM_SMTP_URL";
    /// Sender of report e-mails: `OctopusOn SEO <rapor@octopuson.com>`.
    pub const MAIL_FROM: &str = "PLATFORM_MAIL_FROM";
}

impl Config {
    /// Reads the settings from `lookup` (the process environment in production).
    ///
    /// # Errors
    /// A message naming the setting that cannot be used.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let get = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let bind: SocketAddr = get(variables::BIND)
            .unwrap_or_else(|| "127.0.0.1:8796".to_owned())
            .parse()
            .map_err(|error| format!("{}: {error}", variables::BIND))?;
        let data_dir =
            PathBuf::from(get(variables::DATA_DIR).unwrap_or_else(|| "platform-data".to_owned()));
        let public_url = get(variables::PUBLIC_URL)
            .unwrap_or_else(|| format!("http://localhost:{}", bind.port()))
            .trim_end_matches('/')
            .to_owned();
        if !public_url.starts_with("https://") && !public_url.starts_with("http://localhost") {
            return Err(format!(
                "{}: the public address must be https (or http://localhost)",
                variables::PUBLIC_URL
            ));
        }
        let portal_url = get(variables::PORTAL_URL)
            .unwrap_or_else(|| octopuson_cloud::DEFAULT_PORTAL.to_owned())
            .trim_end_matches('/')
            .to_owned();
        let gateway = octopuson_cloud::api_base(get(octopuson_cloud::API_VARIABLE).as_deref())
            .map_err(|error| format!("{}: {error}", octopuson_cloud::API_VARIABLE))?;
        let dev_login = get(variables::DEV_LOGIN).as_deref() == Some("1");
        if dev_login && !bind.ip().is_loopback() {
            return Err(format!(
                "{}: the development sign-in is only offered on a loopback address",
                variables::DEV_LOGIN
            ));
        }
        let master_key = match get(variables::MASTER_KEY) {
            Some(text) => {
                let bytes = STANDARD
                    .decode(text)
                    .map_err(|error| format!("{}: {error}", variables::MASTER_KEY))?;
                <[u8; 32]>::try_from(bytes.as_slice())
                    .map_err(|_| format!("{}: must be 32 bytes in base64", variables::MASTER_KEY))?
            }
            None if dev_login => *b"octopuson-platform-development!!",
            None => {
                return Err(format!(
                    "{}: set 32 random bytes in base64 (openssl rand -base64 32)",
                    variables::MASTER_KEY
                ));
            }
        };
        let google = match (
            get(variables::GOOGLE_CLIENT_ID),
            get(variables::GOOGLE_CLIENT_SECRET),
        ) {
            (Some(client_id), Some(client_secret)) => Some(OAuthClient {
                client_id,
                client_secret,
            }),
            (None, None) => None,
            _ => {
                return Err(format!(
                    "{} and {}: set both or neither",
                    variables::GOOGLE_CLIENT_ID,
                    variables::GOOGLE_CLIENT_SECRET
                ));
            }
        };
        let smtp = match (get(variables::SMTP_URL), get(variables::MAIL_FROM)) {
            (Some(url), Some(from)) => Some((url, from)),
            (None, None) => None,
            _ => {
                return Err(format!(
                    "{} and {}: set both or neither",
                    variables::SMTP_URL,
                    variables::MAIL_FROM
                ));
            }
        };
        Ok(Self {
            bind,
            data_dir,
            public_url,
            portal_url,
            gateway,
            master_key,
            ui_dir: get(variables::UI_DIR).map(PathBuf::from),
            dev_login,
            google,
            smtp,
        })
    }

    /// Whether cookies need `Secure` (the public address is https).
    pub fn secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    /// The sign-in's return address.
    pub fn callback_url(&self) -> String {
        format!("{}/auth/callback", self.public_url)
    }

    /// Google's return address.
    pub fn google_callback_url(&self) -> String {
        format!("{}/auth/google/callback", self.public_url)
    }
}
