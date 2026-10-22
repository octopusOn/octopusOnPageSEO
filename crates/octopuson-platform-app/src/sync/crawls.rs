//! Audit crawls travel as files beside the sync: after a sync, the computer that ran an audit
//! sends its crawl to the web, and a desktop takes the crawl of each project's latest audit
//! from the web when it does not have it. Older audits keep their details where they ran.
//!
//! A crawl travels as its folder's files, each as name length (2 bytes), name, size (8 bytes)
//! and bytes, compressed with zstd.

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::Path;

use octopuson_core::http::{HttpRequest, HttpTransport};

use super::REQUEST_TIMEOUT;
use crate::{AppError, Workspace};

/// The hub's address for crawls, followed by `/{audit uid}`.
pub const CRAWLS_PATH: &str = "/api/sync/crawls";

/// Media type of a packed crawl.
pub const CRAWL_TYPE: &str = "application/vnd.octopuson.crawl+zstd";

/// Largest packed crawl that travels.
pub const MAX_CRAWL: usize = 30 << 20;

/// Largest crawl once unpacked.
const MAX_UNPACKED: u64 = 1 << 31;

/// Most files a crawl folder has.
const MAX_FILES: usize = 32;

/// Space the web keeps for one account's crawls.
pub const ACCOUNT_CRAWLS: u64 = 5 << 30;

/// Value noting a crawl is on both sides.
const BOTH: &str = "both";

fn invalid(message: impl std::fmt::Display) -> AppError {
    AppError::Invalid(format!("crawl: {message}"))
}

fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The folder of crawl `id` in a workspace, when the id can name one.
pub fn crawl_dir(workspace: &Workspace, id: &str) -> Option<std::path::PathBuf> {
    plain_name(id).then(|| workspace.data_dir().join("crawls").join(id))
}

/// Packs a crawl folder.
///
/// # Errors
/// [`AppError::Invalid`] when it is larger than [`MAX_CRAWL`] packed; I/O errors.
pub fn pack(dir: &Path) -> Result<Vec<u8>, AppError> {
    let io = |error: std::io::Error| AppError::Internal(format!("{}: {error}", dir.display()));
    let mut encoder = zstd::Encoder::new(Vec::new(), 3).map_err(io)?;
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(io)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    if entries.len() > MAX_FILES {
        return Err(invalid("too many files"));
    }
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !plain_name(&name) {
            continue;
        }
        let bytes = fs::read(entry.path()).map_err(io)?;
        let length = u16::try_from(name.len()).map_err(|_| invalid("a long file name"))?;
        encoder.write_all(&length.to_le_bytes()).map_err(io)?;
        encoder.write_all(name.as_bytes()).map_err(io)?;
        encoder
            .write_all(&(bytes.len() as u64).to_le_bytes())
            .map_err(io)?;
        encoder.write_all(&bytes).map_err(io)?;
    }
    let packed = encoder.finish().map_err(io)?;
    if packed.len() > MAX_CRAWL {
        return Err(invalid("too large to travel"));
    }
    Ok(packed)
}

/// Unpacks a crawl into `dir`, which must not exist yet: written beside it first and moved in
/// place when complete.
///
/// # Errors
/// [`AppError::Invalid`] for data that is not a packed crawl; I/O errors.
pub fn unpack(bytes: &[u8], dir: &Path) -> Result<(), AppError> {
    let io = |error: std::io::Error| AppError::Internal(format!("{}: {error}", dir.display()));
    if dir.exists() {
        return Ok(());
    }
    let parent = dir.parent().ok_or_else(|| invalid("no folder"))?;
    fs::create_dir_all(parent).map_err(io)?;
    let temporary = parent.join(format!(
        ".incoming-{}",
        dir.file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    ));
    let _ = fs::remove_dir_all(&temporary);
    fs::create_dir(&temporary).map_err(io)?;
    let result = (|| {
        let mut decoder = zstd::Decoder::new(bytes)
            .map_err(invalid)?
            .take(MAX_UNPACKED);
        let mut files = 0;
        loop {
            let mut length = [0_u8; 2];
            match decoder.read_exact(&mut length) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(invalid(error)),
            }
            files += 1;
            if files > MAX_FILES {
                return Err(invalid("too many files"));
            }
            let mut name = vec![0_u8; usize::from(u16::from_le_bytes(length))];
            decoder.read_exact(&mut name).map_err(invalid)?;
            let name = String::from_utf8(name).map_err(invalid)?;
            if !plain_name(&name) {
                return Err(invalid("a file name that cannot be used"));
            }
            let mut size = [0_u8; 8];
            decoder.read_exact(&mut size).map_err(invalid)?;
            let size = u64::from_le_bytes(size);
            let mut file = fs::File::create(temporary.join(&name)).map_err(io)?;
            let copied = std::io::copy(&mut (&mut decoder).take(size), &mut file).map_err(io)?;
            if copied != size {
                return Err(invalid("cut short"));
            }
        }
        if !temporary.join("crawl.json").is_file() {
            return Err(invalid("no crawl in it"));
        }
        fs::rename(&temporary, dir).map_err(io)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

/// How much the workspace's crawls take, in bytes.
pub fn crawls_size(workspace: &Workspace) -> u64 {
    let Ok(folders) = fs::read_dir(workspace.data_dir().join("crawls")) else {
        return 0;
    };
    folders
        .filter_map(Result::ok)
        .filter_map(|folder| fs::read_dir(folder.path()).ok())
        .flat_map(|files| files.filter_map(Result::ok))
        .filter_map(|file| file.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

/// Moves at most one crawl each way: sends the oldest one not sent yet of an audit run here,
/// takes the latest audit's crawl of a project when it is not here.
///
/// # Errors
/// [`AppError`] when the web cannot be reached; storage errors.
pub(super) fn move_crawls(
    workspace: &Workspace,
    transport: &dyn HttpTransport,
    url: &str,
    token: &str,
) -> Result<(), AppError> {
    let audits = workspace.store()?.sync_audit_crawls()?;
    let mut sent = false;
    let mut taken = false;
    let mut latest: Vec<String> = Vec::new();
    for audit in audits.iter().rev() {
        let mark = format!("crawl:{}", audit.uid);
        let Some(dir) = crawl_dir(workspace, &audit.crawl_id) else {
            continue;
        };
        let newest_of_project = !latest.contains(&audit.project);
        latest.push(audit.project.clone());
        if workspace.store()?.sync_value(&mark)?.is_some() {
            continue;
        }
        let address = format!("{url}{CRAWLS_PATH}/{}", audit.uid);
        if dir.join("crawl.json").is_file() {
            if sent {
                continue;
            }
            sent = true;
            let outcome = match pack(&dir) {
                Ok(packed) => {
                    let mut request = HttpRequest::post(address, packed)
                        .header("Authorization", format!("Bearer {token}"))
                        .header("Content-Type", CRAWL_TYPE);
                    request.timeout = Some(REQUEST_TIMEOUT);
                    let response = transport
                        .send(&request)
                        .map_err(|error| AppError::Unavailable(format!("{url}: {error}")))?;
                    match response.status {
                        200..=299 => BOTH,
                        // Too large, or no room left on the web: the details stay here.
                        413 | 507 => "here",
                        // The audit is not on the web yet: next time.
                        404 => continue,
                        status => {
                            return Err(AppError::Unavailable(format!(
                                "the web version answered HTTP {status} to a crawl"
                            )));
                        }
                    }
                }
                Err(AppError::Invalid(_)) => "here",
                Err(error) => return Err(error),
            };
            workspace.store()?.set_sync_value(&mark, Some(outcome))?;
        } else if newest_of_project && !taken {
            taken = true;
            let mut request = HttpRequest::get(address)
                .header("Authorization", format!("Bearer {token}"))
                .header("Accept", CRAWL_TYPE);
            request.timeout = Some(REQUEST_TIMEOUT);
            let response = transport
                .send(&request)
                .map_err(|error| AppError::Unavailable(format!("{url}: {error}")))?;
            match response.status {
                200 => {
                    unpack(&response.body, &dir)?;
                    workspace.store()?.set_sync_value(&mark, Some(BOTH))?;
                }
                // Not on the web (too large, or run on another computer that has not sent it).
                404 => {}
                status => {
                    return Err(AppError::Unavailable(format!(
                        "the web version answered HTTP {status} to a crawl"
                    )));
                }
            }
        }
    }
    Ok(())
}
