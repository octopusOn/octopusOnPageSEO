//! The command router: every operation by name, with JSON arguments and answers.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jobs::JobManager;
use crate::{AppError, Workspace};

/// The kind of a failed call, for pages to react to (sign in, enter a key, wait).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The arguments or a value cannot be used.
    Invalid,
    /// No such command or record.
    NotFound,
    /// The data is in use by other work.
    Busy,
    /// No own key and no credits for a paid source.
    NoSource,
    /// The month's budget would be passed.
    Budget,
    /// A key or the account was refused.
    Auth,
    /// No balance left with the provider or in credits.
    Quota,
    /// Too many requests.
    RateLimited,
    /// A service is unavailable.
    Unavailable,
    /// The provider refused or answered something unreadable.
    Provider,
    /// Not offered where the program runs.
    Unsupported,
    /// The OctopusOn SEO account cannot be used.
    Account,
    /// Anything else.
    Internal,
}

/// A failed call as pages get it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallError {
    /// What kind of failure.
    pub code: ErrorCode,
    /// For people.
    pub message: String,
    /// Seconds to wait before trying again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CallError {}

impl From<AppError> for CallError {
    fn from(error: AppError) -> Self {
        let message = error.to_string();
        let (code, retry_after) = match error {
            AppError::Invalid(_) => (ErrorCode::Invalid, None),
            AppError::NotFound(_) => (ErrorCode::NotFound, None),
            AppError::Busy(_) => (ErrorCode::Busy, None),
            AppError::NoSource(_) => (ErrorCode::NoSource, None),
            AppError::Budget(_) => (ErrorCode::Budget, None),
            AppError::Auth(_) => (ErrorCode::Auth, None),
            AppError::Quota(_) => (ErrorCode::Quota, None),
            AppError::RateLimited { retry_after } => (ErrorCode::RateLimited, retry_after),
            AppError::Unavailable(_) => (ErrorCode::Unavailable, None),
            AppError::Provider(_) => (ErrorCode::Provider, None),
            AppError::Unsupported(_) => (ErrorCode::Unsupported, None),
            AppError::Account(_) => (ErrorCode::Account, None),
            AppError::Internal(_) => (ErrorCode::Internal, None),
        };
        Self {
            code,
            message,
            retry_after,
        }
    }
}

/// Opens an address in the person's browser; `false` when it could not.
pub type Opener = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// What a command works with.
pub struct Context<'a> {
    /// The data folder.
    pub workspace: &'a Arc<Workspace>,
    /// Its jobs.
    pub jobs: &'a JobManager,
    /// Opens addresses in the browser (desktop); on the web pages open them themselves.
    pub open: &'a Opener,
}

type Handler = Box<dyn Fn(&Context<'_>, Value) -> Result<Value, AppError> + Send + Sync>;

/// Commands by name.
#[derive(Default)]
pub struct Router {
    handlers: BTreeMap<&'static str, Handler>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.handlers.keys()).finish()
    }
}

impl Router {
    /// A router without commands.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a command whose arguments are `A` and whose answer is `R`.
    ///
    /// # Panics
    /// In debug builds, when the name is registered twice (the later one would win).
    pub fn add<A, R>(
        &mut self,
        name: &'static str,
        handler: fn(&Context<'_>, A) -> Result<R, AppError>,
    ) where
        A: DeserializeOwned + 'static,
        R: Serialize + 'static,
    {
        let previous = self.handlers.insert(
            name,
            Box::new(move |context, arguments| {
                let arguments = match arguments {
                    Value::Null => Value::Object(serde_json::Map::new()),
                    other => other,
                };
                let arguments: A = serde_json::from_value(arguments)
                    .map_err(|error| AppError::Invalid(format!("{name}: {error}")))?;
                let answer = handler(context, arguments)?;
                serde_json::to_value(answer).map_err(|error| AppError::Internal(error.to_string()))
            }),
        );
        debug_assert!(previous.is_none(), "command `{name}` registered twice");
    }

    /// Calls a command.
    ///
    /// # Errors
    /// [`CallError`] with [`ErrorCode::NotFound`] for an unknown command, or the command's
    /// own failure.
    pub fn call(
        &self,
        context: &Context<'_>,
        name: &str,
        arguments: Value,
    ) -> Result<Value, CallError> {
        let handler = self.handlers.get(name).ok_or_else(|| CallError {
            code: ErrorCode::NotFound,
            message: format!("unknown command `{name}`"),
            retry_after: None,
        })?;
        handler(context, arguments).map_err(CallError::from)
    }

    /// Names of every command.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.handlers.keys().copied()
    }
}

/// The arguments of a command that takes none.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}
