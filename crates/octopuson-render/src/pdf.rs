//! Prints a self-contained HTML document to PDF with a private, headless browser.
//!
//! The document is opened as a `data:` address with scripts turned off, and printed with
//! `Page.printToPDF`, its page size and margins taken from its own CSS (`@page`). Documents are
//! meant to carry everything inline (styles, images as `data:` addresses) and to forbid other
//! requests with their own Content-Security-Policy; nothing else is loaded for them.

use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::RenderError;
use crate::browser::Browser;

/// Longest document printed (as a `data:` address the browser accepts at most 2 MB).
pub const MAX_PDF_HTML: usize = 1_400_000;

/// How long a print may take, the browser's start included.
const PRINT_TIMEOUT: Duration = Duration::from_secs(45);

/// A blocking DevTools connection to the browser, with flattened page sessions.
struct Connection {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    deadline: Instant,
}

fn browser_error(message: impl Into<String>) -> RenderError {
    RenderError::Browser(message.into())
}

impl Connection {
    fn open(endpoint: &str, deadline: Instant) -> Result<Self, RenderError> {
        let address: SocketAddr = endpoint
            .strip_prefix("ws://")
            .and_then(|rest| rest.split('/').next())
            .and_then(|address| address.parse().ok())
            .ok_or_else(|| browser_error("invalid DevTools endpoint"))?;
        if !address.ip().is_loopback() {
            return Err(browser_error("the DevTools endpoint is not local"));
        }
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let (socket, _) = tungstenite::client(endpoint, stream)
            .map_err(|error| browser_error(error.to_string()))?;
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(200)))?;
        Ok(Self {
            socket,
            next_id: 0,
            deadline,
        })
    }

    /// The next message, or `None` when nothing arrived for a moment.
    fn read(&mut self) -> Result<Option<Value>, RenderError> {
        if Instant::now() >= self.deadline {
            return Err(browser_error("printing took too long"));
        }
        match self.socket.read() {
            Ok(Message::Text(text)) => Ok(serde_json::from_str(&text).ok()),
            Ok(Message::Binary(bytes)) => Ok(serde_json::from_slice(&bytes).ok()),
            Ok(Message::Close(_)) => Err(browser_error("the browser closed the connection")),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(browser_error(error.to_string())),
        }
    }

    /// Sends a command (to a page session when given) and waits for its answer; events seen
    /// meanwhile go to `events`.
    fn call(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: &Value,
        events: &mut Vec<Value>,
    ) -> Result<Value, RenderError> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(|error| browser_error(error.to_string()))?;
        loop {
            let Some(value) = self.read()? else {
                continue;
            };
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = value.get("error") {
                    let text = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("failed");
                    return Err(browser_error(format!("{method}: {text}")));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            if value.get("method").is_some() {
                events.push(value);
            }
        }
    }

    /// Waits for an event of a session.
    fn wait_for(
        &mut self,
        session: &str,
        method: &str,
        events: &[Value],
    ) -> Result<(), RenderError> {
        let matches = |event: &Value| {
            event.get("method").and_then(Value::as_str) == Some(method)
                && event.get("sessionId").and_then(Value::as_str) == Some(session)
        };
        if events.iter().any(matches) {
            return Ok(());
        }
        loop {
            if let Some(event) = self.read()?
                && matches(&event)
            {
                return Ok(());
            }
        }
    }
}

/// Prints `html` to PDF with an installed Chrome, Chromium or Edge (`browser` if given).
///
/// # Errors
/// [`RenderError::BrowserMissing`] without a browser; [`RenderError::Browser`] for a document
/// over [`MAX_PDF_HTML`] or a browser that fails or takes over 45 seconds.
pub fn print_pdf(html: &str, browser: Option<&str>) -> Result<Vec<u8>, RenderError> {
    if html.len() > MAX_PDF_HTML {
        return Err(browser_error("the document is too large to print"));
    }
    let deadline = Instant::now() + PRINT_TIMEOUT;
    let process = Browser::launch(browser)?;
    let mut connection = Connection::open(&process.endpoint, deadline)?;
    let mut events = Vec::new();
    let target = connection.call(
        None,
        "Target.createTarget",
        &json!({ "url": "about:blank" }),
        &mut events,
    )?;
    let target_id = target
        .get("targetId")
        .and_then(Value::as_str)
        .ok_or_else(|| browser_error("no page was opened"))?
        .to_owned();
    let attached = connection.call(
        None,
        "Target.attachToTarget",
        &json!({ "targetId": target_id, "flatten": true }),
        &mut events,
    )?;
    let session = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| browser_error("the page could not be attached"))?
        .to_owned();
    let page = Some(session.as_str());
    connection.call(page, "Page.enable", &json!({}), &mut events)?;
    connection.call(
        page,
        "Emulation.setScriptExecutionDisabled",
        &json!({ "value": true }),
        &mut events,
    )?;
    let address = format!("data:text/html;base64,{}", STANDARD.encode(html));
    events.clear();
    connection.call(
        page,
        "Page.navigate",
        &json!({ "url": address }),
        &mut events,
    )?;
    connection.wait_for(&session, "Page.loadEventFired", &events)?;
    let printed = connection.call(
        page,
        "Page.printToPDF",
        &json!({
            "printBackground": true,
            "preferCSSPageSize": true,
            "displayHeaderFooter": false,
            "marginTop": 0,
            "marginBottom": 0,
            "marginLeft": 0,
            "marginRight": 0,
        }),
        &mut events,
    )?;
    let data = printed
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| browser_error("the browser returned no PDF"))?;
    let pdf = STANDARD
        .decode(data)
        .map_err(|error| browser_error(error.to_string()))?;
    let _ = connection.call(
        None,
        "Target.closeTarget",
        &json!({ "targetId": target_id }),
        &mut events,
    );
    drop(process);
    Ok(pdf)
}
