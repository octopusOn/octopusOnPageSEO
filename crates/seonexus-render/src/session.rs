//! A browser window the person using the program can see and act in.
//!
//! Unlike the crawl renderer, this opens the browser in a normal window with a persistent
//! profile of its own (never the user's everyday profile), so a consent choice or a solved
//! check stays valid across searches, and a person can step in when a site asks them to. It
//! loads pages through the browser's own network stack and reads the resulting document. It
//! changes nothing about how the browser presents itself: no user-agent override, no
//! automation flags, no script injected into pages, no request interception.

use std::fs::{self, File, TryLockError};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::RenderError;
use crate::browser::{discover, wait_for_endpoint};

/// How the window is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOptions {
    /// Chrome or Edge executable; found automatically when `None`.
    pub executable: Option<String>,
    /// Profile directory of this program (created when missing).
    pub profile_dir: PathBuf,
    /// Single proxy the person configured (`http://host:port`).
    pub proxy: Option<String>,
    /// Interface language (`tr`), which also sets `Accept-Language`.
    pub language: Option<String>,
    /// Window size in pixels.
    pub window: (u32, u32),
    /// Whether the window is shown; `false` only for automated tests.
    pub visible: bool,
}

impl BrowserOptions {
    /// A visible 1280×900 window with the profile in `profile_dir`.
    pub fn new(profile_dir: PathBuf) -> Self {
        Self {
            executable: None,
            profile_dir,
            proxy: None,
            language: None,
            window: (1280, 900),
            visible: true,
        }
    }
}

/// A document as the browser shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPage {
    /// Address after redirects.
    pub url: String,
    /// Document title.
    pub title: String,
    /// Serialized DOM (`document.documentElement.outerHTML`).
    pub html: String,
}

/// An open browser window driven over DevTools.
pub struct BrowserSession {
    child: Child,
    socket: WebSocket<TcpStream>,
    session_id: String,
    next_id: u64,
    _lock: File,
}

impl std::fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSession")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl BrowserSession {
    /// Opens the browser with the program's profile.
    ///
    /// # Errors
    /// [`RenderError::Busy`] while another window uses the profile; a missing browser,
    /// launch or DevTools failures otherwise.
    pub fn launch(options: &BrowserOptions) -> Result<Self, RenderError> {
        let binary = discover(options.executable.as_deref()).ok_or(RenderError::BrowserMissing)?;
        fs::create_dir_all(&options.profile_dir)?;
        let lock = lock_profile(&options.profile_dir)?;
        // A previous run leaves its port behind; only the new one may be read.
        match fs::remove_file(options.profile_dir.join("DevToolsActivePort")) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut command = Command::new(&binary);
        command
            .args([
                "--remote-debugging-address=127.0.0.1",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .arg(format!("--user-data-dir={}", options.profile_dir.display()))
            .arg(format!(
                "--window-size={},{}",
                options.window.0, options.window.1
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if !options.visible {
            command.arg("--headless=new");
        }
        if let Some(language) = options
            .language
            .as_deref()
            .filter(|language| !language.is_empty())
        {
            command.arg(format!("--lang={language}"));
        }
        if let Some(proxy) = options
            .proxy
            .as_deref()
            .filter(|proxy| !proxy.trim().is_empty())
        {
            command.arg(format!("--proxy-server={}", proxy.trim()));
        }
        command.arg("about:blank");
        let mut child = command.spawn().map_err(|source| RenderError::Launch {
            path: binary,
            source,
        })?;
        let endpoint = match wait_for_endpoint(&mut child, &options.profile_dir) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let socket = match connect(&endpoint) {
            Ok(socket) => socket,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let mut session = Self {
            child,
            socket,
            session_id: String::new(),
            next_id: 0,
            _lock: lock,
        };
        session.attach()?;
        Ok(session)
    }

    fn attach(&mut self) -> Result<(), RenderError> {
        let targets = self.call("Target.getTargets", &json!({}), false)?;
        let target = targets["targetInfos"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|target| target["type"] == "page")
            .and_then(|target| target["targetId"].as_str())
            .map(str::to_owned);
        let target = match target {
            Some(target) => target,
            None => {
                self.call("Target.createTarget", &json!({"url": "about:blank"}), false)?["targetId"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| protocol("no page target"))?
            }
        };
        let attached = self.call(
            "Target.attachToTarget",
            &json!({"targetId": target, "flatten": true}),
            false,
        )?;
        attached["sessionId"]
            .as_str()
            .ok_or_else(|| protocol("attaching to the page failed"))?
            .clone_into(&mut self.session_id);
        self.call("Page.enable", &json!({}), true)?;
        Ok(())
    }

    /// Loads `url` and returns the document once it has loaded and `settle` has passed.
    ///
    /// # Errors
    /// [`RenderError::Browser`] when the address cannot be loaded or the page does not finish
    /// loading within `timeout`.
    pub fn navigate(
        &mut self,
        url: &str,
        timeout: Duration,
        settle: Duration,
    ) -> Result<BrowserPage, RenderError> {
        let deadline = Instant::now() + timeout;
        let navigated = self.call("Page.navigate", &json!({"url": url}), true)?;
        if let Some(error) = navigated["errorText"]
            .as_str()
            .filter(|error| !error.is_empty())
        {
            return Err(RenderError::Browser(format!("cannot load {url}: {error}")));
        }
        loop {
            let state = self.evaluate("document.readyState")?;
            if state == "complete" {
                break;
            }
            if Instant::now() >= deadline {
                return Err(RenderError::Browser(format!(
                    "{url} did not finish loading within {} seconds",
                    timeout.as_secs()
                )));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        std::thread::sleep(settle);
        self.current()
    }

    /// The document the window shows now (after the person interacted with it, say).
    ///
    /// # Errors
    /// [`RenderError::Browser`] for DevTools failures.
    pub fn current(&mut self) -> Result<BrowserPage, RenderError> {
        let value = self.evaluate(
            "JSON.stringify({url: location.href, title: document.title, html: document.documentElement ? document.documentElement.outerHTML : ''})",
        )?;
        let text = value
            .as_str()
            .ok_or_else(|| protocol("the page did not return its document"))?;
        let page: Value =
            serde_json::from_str(text).map_err(|error| protocol(&error.to_string()))?;
        Ok(BrowserPage {
            url: page["url"].as_str().unwrap_or_default().to_owned(),
            title: page["title"].as_str().unwrap_or_default().to_owned(),
            html: page["html"].as_str().unwrap_or_default().to_owned(),
        })
    }

    /// Brings the window to the front so the person sees what is asked of them.
    ///
    /// # Errors
    /// [`RenderError::Browser`] for DevTools failures.
    pub fn bring_to_front(&mut self) -> Result<(), RenderError> {
        self.call("Page.bringToFront", &json!({}), true).map(|_| ())
    }

    /// Evaluates a read-only expression in the page and returns its value.
    ///
    /// # Errors
    /// [`RenderError::Browser`] for DevTools failures and exceptions in the page.
    pub fn evaluate(&mut self, expression: &str) -> Result<Value, RenderError> {
        let result = self.call(
            "Runtime.evaluate",
            &json!({"expression": expression, "returnByValue": true}),
            true,
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(RenderError::Browser(format!(
                "the page raised an exception: {}",
                exception["text"].as_str().unwrap_or("unknown")
            )));
        }
        Ok(result["result"]["value"].clone())
    }

    /// Closes the browser cleanly, so the profile is not marked as crashed.
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.call("Browser.close", &json!({}), false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn call(&mut self, method: &str, params: &Value, in_page: bool) -> Result<Value, RenderError> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({"id": id, "method": method, "params": params});
        if in_page {
            message["sessionId"] = Value::String(self.session_id.clone());
        }
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(|error| protocol(&error.to_string()))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let reply: Value = serde_json::from_str(&text)
                        .map_err(|error| protocol(&error.to_string()))?;
                    if reply["id"].as_u64() != Some(id) {
                        // Events and replies to other commands.
                        continue;
                    }
                    if let Some(error) = reply.get("error") {
                        return Err(RenderError::Browser(format!(
                            "{method}: {}",
                            error["message"].as_str().unwrap_or("failed")
                        )));
                    }
                    return Ok(reply["result"].clone());
                }
                Ok(Message::Close(_)) => return Err(protocol("the browser closed the connection")),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(error) => return Err(protocol(&error.to_string())),
            }
            if Instant::now() >= deadline {
                return Err(RenderError::Browser(format!("{method} got no answer")));
            }
        }
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            self.shutdown();
        }
    }
}

fn connect(endpoint: &str) -> Result<WebSocket<TcpStream>, RenderError> {
    let address: SocketAddr = endpoint
        .strip_prefix("ws://")
        .and_then(|rest| rest.split('/').next())
        .and_then(|address| address.parse().ok())
        .ok_or_else(|| protocol("invalid DevTools endpoint"))?;
    if !address.ip().is_loopback() {
        return Err(protocol("the DevTools endpoint is not local"));
    }
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let (socket, _) =
        tungstenite::client(endpoint, stream).map_err(|error| protocol(&error.to_string()))?;
    Ok(socket)
}

/// Holds `<profile>.lock` beside the profile while a window uses it.
fn lock_profile(profile: &Path) -> Result<File, RenderError> {
    let path = profile.with_extension("lock");
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(RenderError::Busy(profile.to_owned())),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn protocol(message: &str) -> RenderError {
    RenderError::Browser(message.to_owned())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Serves `/` with a results page and `/sorry/` with a check page.
    fn serve() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let _ = reader.read_line(&mut request);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                    line.clear();
                }
                let body = if request.contains("/sorry/") {
                    "<html><head><title>Check</title></head><body><form id=captcha-form></form></body></html>"
                } else {
                    "<html><head><title>Results</title></head><body><div id=rso><a href='https://a.example/'><h3>A</h3></a></div><script>document.body.dataset.ready='1'</script></body></html>"
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        format!("http://{address}")
    }

    #[test]
    #[ignore = "needs an installed Chrome or Edge"]
    fn loads_pages_in_a_persistent_profile() {
        let base = serve();
        let dir = tempfile::tempdir().unwrap();
        let mut options = BrowserOptions::new(dir.path().join("profile"));
        options.visible = false;
        let mut session = BrowserSession::launch(&options).unwrap();
        assert!(matches!(
            BrowserSession::launch(&options),
            Err(RenderError::Busy(_))
        ));
        let page = session
            .navigate(
                &format!("{base}/"),
                Duration::from_secs(20),
                Duration::from_millis(100),
            )
            .unwrap();
        assert_eq!(page.title, "Results");
        assert!(page.html.contains("data-ready=\"1\""));
        let check = session
            .navigate(
                &format!("{base}/sorry/index"),
                Duration::from_secs(20),
                Duration::ZERO,
            )
            .unwrap();
        assert!(check.url.ends_with("/sorry/index"));
        assert!(check.html.contains("captcha-form"));
        session.bring_to_front().unwrap();
        session.close();
        assert!(dir.path().join("profile").is_dir());
        let again = BrowserSession::launch(&options).unwrap();
        again.close();
    }
}
