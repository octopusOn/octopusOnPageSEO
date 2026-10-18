//! Small blocking DevTools client. Commands keep consuming events while awaiting a reply.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use octopuson_crawl::{
    BrowserRequest, CrawlControl, FetchError, RenderRequest, RenderedPage, Viewport,
};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::downloads::{Admission, Downloads, Flight, Lease};

/// A cache-fill wait never occupies an entire page budget or blocks CDP reception.
const MAX_CACHE_WAIT: Duration = Duration::from_secs(2);
const MAX_WAITERS: usize = 128;

struct WaitingResource {
    params: Value,
    flight: Arc<Flight>,
    until: Instant,
}

type CompletedResource<'a> = (String, crate::cache::Response, Option<Lease<'a>>);

pub(crate) struct Session<'a> {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    attached_id: Option<String>,
    context: Option<String>,
    request: &'a RenderRequest<'a>,
    deadline: Instant,
    initial_fulfilled: bool,
    main_frame: Option<String>,
    dom_ready: Option<Instant>,
    last_network: Instant,
    in_flight: HashSet<String>,
    cache: Option<&'a crate::cache::ResourceCache>,
    cache_requests: HashMap<String, String>,
    cache_responses: HashMap<String, crate::cache::Response>,
    completed: VecDeque<CompletedResource<'a>>,
    downloads: &'a Downloads,
    leases: HashMap<String, Lease<'a>>,
    waiting: VecDeque<WaitingResource>,
}

impl<'a> Session<'a> {
    pub(crate) fn connect(
        endpoint: &str,
        request: &'a RenderRequest<'a>,
        deadline: Instant,
        cache: Option<&'a crate::cache::ResourceCache>,
        downloads: &'a Downloads,
    ) -> Result<Self, FetchError> {
        let address = endpoint
            .strip_prefix("ws://")
            .and_then(|s| s.split('/').next())
            .ok_or_else(|| error("invalid browser endpoint"))?;
        let address: SocketAddr = address
            .parse()
            .map_err(|_| error("invalid browser address"))?;
        if !address.ip().is_loopback() {
            return Err(error("DevTools endpoint is not local"));
        }
        let stream =
            TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(io_error)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(io_error)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(io_error)?;
        let (socket, _) =
            tungstenite::client(endpoint, stream).map_err(|e| error(&e.to_string()))?;
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(io_error)?;
        Ok(Self {
            socket,
            next_id: 0,
            attached_id: None,
            context: None,
            request,
            deadline,
            initial_fulfilled: false,
            main_frame: None,
            dom_ready: None,
            last_network: Instant::now(),
            in_flight: HashSet::new(),
            cache,
            cache_requests: HashMap::new(),
            cache_responses: HashMap::new(),
            completed: VecDeque::new(),
            downloads,
            leases: HashMap::new(),
            waiting: VecDeque::new(),
        })
    }

    pub(crate) fn render(
        &mut self,
        user_agent: &str,
        language: Option<&str>,
        viewport: Viewport,
        wait: Duration,
    ) -> Result<RenderedPage, FetchError> {
        let started = Instant::now();
        let context = self.call(
            "Target.createBrowserContext",
            json!({"disposeOnDetach":true}),
        )?;
        let context = string(&context, "browserContextId")?;
        self.context = Some(context.clone());
        self.call(
            "Browser.setDownloadBehavior",
            json!({"behavior":"deny","browserContextId":context}),
        )?;
        let target = self.call(
            "Target.createTarget",
            json!({"url":"about:blank","browserContextId":context}),
        )?;
        let attached = self.call(
            "Target.attachToTarget",
            json!({"targetId":string(&target,"targetId")?,"flatten":true}),
        )?;
        self.attached_id = Some(string(&attached, "sessionId")?);
        self.call("Page.enable", json!({}))?;
        let frame = self.call("Page.getFrameTree", json!({}))?;
        self.main_frame = frame["frameTree"]["frame"]["id"]
            .as_str()
            .map(str::to_owned);
        self.call("Network.enable", json!({"maxTotalBufferSize":32*1024*1024,"maxResourceBufferSize":crate::cache::ENTRY_BYTES}))?;
        self.call("Network.setCacheDisabled", json!({"cacheDisabled":true}))?;
        self.call("Network.setBypassServiceWorker", json!({"bypass":true}))?;
        let mut agent = json!({"userAgent":user_agent});
        if let Some(language) = language {
            agent["acceptLanguage"] = json!(language);
        }
        self.call("Network.setUserAgentOverride", agent)?;
        self.call("Emulation.setDeviceMetricsOverride",json!({"width":viewport.width,"height":viewport.height,"deviceScaleFactor":1,"mobile":viewport.mobile}))?;
        self.call(
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*","requestStage":"Request"}]}),
        )?;
        // Page.navigate may not reply until the paused main request is fulfilled. call()
        // handles Fetch.requestPaused in the same receive loop, so navigation cannot deadlock.
        let navigation = self.call("Page.navigate", json!({"url":self.request.url}))?;
        if let Some(reason) = navigation
            .get("errorText")
            .and_then(Value::as_str)
            .filter(|reason| !reason.is_empty())
        {
            return Err(error(reason));
        }
        loop {
            self.check()?;
            self.cache_completed()?;
            self.resume_waiters()?;
            if self.dom_ready.is_some_and(|ready| ready.elapsed() >= wait)
                && self.in_flight.is_empty()
                && self.waiting.is_empty()
                && self.last_network.elapsed() >= Duration::from_millis(500)
            {
                break;
            }
            if let Some(message) = self.read()? {
                self.event(&message)?;
            }
        }
        let limit = self.request.body_limit.min(64 * 1024 * 1024);
        let expression = format!(
            "(() => {{ const html=document.documentElement?.outerHTML ?? ''; let clipped=html.slice(0,{limit}); const last=clipped.charCodeAt(clipped.length-1); if(last>=0xD800 && last<=0xDBFF) clipped=clipped.slice(0,-1); return {{html:clipped,truncated:html.length>{limit},url:document.URL}}; }})()"
        );
        let snapshot=self.call("Runtime.evaluate",json!({"expression":expression,"returnByValue":true,"awaitPromise":false,"timeout":1000}))?;
        if snapshot.get("exceptionDetails").is_some() {
            return Err(error("could not serialize the rendered DOM"));
        }
        let value = &snapshot["result"]["value"];
        let mut html = string(value, "html")?;
        let mut truncated = value["truncated"].as_bool().unwrap_or(false);
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        if html.len() > limit {
            let mut end = limit;
            while !html.is_char_boundary(end) {
                end -= 1;
            }
            html.truncate(end);
            truncated = true;
        }
        if !self.initial_fulfilled {
            return Err(error("browser did not consume the fetched HTML"));
        }
        Ok(RenderedPage {
            html,
            url: string(value, "url")?,
            elapsed: started.elapsed(),
            truncated,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, FetchError> {
        self.check()?;
        let id = self.send(method, params)?;
        loop {
            if let Some(message) = self.read()? {
                if message["id"].as_u64() == Some(id) {
                    if let Some(reason) = message.get("error") {
                        return Err(error(&format!("{method}: {reason}")));
                    }
                    return Ok(message["result"].clone());
                }
                self.event(&message)?;
            }
        }
    }

    fn send(&mut self, method: &str, params: Value) -> Result<u64, FetchError> {
        self.next_id += 1;
        let mut message = json!({"id":self.next_id,"method":method});
        message["params"] = params;
        if let Some(session) = &self.attached_id {
            message["sessionId"] = json!(session);
        }
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(|e| error(&e.to_string()))?;
        Ok(self.next_id)
    }

    fn read(&mut self) -> Result<Option<Value>, FetchError> {
        self.check()?;
        match self.socket.read() {
            Ok(Message::Text(text)) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| error(&e.to_string())),
            Ok(Message::Close(_)) => Err(error("browser closed the rendering session")),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                Ok(None)
            }
            Err(e) => Err(error(&e.to_string())),
        }
    }

    fn event(&mut self, message: &Value) -> Result<(), FetchError> {
        if message.get("sessionId").and_then(Value::as_str) != self.attached_id.as_deref() {
            return Ok(());
        }
        let params = &message["params"];
        match message["method"].as_str().unwrap_or_default() {
            "Fetch.requestPaused" => self.paused(params)?,
            "Page.domContentEventFired" => {
                self.dom_ready = Some(Instant::now());
            }
            "Network.requestWillBeSent" => {
                if !matches!(params["type"].as_str(), Some("WebSocket" | "EventSource")) {
                    if let Some(id) = params["requestId"].as_str() {
                        self.in_flight.insert(id.to_owned());
                    }
                    self.last_network = Instant::now();
                }
            }
            "Network.responseReceived" => self.cache_response(params),
            "Network.loadingFinished" | "Network.loadingFailed" => {
                if let Some(id) = params["requestId"].as_str() {
                    self.in_flight.remove(id);
                    self.cache_requests.remove(id);
                    let lease = self.leases.remove(id);
                    if let Some(response) = self.cache_responses.remove(id)
                        && message["method"] == "Network.loadingFinished"
                        && params["encodedDataLength"]
                            .as_f64()
                            .is_some_and(|len| len <= f64::from(2 * 1024 * 1024))
                    {
                        self.completed.push_back((id.to_owned(), response, lease));
                    }
                }
                self.last_network = Instant::now();
            }
            "Page.javascriptDialogOpening" => {
                self.send("Page.handleJavaScriptDialog", json!({"accept":false}))?;
            }
            "Inspector.targetCrashed" => return Err(error("browser page crashed")),
            _ => {}
        }
        Ok(())
    }

    fn paused(&mut self, params: &Value) -> Result<(), FetchError> {
        let id = string(params, "requestId")?;
        if params.get("responseStatusCode").is_some() || params.get("responseErrorReason").is_some()
        {
            if let Some(network) = params["networkId"].as_str()
                && crate::cache::Response::intercepted(params).is_none()
            {
                self.leases.remove(network);
            }
            self.send("Fetch.continueRequest", json!({"requestId":id}))?;
            return Ok(());
        }
        let url = params["request"]["url"].as_str().unwrap_or_default();
        if !self.initial_fulfilled && url == self.request.url {
            self.initial_fulfilled = true;
            let headers: Vec<_> = self
                .request
                .response
                .headers
                .iter()
                .filter(|(name, _)| {
                    !matches!(
                        name.to_ascii_lowercase().as_str(),
                        "content-encoding" | "content-length" | "transfer-encoding" | "connection"
                    )
                })
                .map(|(name, value)| json!({"name":name,"value":value}))
                .collect();
            self.send("Fetch.fulfillRequest",json!({"requestId":id,"responseCode":self.request.response.status,"responseHeaders":headers,"body":base64::engine::general_purpose::STANDARD.encode(&self.request.response.body)}))?;
        } else {
            self.resource(params)?;
        }
        Ok(())
    }

    fn resource(&mut self, params: &Value) -> Result<(), FetchError> {
        let reusable =
            self.cache.is_some() && crate::cache::reusable_request(params, self.request.url);
        let url = params["request"]["url"].as_str().unwrap_or_default();
        let cached = reusable
            .then(|| self.cache.and_then(|cache| cache.get(url)))
            .flatten();
        if !reusable || cached.is_some() {
            return self.dispatch(params, cached, None);
        }
        match self.downloads.reserve(url) {
            Admission::Network(lease) => {
                // A fill can complete between the initial cache lookup and admission.
                let cached = self.cache.and_then(|cache| cache.get(url));
                self.dispatch(params, cached, lease)
            }
            Admission::Wait(flight) if self.waiting.len() < MAX_WAITERS => {
                let now = Instant::now();
                let wait = self
                    .deadline
                    .saturating_duration_since(now)
                    .div_f64(4.0)
                    .min(MAX_CACHE_WAIT);
                self.waiting.push_back(WaitingResource {
                    params: params.clone(),
                    flight,
                    until: now.checked_add(wait).unwrap_or(self.deadline),
                });
                Ok(())
            }
            Admission::Wait(_) => self.dispatch(params, None, None),
        }
    }

    fn dispatch(
        &mut self,
        params: &Value,
        cached: Option<Arc<crate::cache::Resource>>,
        lease: Option<Lease<'a>>,
    ) -> Result<(), FetchError> {
        let id = string(params, "requestId")?;
        let url = params["request"]["url"].as_str().unwrap_or_default();
        let document = params["resourceType"].as_str() == Some("Document");
        let reusable =
            self.cache.is_some() && crate::cache::reusable_request(params, self.request.url);
        let kind = if document {
            BrowserRequest::Document
        } else if cached.is_some() {
            BrowserRequest::CachedResource
        } else {
            BrowserRequest::Resource
        };
        let allowed = self
            .request
            .request_allowed
            .is_none_or(|allowed| allowed(url, kind, self.deadline));
        self.check()?;
        if allowed {
            // Even a normal rate wait may have given another page time to populate the cache.
            let cached = cached.or_else(|| {
                reusable
                    .then(|| self.cache.and_then(|cache| cache.get(url)))
                    .flatten()
            });
            if let Some(cached) = cached {
                let headers: Vec<_> = cached
                    .headers
                    .iter()
                    .map(|(name, value)| json!({"name":name,"value":value}))
                    .collect();
                self.send("Fetch.fulfillRequest",json!({"requestId":id,"responseCode":200,"responseHeaders":headers,"body":base64::engine::general_purpose::STANDARD.encode(&cached.body)}))?;
            } else {
                let mut intercept_response = false;
                if let Some(network) = params["networkId"].as_str() {
                    self.cache_requests.remove(network);
                    self.leases.remove(network);
                    if reusable {
                        self.cache_requests
                            .insert(network.to_owned(), url.to_owned());
                        if let Some(lease) = lease {
                            self.leases.insert(network.to_owned(), lease);
                            intercept_response = true;
                        }
                    }
                }
                self.send(
                    "Fetch.continueRequest",
                    json!({"requestId":id,"interceptResponse":intercept_response}),
                )?;
            }
        } else {
            self.send(
                "Fetch.failRequest",
                json!({"requestId":id,"errorReason":"BlockedByClient"}),
            )?;
            if document && params["frameId"].as_str() == self.main_frame.as_deref() {
                return Err(error(
                    "document navigation blocked by crawl scope or robots.txt",
                ));
            }
        }
        Ok(())
    }

    fn resume_waiters(&mut self) -> Result<(), FetchError> {
        // Inspect only this round's queue: newly paused requests can arrive inside calls.
        for _ in 0..self.waiting.len() {
            self.check()?;
            let Some(waiting) = self.waiting.pop_front() else {
                break;
            };
            let url = waiting.params["request"]["url"]
                .as_str()
                .unwrap_or_default();
            let cached = self.cache.and_then(|cache| cache.get(url));
            if cached.is_some() || waiting.flight.finished() || Instant::now() >= waiting.until {
                // Private/failed fills and bounded waits fall back to independent networking.
                self.dispatch(&waiting.params, cached, None)?;
            } else {
                self.waiting.push_back(waiting);
            }
        }
        Ok(())
    }

    fn cache_response(&mut self, params: &Value) {
        let Some(id) = params["requestId"].as_str() else {
            return;
        };
        let response = &params["response"];
        if self.cache_requests.get(id).map(String::as_str) == response["url"].as_str()
            && let Some(response) = crate::cache::Response::new(response)
        {
            self.cache_responses.insert(id.to_owned(), response);
        } else {
            // Ineligible response headers release other pages before a slow body completes.
            self.leases.remove(id);
        }
    }

    fn cache_completed(&mut self) -> Result<(), FetchError> {
        while let Some((id, response, _lease)) = self.completed.pop_front() {
            self.check()?;
            // An evicted/oversized Chrome body is simply a cache miss, never a render failure.
            if let Ok(value) = self.call("Network.getResponseBody", json!({"requestId":id})) {
                let Some(body) = value["body"].as_str() else {
                    continue;
                };
                let body = if value["base64Encoded"] == true {
                    let Ok(body) = base64::engine::general_purpose::STANDARD.decode(body) else {
                        continue;
                    };
                    body
                } else {
                    body.as_bytes().to_vec()
                };
                if let Some(resource) = response.with_body(body)
                    && let Some(cache) = self.cache
                {
                    cache.insert(resource);
                }
            }
        }
        self.check()
    }

    fn check(&self) -> Result<(), FetchError> {
        check(self.request.control, self.deadline)
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            self.attached_id = None;
            let _ = self.send(
                "Target.disposeBrowserContext",
                json!({"browserContextId":context}),
            );
        }
    }
}

pub(crate) fn check(control: &CrawlControl, deadline: Instant) -> Result<(), FetchError> {
    if control.is_stopped() {
        return Err(error("rendering cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(FetchError::Timeout);
    }
    Ok(())
}
fn string(value: &Value, key: &str) -> Result<String, FetchError> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| error(&format!("missing DevTools field: {key}")))
}
fn error(message: &str) -> FetchError {
    FetchError::Other(format!("rendering: {message}"))
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "io::Result::map_err adapter consumes the I/O error"
)]
fn io_error(error: std::io::Error) -> FetchError {
    FetchError::Connection(error.to_string())
}
