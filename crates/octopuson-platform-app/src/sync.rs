//! Keeping the desktop and the account's web workspace the same (ADR 0078).
//!
//! The web workspace is the hub: the web server answers each copy's request with
//! [`hub_answer`]. A desktop signed in to the account is a copy: it runs a [`round`] every
//! half minute and shortly after the person changes something. A round first greets the hub
//! (who it is, which account, how many projects), then sends the copy's own changes, then
//! takes the others' (the store's `sync` module decides what wins). Requests and answers are
//! JSON compressed with zstd.
//!
//! The first sync of a computer that has projects with an account that has projects too, a
//! different account, and a hub whose data was reset all wait for the person's choice
//! ([`choose`]): merge, put this computer's data on the web, take the web's data here, or
//! pause. While a desktop syncs, scheduled work runs on the web server only
//! ([`Workspace::schedules_here`]), so nothing is paid twice.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use octopuson_core::http::{HttpRequest, HttpTransport};
use octopuson_crawl_store::CrawlLibrary;
use octopuson_platform::{
    PlatformStore, SyncAck, SyncApplied, SyncChange, SyncSide, first_sync_kinds, sync_kind_versions,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{AppError, Host, Workspace};

/// Environment variable naming another web address to sync with (tests, a local server).
pub const SYNC_URL_VARIABLE: &str = "PLATFORM_SYNC_URL";

/// The web version desktops sync with.
pub const DEFAULT_SYNC_URL: &str = "https://app.octopuson.com";

/// The hub's address for sync requests.
pub const SYNC_PATH: &str = "/api/sync";

/// Media type of requests and answers: JSON compressed with zstd.
pub const CONTENT_TYPE: &str = "application/vnd.octopuson.sync+zstd";

/// Version of the requests; both sides must speak the same one.
pub const PROTOCOL: u32 = 1;

/// Largest request or answer once uncompressed.
pub const MAX_BODY: usize = 128 << 20;

/// About how much JSON one request or answer carries.
const BATCH_BYTES: usize = 4_000_000;

/// How long one request may take: a first sync sends a lot.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Changes taken in one go before the data is let go of, so other work is not kept waiting.
const APPLY_SLICE: usize = 200;

/// Values a copy keeps in `sync_state` (the hub keeps only its epoch).
#[allow(unreachable_pub)] // a namespace for the names, private to this file
mod names {
    pub const REPLICA: &str = "replica";
    pub const EPOCH: &str = "epoch";
    pub const CURSOR: &str = "cursor";
    pub const PUSHED: &str = "pushed";
    pub const ACCOUNT: &str = "account";
    pub const OFFSET: &str = "offset";
    pub const CHOICE: &str = "choice";
    pub const OFF: &str = "off";
    pub const REPLACE: &str = "replace";
    pub const LAST_AT: &str = "last_at";
    pub const LAST_ERROR: &str = "last_error";
    pub const KINDS: &str = "kinds";
}

/// A copy's request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRequest {
    /// [`PROTOCOL`].
    pub protocol: u32,
    /// The copy.
    pub replica: String,
    /// The hub's epoch the copy knows; none before the first sync.
    #[serde(default)]
    pub epoch: Option<String>,
    /// The hub's last change the copy took.
    #[serde(default)]
    pub cursor: i64,
    /// The copy's changes.
    #[serde(default)]
    pub changes: Vec<SyncChange>,
    /// No more changes follow in this sync.
    #[serde(default)]
    pub last: bool,
    /// Send the hub's changes back.
    #[serde(default)]
    pub pull: bool,
    /// Delete the hub's data first: the copy's data replaces it.
    #[serde(default)]
    pub replace: bool,
    /// Kinds to send again in full: the copy learnt them (or their new columns) after it took
    /// their records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resend: Vec<String>,
    /// The hub's change after which to continue sending them.
    #[serde(default)]
    pub resend_after: i64,
}

/// The hub's answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncResponse {
    /// The hub's epoch: it changes only when its sync records start over.
    pub epoch: String,
    /// The account the token belongs to.
    pub account: String,
    /// The copy's epoch or cursor does not match the hub's: nothing was done.
    #[serde(default)]
    pub reset: bool,
    /// Changes taken as sent, with the hub's numbers.
    #[serde(default)]
    pub accepted: Vec<SyncAck>,
    /// The hub's versions of records whose change from the copy lost.
    #[serde(default)]
    pub rejected: Vec<SyncChange>,
    /// The hub's changes after the cursor.
    #[serde(default)]
    pub changes: Vec<SyncChange>,
    /// Where to continue taking changes.
    pub cursor: i64,
    /// More changes wait.
    #[serde(default)]
    pub more: bool,
    /// How many projects the hub holds.
    pub projects: u64,
    /// The hub's clock, Unix milliseconds.
    pub now_ms: i64,
    /// Records of the kinds asked again.
    #[serde(default)]
    pub resent: Vec<SyncChange>,
    /// The last change looked at among them.
    #[serde(default)]
    pub resend_last: i64,
    /// More of them wait.
    #[serde(default)]
    pub resend_more: bool,
}

/// Why a sync waits for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// `first` (both sides hold projects), `account` (another account is signed in) or `reset`
    /// (the web's sync records started over).
    pub reason: String,
    /// Projects on this computer.
    pub here: u64,
    /// Projects on the web.
    pub web: u64,
    /// The account met.
    pub account: String,
    /// The hub's epoch met.
    pub epoch: String,
}

/// What a round did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RoundReport {
    /// Changes the web took.
    pub sent: usize,
    /// Records written here.
    pub received: usize,
}

/// How a round ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundOutcome {
    /// It ran.
    Done(RoundReport),
    /// Syncing is switched off.
    Off,
    /// No account is signed in.
    SignedOut,
    /// It waits for the person's choice.
    Choose(Choice),
}

/// Where syncing stands, for the Settings page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)] // independent facts the page shows
pub struct SyncStatus {
    /// Where the program runs: only desktops sync.
    pub host: Host,
    /// Not switched off.
    pub enabled: bool,
    /// An account is signed in.
    pub signed_in: bool,
    /// Syncing with the account: scheduled work runs on the web.
    pub active: bool,
    /// A round runs now.
    pub syncing: bool,
    /// The choice it waits for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<Choice>,
    /// When the last round ended well, Unix milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_at: Option<i64>,
    /// Why the last round failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Own changes not sent yet.
    pub unsent: u64,
    /// Changes waiting for a record they refer to.
    pub parked: u64,
    /// The web address synced with.
    pub url: String,
}

/// A workspace's sync in this process: one round at a time, and a counter of changes made
/// through commands, which a host watches to sync soon after.
#[derive(Debug, Default)]
pub(crate) struct SyncRuntime {
    round: Mutex<()>,
    generation: AtomicU64,
    running: AtomicBool,
}

impl Workspace {
    /// Notes that a command may have changed data (hosts sync soon after).
    pub fn touch(&self) {
        self.sync.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Grows with every change made through commands.
    pub fn generation(&self) -> u64 {
        self.sync.generation.load(Ordering::Relaxed)
    }

    /// Whether scheduled work runs here: always on the web; on a desktop unless it syncs with
    /// the account, whose web workspace then runs it.
    pub fn schedules_here(&self) -> bool {
        self.host == Host::Web || !self.syncing_with_account()
    }

    /// Whether this desktop syncs with the account: switched on, signed in, synced once and
    /// not waiting for a choice.
    pub fn syncing_with_account(&self) -> bool {
        if self.host == Host::Web || self.account_token().is_none() {
            return false;
        }
        self.store().is_ok_and(|store| {
            let value = |name| store.sync_value(name).ok().flatten();
            value(names::OFF).is_none()
                && value(names::CHOICE).is_none()
                && value(names::ACCOUNT).is_some()
        })
    }
}

/// The web address to sync with: [`SYNC_URL_VARIABLE`] when set, else the public one.
///
/// # Errors
/// [`AppError::Invalid`] for an address that is not https (or http on this computer).
pub fn sync_url() -> Result<String, AppError> {
    let value = std::env::var(SYNC_URL_VARIABLE).ok();
    let Some(value) = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(DEFAULT_SYNC_URL.to_owned());
    };
    let url = url::Url::parse(value)
        .map_err(|error| AppError::Invalid(format!("{SYNC_URL_VARIABLE}: {error}")))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err(AppError::Invalid(format!(
            "{SYNC_URL_VARIABLE} must be an https address"
        )));
    }
    Ok(value.trim_end_matches('/').to_owned())
}

/// JSON compressed with zstd.
///
/// # Errors
/// [`AppError::Internal`] when it cannot be written.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, AppError> {
    let json = serde_json::to_vec(value).map_err(|error| AppError::Internal(error.to_string()))?;
    zstd::encode_all(json.as_slice(), 3).map_err(|error| AppError::Internal(error.to_string()))
}

/// Reads JSON compressed with zstd, at most [`MAX_BODY`] once uncompressed.
///
/// # Errors
/// [`AppError::Invalid`] for anything else.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, AppError> {
    let invalid = |error: &dyn std::fmt::Display| AppError::Invalid(format!("sync data: {error}"));
    let decoder = zstd::Decoder::new(bytes).map_err(|error| invalid(&error))?;
    let mut json = Vec::new();
    decoder
        .take(u64::try_from(MAX_BODY).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut json)
        .map_err(|error| invalid(&error))?;
    if json.len() > MAX_BODY {
        return Err(AppError::Invalid("sync data: too large".to_owned()));
    }
    serde_json::from_slice(&json).map_err(|error| invalid(&error))
}

/// Unix milliseconds now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// A random id: 16 bytes in hex.
fn random_id() -> Result<String, AppError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(bytes
        .iter()
        .fold(String::with_capacity(32), |mut id, byte| {
            let _ = write!(id, "{byte:02x}");
            id
        }))
}

fn valid_replica(replica: &str) -> bool {
    (8..=64).contains(&replica.len())
        && replica
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Deletes the crawl folders of audits that were deleted.
fn remove_crawls(workspace: &Workspace, crawls: &[String]) {
    let library = CrawlLibrary::new(workspace.data_dir());
    for crawl in crawls {
        if let Err(error) = library.delete(crawl) {
            tracing::debug!(%error, "a deleted audit's crawl was not there");
        }
    }
}

/// What answering a copy did besides the answer.
#[derive(Debug, Clone, PartialEq)]
pub struct HubAnswer {
    /// The answer.
    pub response: SyncResponse,
    /// Report links that stopped working (the server forgets them).
    pub removed_shares: Vec<String>,
    /// Data changed here (the server notes the schedule again and tells open pages).
    pub changed: bool,
}

/// The hub's epoch, made on first use.
fn hub_epoch(store: &PlatformStore) -> Result<String, AppError> {
    if let Some(epoch) = store.sync_value(names::EPOCH)? {
        return Ok(epoch);
    }
    let epoch = random_id()?;
    store.set_sync_value(names::EPOCH, Some(&epoch))?;
    Ok(epoch)
}

/// The hub's side: answers one request of a copy of `account`'s workspace.
///
/// # Errors
/// [`AppError::Invalid`] for a request it cannot take; storage errors.
pub fn hub_answer(
    workspace: &Workspace,
    account: &str,
    request: &SyncRequest,
    now_ms: i64,
) -> Result<HubAnswer, AppError> {
    if request.protocol != PROTOCOL {
        return Err(AppError::Invalid(
            "this program syncs differently from the web version: update it".to_owned(),
        ));
    }
    if !valid_replica(&request.replica) {
        return Err(AppError::Invalid("replica: not a valid id".to_owned()));
    }
    let (epoch, head, projects) = {
        let store = workspace.store()?;
        (
            hub_epoch(&store)?,
            store.sync_head()?,
            store.project_count()?,
        )
    };
    let mut response = SyncResponse {
        epoch: epoch.clone(),
        account: account.to_owned(),
        cursor: request.cursor,
        projects,
        now_ms,
        ..SyncResponse::default()
    };
    if request.epoch.as_deref().is_some_and(|known| known != epoch) || request.cursor > head {
        response.reset = true;
        return Ok(HubAnswer {
            response,
            removed_shares: Vec::new(),
            changed: false,
        });
    }
    let mut crawls = Vec::new();
    {
        let store = workspace.store()?;
        if request.replace {
            crawls.extend(store.sync_clear()?);
        }
        store.sync_scan(now_ms)?;
    }
    let side = SyncSide::Hub {
        replica: &request.replica,
    };
    let mut applied = SyncApplied::default();
    let slices: Vec<&[SyncChange]> = request.changes.chunks(APPLY_SLICE).collect();
    let count = slices.len();
    for (index, slice) in slices.into_iter().enumerate() {
        let finish = request.last && index + 1 == count;
        applied.absorb(workspace.store()?.sync_apply(side, slice, now_ms, finish)?);
    }
    if count == 0 && request.last {
        applied.absorb(workspace.store()?.sync_apply(side, &[], now_ms, true)?);
    }
    let store = workspace.store()?;
    response.rejected = store.sync_versions(&applied.rejected)?;
    if request.pull {
        let batch = store.sync_hub_changes(request.cursor, &request.replica, BATCH_BYTES)?;
        response.changes = batch.changes;
        response.cursor = batch.last.max(request.cursor);
        response.more = batch.more;
    }
    if !request.resend.is_empty() {
        let batch = store.sync_kind_records(&request.resend, request.resend_after, BATCH_BYTES)?;
        response.resent = batch.changes;
        response.resend_last = batch.last.max(request.resend_after);
        response.resend_more = batch.more;
    }
    response.projects = store.project_count()?;
    drop(store);
    response.accepted = std::mem::take(&mut applied.accepted);
    crawls.extend(applied.removed_crawls);
    remove_crawls(workspace, &crawls);
    Ok(HubAnswer {
        response,
        removed_shares: applied.removed_shares,
        changed: applied.applied > 0 || request.replace,
    })
}

/// Sends one request to the hub.
fn post(
    transport: &dyn HttpTransport,
    url: &str,
    token: &str,
    request: &SyncRequest,
) -> Result<SyncResponse, AppError> {
    let mut http = HttpRequest::post(format!("{url}{SYNC_PATH}"), encode(request)?)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", CONTENT_TYPE)
        .header("Accept", CONTENT_TYPE);
    http.timeout = Some(REQUEST_TIMEOUT);
    let response = transport
        .send(&http)
        .map_err(|error| AppError::Unavailable(format!("{url}: {error}")))?;
    match response.status {
        200 => decode(&response.body),
        401 => Err(AppError::Account(
            "the account's sign-in is no longer valid: sign in again".to_owned(),
        )),
        413 => Err(AppError::Invalid(
            "a record is too large to sync".to_owned(),
        )),
        429 => Err(AppError::RateLimited {
            retry_after: response.retry_after(),
        }),
        400 => Err(AppError::Invalid(format!(
            "the web version refused the sync: {}",
            response.text().chars().take(200).collect::<String>()
        ))),
        status => Err(AppError::Unavailable(format!(
            "the web version answered HTTP {status}"
        ))),
    }
}

/// A copy's numbers kept between rounds.
fn number(store: &PlatformStore, name: &str) -> Result<i64, AppError> {
    Ok(store
        .sync_value(name)?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0))
}

/// The copy's id, made on first use.
fn replica_id(store: &PlatformStore) -> Result<String, AppError> {
    if let Some(replica) = store.sync_value(names::REPLICA)? {
        return Ok(replica);
    }
    let replica = random_id()?;
    store.set_sync_value(names::REPLICA, Some(&replica))?;
    Ok(replica)
}

/// Runs one sync of this desktop with the account's web workspace.
///
/// # Errors
/// [`AppError`] when the web cannot be reached or refuses, or for storage errors; the caller
/// notes it with [`note_failure`].
#[allow(clippy::too_many_lines)] // greeting, sending and taking, in order
pub fn round(workspace: &Workspace) -> Result<RoundOutcome, AppError> {
    if workspace.host() == Host::Web {
        return Err(AppError::Unsupported(
            "the web version is the account's workspace itself".to_owned(),
        ));
    }
    let _round = workspace
        .sync
        .round
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    workspace.sync.running.store(true, Ordering::Relaxed);
    let outcome = run_round(workspace);
    workspace.sync.running.store(false, Ordering::Relaxed);
    outcome
}

#[allow(clippy::too_many_lines)] // greeting, sending and taking, in order
fn run_round(workspace: &Workspace) -> Result<RoundOutcome, AppError> {
    let Some(token) = workspace.account_token() else {
        return Ok(RoundOutcome::SignedOut);
    };
    let url = sync_url()?;
    let started = now_ms();
    let (replica, epoch, account, mut cursor, mut pushed, here) = {
        let store = workspace.store()?;
        if store.sync_value(names::OFF)?.is_some() {
            return Ok(RoundOutcome::Off);
        }
        if let Some(choice) = store.sync_value(names::CHOICE)? {
            let choice: Choice = serde_json::from_str(&choice)
                .map_err(|error| AppError::Internal(error.to_string()))?;
            return Ok(RoundOutcome::Choose(choice));
        }
        // Kinds that gained columns in an update: what they gained is not a change made here.
        store.sync_rebaseline(&missing_kinds(&store, number(&store, names::CURSOR)?)?)?;
        store.sync_scan(started)?;
        (
            replica_id(&store)?,
            store.sync_value(names::EPOCH)?,
            store.sync_value(names::ACCOUNT)?,
            number(&store, names::CURSOR)?,
            number(&store, names::PUSHED)?,
            store.project_count()?,
        )
    };
    let transport = workspace.free_transport()?;
    let request = |changes: Vec<SyncChange>, cursor: i64, last: bool, pull: bool| SyncRequest {
        protocol: PROTOCOL,
        replica: replica.clone(),
        epoch: epoch.clone(),
        cursor,
        changes,
        last,
        pull,
        replace: false,
        resend: Vec::new(),
        resend_after: 0,
    };
    let hello = post(
        &transport,
        &url,
        &token,
        &request(Vec::new(), cursor, false, false),
    )?;
    let reason = if hello.reset && epoch.is_some() {
        Some("reset")
    } else if account
        .as_ref()
        .is_some_and(|known| *known != hello.account)
    {
        Some("account")
    } else if account.is_none() && here > 0 && hello.projects > 0 {
        Some("first")
    } else {
        None
    };
    let store = workspace.store()?;
    if let Some(reason) = reason {
        let choice = Choice {
            reason: reason.to_owned(),
            here,
            web: hello.projects,
            account: hello.account,
            epoch: hello.epoch,
        };
        let text = serde_json::to_string(&choice)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        store.set_sync_value(names::CHOICE, Some(&text))?;
        return Ok(RoundOutcome::Choose(choice));
    }
    let offset = hello.now_ms - now_ms();
    store.set_sync_value(names::ACCOUNT, Some(&hello.account))?;
    store.set_sync_value(names::EPOCH, Some(&hello.epoch))?;
    store.set_sync_value(names::OFFSET, Some(&offset.to_string()))?;
    let replace = store.sync_value(names::REPLACE)?.is_some();
    drop(store);
    let epoch = Some(hello.epoch.clone());
    let request = |changes: Vec<SyncChange>, cursor: i64, last: bool, pull: bool| SyncRequest {
        protocol: PROTOCOL,
        replica: replica.clone(),
        epoch: epoch.clone(),
        cursor,
        changes,
        last,
        pull,
        replace: false,
        resend: Vec::new(),
        resend_after: 0,
    };
    if replace {
        let mut wipe = request(Vec::new(), cursor, false, false);
        wipe.replace = true;
        post(&transport, &url, &token, &wipe)?;
        workspace.store()?.set_sync_value(names::REPLACE, None)?;
    }
    let mut report = RoundReport::default();
    let mut crawls: Vec<String> = Vec::new();
    // Send this computer's changes first, so the web decides on them before answering.
    loop {
        let batch = workspace
            .store()?
            .sync_local_changes(pushed, offset, BATCH_BYTES)?;
        if batch.changes.is_empty() && !batch.more {
            if batch.last > pushed {
                pushed = batch.last;
                workspace
                    .store()?
                    .set_sync_value(names::PUSHED, Some(&pushed.to_string()))?;
            }
            break;
        }
        let more = batch.more;
        let answer = post(
            &transport,
            &url,
            &token,
            &request(batch.changes, cursor, !more, false),
        )?;
        if answer.reset {
            break;
        }
        pushed = batch.last.max(pushed);
        report.sent += answer.accepted.len();
        let store = workspace.store()?;
        store.set_sync_value(names::PUSHED, Some(&pushed.to_string()))?;
        store.sync_acknowledge(&answer.accepted)?;
        let applied =
            store.sync_apply(SyncSide::Copy { pushed }, &answer.rejected, now_ms(), false)?;
        report.received += applied.applied;
        crawls.extend(applied.removed_crawls);
        if !more {
            break;
        }
    }
    // Then take everyone else's.
    loop {
        let answer = post(
            &transport,
            &url,
            &token,
            &request(Vec::new(), cursor, false, true),
        )?;
        if answer.reset {
            break;
        }
        let slices: Vec<&[SyncChange]> = answer.changes.chunks(APPLY_SLICE).collect();
        let count = slices.len();
        for (index, slice) in slices.into_iter().enumerate() {
            let finish = !answer.more && index + 1 == count;
            let applied = workspace.store()?.sync_apply(
                SyncSide::Copy { pushed },
                slice,
                now_ms(),
                finish,
            )?;
            report.received += applied.applied;
            crawls.extend(applied.removed_crawls);
        }
        cursor = answer.cursor;
        workspace
            .store()?
            .set_sync_value(names::CURSOR, Some(&cursor.to_string()))?;
        if !answer.more {
            break;
        }
    }
    // Then the kinds this program learnt since it last synced, which it skipped before.
    let missing = missing_kinds(&workspace.store()?, cursor)?;
    if !missing.is_empty() {
        let mut after = 0;
        loop {
            let mut ask = request(Vec::new(), cursor, false, false);
            ask.resend.clone_from(&missing);
            ask.resend_after = after;
            let answer = post(&transport, &url, &token, &ask)?;
            if answer.reset {
                break;
            }
            let slices: Vec<&[SyncChange]> = answer.resent.chunks(APPLY_SLICE).collect();
            let count = slices.len();
            for (index, slice) in slices.into_iter().enumerate() {
                let finish = !answer.resend_more && index + 1 == count;
                let applied = workspace.store()?.sync_apply(
                    SyncSide::Copy { pushed },
                    slice,
                    now_ms(),
                    finish,
                )?;
                report.received += applied.applied;
                crawls.extend(applied.removed_crawls);
            }
            after = answer.resend_last;
            if !answer.resend_more {
                break;
            }
        }
    }
    note_kinds(&workspace.store()?)?;
    remove_crawls(workspace, &crawls);
    // Crawls are large: a failure to move one does not fail the sync.
    if let Err(error) = crawls::move_crawls(workspace, &transport, &url, &token) {
        tracing::warn!(%error, "a crawl did not travel");
    }
    let store = workspace.store()?;
    // What was taken is not a change of this computer's.
    store.sync_scan(now_ms())?;
    store.set_sync_value(names::LAST_AT, Some(&now_ms().to_string()))?;
    store.set_sync_value(names::LAST_ERROR, None)?;
    Ok(RoundOutcome::Done(report))
}

/// The kinds a copy has not taken in full yet: those this program knows newer than what the
/// copy noted at its last sync. A copy that never synced takes everything anyway.
fn missing_kinds(store: &PlatformStore, cursor: i64) -> Result<Vec<String>, AppError> {
    let known: HashMap<String, u32> = match store.sync_value(names::KINDS)? {
        Some(text) => serde_json::from_str(&text).unwrap_or_default(),
        None if cursor > 0 => first_sync_kinds()
            .into_iter()
            .map(|(name, version)| (name.to_owned(), version))
            .collect(),
        None => return Ok(Vec::new()),
    };
    Ok(sync_kind_versions()
        .into_iter()
        .filter(|(name, version)| known.get(*name).copied().unwrap_or(0) < *version)
        .map(|(name, _)| name.to_owned())
        .collect())
}

/// Notes that this copy now holds every kind this program knows, at its version.
fn note_kinds(store: &PlatformStore) -> Result<(), AppError> {
    let kinds: HashMap<&str, u32> = sync_kind_versions().into_iter().collect();
    let text =
        serde_json::to_string(&kinds).map_err(|error| AppError::Internal(error.to_string()))?;
    store.set_sync_value(names::KINDS, Some(&text))?;
    Ok(())
}

/// Notes the changes made here so far, with the time now (the program is closing: they keep
/// this time rather than the next start's).
pub fn note_changes(workspace: &Workspace) {
    if workspace.host() == Host::Desktop
        && let Ok(store) = workspace.store()
        && let Err(error) = store.sync_scan(now_ms())
    {
        tracing::warn!(%error, "changes were not noted for syncing");
    }
}

/// Notes why a round failed, for the Settings page.
pub fn note_failure(workspace: &Workspace, error: &AppError) {
    if let Ok(store) = workspace.store() {
        let _ = store.set_sync_value(names::LAST_ERROR, Some(&error.to_string()));
    }
}

/// Answers the choice a sync waits for: `merge`, `upload` (this computer's data replaces the
/// web's), `download` (the web's data replaces this computer's) or `pause`.
///
/// # Errors
/// [`AppError::Invalid`] for an unknown choice; storage errors.
pub fn choose(workspace: &Workspace, choice: &str) -> Result<(), AppError> {
    let _round = workspace
        .sync
        .round
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let store = workspace.store()?;
    if choice == "pause" {
        store.set_sync_value(names::OFF, Some("1"))?;
        store.set_sync_value(names::CHOICE, None)?;
        return Ok(());
    }
    let Some(waiting) = store.sync_value(names::CHOICE)? else {
        return Err(AppError::Invalid("nothing waits for a choice".to_owned()));
    };
    let waiting: Choice =
        serde_json::from_str(&waiting).map_err(|error| AppError::Internal(error.to_string()))?;
    let keep = [names::REPLICA, names::OFF];
    match choice {
        "merge" => {
            if waiting.reason != "first" {
                store.sync_forget(&keep)?;
            }
        }
        "upload" => {
            store.sync_forget(&keep)?;
            store.set_sync_value(names::REPLACE, Some("1"))?;
        }
        "download" => {
            let crawls = store.sync_clear()?;
            store.sync_forget(&keep)?;
            remove_crawls(workspace, &crawls);
        }
        _ => {
            return Err(AppError::Invalid(
                "choice: merge, upload, download or pause".to_owned(),
            ));
        }
    }
    store.set_sync_value(names::ACCOUNT, Some(&waiting.account))?;
    store.set_sync_value(names::EPOCH, Some(&waiting.epoch))?;
    store.set_sync_value(names::CHOICE, None)?;
    Ok(())
}

/// Switches syncing on or off.
///
/// # Errors
/// Storage errors.
pub fn set_enabled(workspace: &Workspace, enabled: bool) -> Result<(), AppError> {
    let store = workspace.store()?;
    store.set_sync_value(names::OFF, (!enabled).then_some("1"))?;
    if enabled {
        store.set_sync_value(names::LAST_ERROR, None)?;
    }
    Ok(())
}

/// Where syncing stands.
///
/// # Errors
/// Storage errors.
pub fn status(workspace: &Workspace) -> Result<SyncStatus, AppError> {
    let url = sync_url().unwrap_or_else(|_| DEFAULT_SYNC_URL.to_owned());
    let signed_in = workspace.account_token().is_some();
    let syncing = workspace.sync.running.load(Ordering::Relaxed);
    if workspace.host() == Host::Web {
        return Ok(SyncStatus {
            host: Host::Web,
            enabled: false,
            signed_in,
            active: false,
            syncing: false,
            choice: None,
            last_at: None,
            last_error: None,
            unsent: 0,
            parked: 0,
            url,
        });
    }
    let store = workspace.store()?;
    let choice = store
        .sync_value(names::CHOICE)?
        .and_then(|text| serde_json::from_str(&text).ok());
    let enabled = store.sync_value(names::OFF)?.is_none();
    let synced = store.sync_value(names::ACCOUNT)?.is_some();
    let pushed = number(&store, names::PUSHED)?;
    Ok(SyncStatus {
        host: Host::Desktop,
        enabled,
        signed_in,
        active: enabled && signed_in && synced && choice.is_none(),
        syncing,
        choice,
        last_at: store
            .sync_value(names::LAST_AT)?
            .and_then(|value| value.parse().ok()),
        last_error: store.sync_value(names::LAST_ERROR)?,
        unsent: store.sync_unsent(pushed)?,
        parked: store.sync_parked()?,
        url,
    })
}

pub mod crawls;

#[cfg(test)]
mod tests;
