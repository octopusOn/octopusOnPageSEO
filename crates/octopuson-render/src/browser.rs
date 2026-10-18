//! A private, headless browser process; never reuses the user's browser profile.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::RenderError;

/// Owned process and temporary profile. Drop kills/waits before removing the profile.
pub(crate) struct Browser {
    child: Child,
    profile: tempfile::TempDir,
    pub(crate) endpoint: String,
}

impl Browser {
    pub(crate) fn launch(explicit: Option<&str>) -> Result<Self, RenderError> {
        let binary = discover(explicit).ok_or(RenderError::BrowserMissing)?;
        let profile = tempfile::Builder::new()
            .prefix("octopusseo-render-")
            .tempdir()?;
        let mut command = Command::new(&binary);
        command
            .args([
                "--headless=new",
                "--remote-debugging-address=127.0.0.1",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-sync",
                "--disable-component-update",
                "--disable-extensions",
            ])
            .arg(format!("--user-data-dir={}", profile.path().display()))
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn().map_err(|source| RenderError::Launch {
            path: binary,
            source,
        })?;
        let mut browser = Self {
            child,
            profile,
            endpoint: String::new(),
        };
        browser.endpoint = wait_for_endpoint(&mut browser.child, browser.profile.path())?;
        Ok(browser)
    }
}

/// Waits until a launched browser publishes its DevTools endpoint in `profile`.
pub(crate) fn wait_for_endpoint(child: &mut Child, profile: &Path) -> Result<String, RenderError> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(RenderError::Startup(format!(
                "browser exited with {status}"
            )));
        }
        if let Ok(contents) = std::fs::read_to_string(profile.join("DevToolsActivePort")) {
            let mut lines = contents.lines();
            let port = lines.next().and_then(|line| line.parse::<u16>().ok());
            let path = lines.next().filter(|path| {
                path.starts_with("/devtools/browser/") && !path.chars().any(char::is_whitespace)
            });
            if let (Some(port), Some(path)) = (port, path)
                && port != 0
            {
                return Ok(format!("ws://127.0.0.1:{port}{path}"));
            }
        }
        if Instant::now() >= deadline {
            return Err(RenderError::Startup(
                "browser did not publish a DevTools endpoint within 15 seconds".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) fn discover(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = explicit.filter(|path| !path.trim().is_empty()) {
        return Path::new(path).is_file().then(|| PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("OCTOPUSSEO_CHROME_PATH") {
        return Path::new(&path).is_file().then(|| PathBuf::from(path));
    }
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    for root in [
        Some(PathBuf::from("/Applications")),
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications")),
    ]
    .into_iter()
    .flatten()
    {
        for app in [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ] {
            candidates.push(root.join(app));
        }
    }
    #[cfg(target_os = "windows")]
    for root in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
    {
        for app in [
            "Google/Chrome/Application/chrome.exe",
            "Microsoft/Edge/Application/msedge.exe",
            "Chromium/Application/chrome.exe",
        ] {
            candidates.push(PathBuf::from(&root).join(app));
        }
    }
    for root in std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        for name in [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "microsoft-edge",
            "chrome.exe",
            "msedge.exe",
        ] {
            candidates.push(root.join(name));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}
