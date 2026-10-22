//! The gateway's account API: device sign-in, the account and the connector catalog.

use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::CloudError;

/// A sign-in waiting for the person to approve its code in the browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIn {
    /// Secret the app polls with.
    pub device_code: String,
    /// Code the person compares and approves (`ABCD-EFGH`).
    pub user_code: String,
    /// Approval page.
    pub verification_uri: String,
    /// Approval page with the code filled in; open this in the browser.
    pub verification_uri_complete: String,
    /// Seconds until the code expires.
    pub expires_in: u64,
    /// Seconds between polls.
    pub interval: u64,
}

/// What a poll found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll {
    /// Not approved yet.
    Pending,
    /// Polled too often; wait longer.
    SlowDown,
    /// Approved: the account token, delivered once.
    Approved(String),
}

/// The signed-in account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// The account's id (the web version keeps each account's data under it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// E-mail.
    pub email: Option<String>,
    /// Name.
    pub name: Option<String>,
    /// Balance, micro-dollars.
    pub balance_micros: i64,
    /// Reserved by requests in flight, micro-dollars.
    pub held_micros: i64,
    /// What new requests may use, micro-dollars.
    pub available_micros: i64,
    /// Micro-dollars in one credit.
    pub credit_unit_micros: i64,
    /// Below the low-balance threshold.
    pub low_balance: bool,
    /// The account site (buying credits).
    pub portal_url: String,
}

impl Account {
    /// The available balance in credits.
    #[allow(clippy::cast_precision_loss)]
    pub fn credits(&self) -> f64 {
        self.available_micros as f64 / self.credit_unit_micros.max(1) as f64
    }
}

/// An allowed request of a connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    /// GET or POST.
    pub method: String,
    /// Path pattern: `*` one segment, `**` any number.
    pub path: String,
}

/// A source the gateway offers on credits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Connector id (`dataforseo`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// What it offers.
    #[serde(default)]
    pub description: String,
    /// The provider's origin (`https://api.dataforseo.com`).
    pub upstream: String,
    /// Requests it forwards.
    pub routes: Vec<Route>,
    /// `active`, or `test` (admins only).
    pub status: String,
    /// Multiply a provider price by this for the credit price in US dollars.
    pub price_multiplier: f64,
}

/// The connectors offered on credits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    /// Micro-dollars in one credit.
    pub credit_unit_micros: i64,
    /// The account site.
    pub portal_url: String,
    /// Connectors.
    pub connectors: Vec<Connector>,
    /// Server work sold by the unit (pages a web program crawls on the server).
    #[serde(default)]
    pub meters: Vec<MeterPrice>,
}

/// The price of metered server work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterPrice {
    /// What is metered (`crawl.page`).
    pub sku: String,
    /// Micro-dollars a unit after the free ones.
    pub micros_per_unit: i64,
    /// Units free every month.
    pub free_per_month: i64,
}

/// What a metered batch cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterReceipt {
    /// What was metered.
    pub sku: String,
    /// Units in the batch.
    pub units: i64,
    /// Units taken from the month's free allowance.
    pub free_units: i64,
    /// Micro-dollars charged.
    pub charged_micros: i64,
    /// Free units left this month.
    pub free_left: i64,
    /// Micro-dollars left to spend.
    pub available_micros: i64,
    /// The batch was already metered; nothing was charged now.
    pub duplicate: bool,
}

/// How metering a batch went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Metered {
    /// Charged (or free).
    Charged(MeterReceipt),
    /// The free units are used and the credits do not cover the batch; nothing was charged.
    NoCredits {
        /// Micro-dollars left.
        available_micros: i64,
    },
}

impl Catalog {
    /// A connector by id.
    pub fn connector(&self, id: &str) -> Option<&Connector> {
        self.connectors.iter().find(|connector| connector.id == id)
    }
}

/// The gateway's account API over the app's transport.
pub struct CloudClient<'a> {
    transport: &'a dyn HttpTransport,
    base: String,
    app: String,
}

impl std::fmt::Debug for CloudClient<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudClient")
            .field("base", &self.base)
            .field("app", &self.app)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Problem {
    error: Option<String>,
    message: Option<String>,
}

fn problem(response: &HttpResponse) -> Problem {
    serde_json::from_slice(&response.body).unwrap_or(Problem {
        error: None,
        message: None,
    })
}

impl<'a> CloudClient<'a> {
    /// A client of the gateway at `base` for the program `app` (`keyword-miner/0.1.0 macOS`).
    pub fn new(
        transport: &'a dyn HttpTransport,
        base: impl Into<String>,
        app: impl Into<String>,
    ) -> Self {
        Self {
            transport,
            base: base.into(),
            app: app.into(),
        }
    }

    fn send(&self, request: HttpRequest) -> Result<HttpResponse, CloudError> {
        self.transport
            .send(&request.header("x-octopus-app", self.app.clone()))
            .map_err(|error| CloudError::Unavailable(error.to_string()))
    }

    fn json<T: DeserializeOwned>(response: &HttpResponse) -> Result<T, CloudError> {
        if response.status == 401 {
            return Err(CloudError::SignedOut);
        }
        if !response.is_success() {
            let problem = problem(response);
            return Err(CloudError::Unavailable(
                problem
                    .message
                    .or(problem.error)
                    .unwrap_or_else(|| format!("status {}", response.status)),
            ));
        }
        serde_json::from_slice(&response.body)
            .map_err(|error| CloudError::Invalid(format!("unexpected answer: {error}")))
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<HttpResponse, CloudError> {
        self.send(
            HttpRequest::post(
                format!("{}{path}", self.base),
                body.to_string().into_bytes(),
            )
            .header("content-type", "application/json"),
        )
    }

    /// Starts a sign-in: open [`SignIn::verification_uri_complete`] and poll with
    /// [`Self::poll_sign_in`] every [`SignIn::interval`] seconds.
    ///
    /// # Errors
    /// [`CloudError::Unavailable`] when the gateway cannot be reached.
    pub fn start_sign_in(&self) -> Result<SignIn, CloudError> {
        let response = self.post_json(
            "/v1/device/code",
            &serde_json::json!({ "client": self.app }),
        )?;
        Self::json(&response)
    }

    /// Asks whether the code was approved.
    ///
    /// # Errors
    /// [`CloudError::Denied`], [`CloudError::Expired`], [`CloudError::Unavailable`].
    pub fn poll_sign_in(&self, device_code: &str) -> Result<DevicePoll, CloudError> {
        let response = self.post_json(
            "/v1/device/token",
            &serde_json::json!({ "device_code": device_code }),
        )?;
        if response.is_success() {
            #[derive(Deserialize)]
            struct Granted {
                access_token: String,
            }
            let granted: Granted = Self::json(&response)?;
            return Ok(DevicePoll::Approved(granted.access_token));
        }
        match problem(&response).error.as_deref() {
            Some("authorization_pending") => Ok(DevicePoll::Pending),
            Some("slow_down") => Ok(DevicePoll::SlowDown),
            Some("access_denied") => Err(CloudError::Denied),
            Some("expired_token" | "invalid_grant") => Err(CloudError::Expired),
            other => Err(CloudError::Unavailable(other.map_or_else(
                || format!("status {}", response.status),
                str::to_owned,
            ))),
        }
    }

    /// Swaps the code the portal sent to this program's loopback address for a token.
    ///
    /// # Errors
    /// [`CloudError::Expired`] for a used, expired or foreign code; [`CloudError::Unavailable`].
    pub fn swap_connect_code(&self, code: &str, verifier: &str) -> Result<String, CloudError> {
        #[derive(Deserialize)]
        struct Granted {
            access_token: String,
        }
        let response = self.post_json(
            "/v1/connect/token",
            &serde_json::json!({ "code": code, "code_verifier": verifier }),
        )?;
        if response.status == 400 {
            return Err(CloudError::Expired);
        }
        let granted: Granted = Self::json(&response)?;
        Ok(granted.access_token)
    }

    /// The signed-in account and its balance.
    ///
    /// # Errors
    /// [`CloudError::SignedOut`] when the token is no longer valid.
    pub fn account(&self, token: &str) -> Result<Account, CloudError> {
        let response = self.send(
            HttpRequest::get(format!("{}/v1/me", self.base))
                .header("authorization", format!("Bearer {token}")),
        )?;
        Self::json(&response)
    }

    /// The connectors on offer (with a token, admins also see those in test).
    ///
    /// # Errors
    /// [`CloudError::Unavailable`] when the gateway cannot be reached.
    pub fn catalog(&self, token: Option<&str>) -> Result<Catalog, CloudError> {
        let mut request = HttpRequest::get(format!("{}/v1/catalog", self.base));
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        Self::json(&self.send(request)?)
    }

    /// Meters server work done for the account: `units` of `sku` under `key`, which the gateway
    /// charges once however often the batch is sent.
    ///
    /// # Errors
    /// [`CloudError::SignedOut`] for a token no longer valid; [`CloudError::Unavailable`] when the
    /// gateway cannot be reached or refuses the batch.
    pub fn meter(
        &self,
        token: &str,
        sku: &str,
        units: u32,
        key: &str,
    ) -> Result<Metered, CloudError> {
        let response = self.send(
            HttpRequest::post(
                format!("{}/v1/meter", self.base),
                serde_json::json!({ "sku": sku, "units": units, "idempotency_key": key })
                    .to_string()
                    .into_bytes(),
            )
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}")),
        )?;
        if response.status == 402 {
            #[derive(Deserialize)]
            struct Refused {
                #[serde(default)]
                available_micros: i64,
            }
            let refused: Refused = serde_json::from_slice(&response.body).unwrap_or(Refused {
                available_micros: 0,
            });
            return Ok(Metered::NoCredits {
                available_micros: refused.available_micros,
            });
        }
        Ok(Metered::Charged(Self::json(&response)?))
    }

    /// Sends a message to OctopusOn support: what the person wrote, and the program, page and
    /// error it is about (`context`, an object of short strings).
    ///
    /// # Errors
    /// [`CloudError::SignedOut`] for a token no longer valid; [`CloudError::Unavailable`] when the
    /// gateway cannot be reached or refuses the message (too many in an hour).
    pub fn support(
        &self,
        token: &str,
        message: &str,
        context: &serde_json::Value,
    ) -> Result<(), CloudError> {
        let response = self.send(
            HttpRequest::post(
                format!("{}/v1/support", self.base),
                serde_json::json!({ "message": message, "context": context })
                    .to_string()
                    .into_bytes(),
            )
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}")),
        )?;
        Self::json::<serde_json::Value>(&response).map(|_| ())
    }

    /// Ends the session: the gateway stops accepting the token.
    ///
    /// # Errors
    /// [`CloudError::Unavailable`] when the gateway cannot be reached.
    pub fn sign_out(&self, token: &str) -> Result<(), CloudError> {
        let response = self.send(
            HttpRequest::post(format!("{}/v1/logout", self.base), Vec::new())
                .header("authorization", format!("Bearer {token}")),
        )?;
        if response.is_success() || response.status == 401 {
            Ok(())
        } else {
            Err(CloudError::Unavailable(format!(
                "status {}",
                response.status
            )))
        }
    }
}
