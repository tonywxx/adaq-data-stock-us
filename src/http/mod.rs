//! HTTP session: the `YfSession` is the shared, thread-safe core that owns the
//! impersonating HTTP client, cookie/consent bootstrap, crumb, retry/backoff,
//! and the sqlite cache. Mirrors yfinance's `YfData` singleton.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use primp::{Client, Impersonate};
use serde_json::Value;

use crate::cache::Cache;
use crate::config::Config;
use crate::error::{Result, YfError};

mod retry;
use retry::{Decision, decide_status, decide_transport};

const QUERY1: &str = "https://query1.finance.yahoo.com";
const QUERY2: &str = "https://query2.finance.yahoo.com";
const ROOT: &str = "https://finance.yahoo.com";
const CRUMB_TTL: i64 = 60; // seconds
const SUBSCRIPTIONS_URL: &str =
    "https://query1.finance.yahoo.com/ws/obi-integration/v1/subscriptions";

/// A Yahoo Finance HTTP session (cookie jar + crumb + cache + retry).
///
/// Cheap to clone (internals are behind `Arc`/shared handles) — clones share
/// the same underlying client and cache.
#[derive(Clone)]
pub struct YfSession {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    transport: Arc<dyn Transport>,
    config: Config,
    cache: Cache,
    cookie_jar: Arc<primp::cookie::Jar>,
    crumb: Mutex<Option<String>>,
    cookie_ready: Mutex<bool>,
}

impl YfSession {
    /// Build a new session from config, using the production `primp` transport.
    pub fn new(config: Config) -> Result<Self> {
        let cookie_jar = Arc::new(primp::cookie::Jar::default());
        let mut builder = Client::builder()
            .impersonate(Impersonate::ChromeV146)
            .cookie_provider(cookie_jar.clone())
            .user_agent(config.user_agent.clone())
            .timeout(Duration::from_secs(config.timeout_secs));
        if let Some(proxy) = &config.proxy {
            builder = builder.proxy(
                primp::Proxy::all(proxy.clone())
                    .map_err(|e| YfError::msg(format!("invalid proxy: {e}")))?,
            );
        }
        let client = builder
            .build()
            .map_err(|e| YfError::msg(format!("build client: {e}")))?;
        let transport: Arc<dyn Transport> = Arc::new(PrimpTransport { client });
        Self::with_transport(config, transport, Some(cookie_jar))
    }

    /// Build a session around an explicit transport — used by offline tests to
    /// inject a [`MockTransport`]. `cookie_jar` is created internally when
    /// `None` (production path); tests may pass one to seed login cookies.
    pub(crate) fn with_transport(
        config: Config,
        transport: Arc<dyn Transport>,
        cookie_jar: Option<Arc<primp::cookie::Jar>>,
    ) -> Result<Self> {
        let cookie_jar = cookie_jar.unwrap_or_else(|| Arc::new(primp::cookie::Jar::default()));
        let cache = Cache::open(config.cache_dir.clone())?;
        let session = Self {
            inner: Arc::new(SessionInner {
                transport,
                config,
                cache,
                cookie_jar: cookie_jar.clone(),
                crumb: Mutex::new(None),
                cookie_ready: Mutex::new(false),
            }),
        };
        // Seed any configured login cookies (mirrors yfinance reading cookies
        // from config at session init). Verification happens later via
        // `check_login`/`set_login_cookies`; here we only store them.
        if let (Some(t), Some(y)) = (
            &session.inner.config.cookie_t,
            &session.inner.config.cookie_y,
        ) {
            let url: primp::Url = ROOT.parse().expect("valid root url");
            cookie_jar.add_cookie_str(&format!("T={t}; Domain=finance.yahoo.com; Path=/"), &url);
            cookie_jar.add_cookie_str(&format!("Y={y}; Domain=finance.yahoo.com; Path=/"), &url);
        }
        Ok(session)
    }

    /// Access the configuration.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Access the cache.
    pub fn cache(&self) -> &Cache {
        &self.inner.cache
    }

    /// Ensure a consent cookie exists by hitting a Yahoo endpoint that sets one.
    async fn ensure_cookie(&self) -> Result<()> {
        if *self.inner.cookie_ready.lock().unwrap() {
            return Ok(());
        }
        // fc.yahoo.com is the endpoint yfinance uses to obtain the consent cookie.
        let _ = self
            .inner
            .transport
            .send(TransportRequest {
                method: TransportMethod::Get,
                url: "https://fc.yahoo.com".to_string(),
                headers: vec![("accept".to_string(), "*/*".to_string())],
                query: vec![],
                json_body: None,
            })
            .await;
        *self.inner.cookie_ready.lock().unwrap() = true;
        Ok(())
    }

    /// Fetch (and cache) the crumb token used to authorize query requests.
    async fn get_crumb(&self) -> Result<String> {
        if let Some(c) = self.inner.crumb.lock().unwrap().clone() {
            return Ok(c);
        }
        self.ensure_cookie().await?;
        let url = format!("{QUERY1}/v1/test/getcrumb");
        let resp = self
            .inner
            .transport
            .send(TransportRequest {
                method: TransportMethod::Get,
                url,
                headers: vec![("accept".to_string(), "*/*".to_string())],
                query: vec![],
                json_body: None,
            })
            .await?;
        let crumb = String::from_utf8_lossy(&resp.body).trim().to_string();
        if !crumb.is_empty() {
            self.inner.cache.set_crumb(&crumb, CRUMB_TTL);
            *self.inner.crumb.lock().unwrap() = Some(crumb.clone());
        }
        Ok(crumb)
    }

    fn reset_auth(&self) {
        *self.inner.crumb.lock().unwrap() = None;
        *self.inner.cookie_ready.lock().unwrap() = false;
    }

    /// Perform a GET returning parsed JSON, with cookie/consent bootstrap, crumb
    /// injection, and retry/backoff. `base` is the host-prefixed path-less URL.
    pub async fn get_json(&self, url: &str, params: &[(&str, String)]) -> Result<Value> {
        let retries = self.inner.config.retries;
        let mut attempt: u32 = 0;
        loop {
            self.ensure_cookie().await?;
            // Crumb is (re)fetched on every attempt: a 401 invalidates it via
            // `reset_auth`, and the next attempt must send a fresh crumb rather
            // than the stale one captured before the loop. On a cache hit this
            // is a cheap in-memory/sqlite lookup, so retried calls stay cheap.
            let crumb = self.get_crumb().await.unwrap_or_default();
            let mut qp: Vec<(String, String)> = params
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            if !crumb.is_empty() {
                qp.push(("crumb".to_string(), crumb));
            }
            if !self.inner.config.locale.lang.is_empty() {
                qp.push(("lang".to_string(), self.inner.config.locale.lang.clone()));
                qp.push((
                    "region".to_string(),
                    self.inner.config.locale.region.clone(),
                ));
            }
            let req = TransportRequest {
                method: TransportMethod::Get,
                url: url.to_string(),
                headers: vec![
                    ("accept".to_string(), "*/*".to_string()),
                    (
                        "accept-language".to_string(),
                        format!(
                            "{}-{}",
                            self.inner.config.locale.lang, self.inner.config.locale.region
                        ),
                    ),
                ],
                query: qp.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                json_body: None,
            };
            let resp = match self.inner.transport.send(req).await {
                Ok(r) => r,
                Err(e) => match decide_transport(e, attempt, retries) {
                    Decision::Retry { backoff, .. } => {
                        attempt += 1;
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    Decision::GiveUp { error } => return Err(error),
                    Decision::Success => unreachable!("transport error cannot be Success"),
                },
            };
            let status = resp.status;
            let decision = decide_status(status, attempt, retries, /*retry_401=*/ true);
            match decision {
                Decision::Success => {
                    let value: Value = serde_json::from_slice(&resp.body)?;
                    return Ok(value);
                }
                Decision::Retry {
                    backoff,
                    reset_auth,
                } => {
                    if reset_auth {
                        self.reset_auth();
                    }
                    attempt += 1;
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                Decision::GiveUp { error } => {
                    // On a terminal non-success status we surface the response
                    // body for diagnostics, matching the prior behavior. Other
                    // terminal errors (e.g. RateLimited, 401-after-retries
                    // message) carry no body.
                    match error {
                        YfError::Status { status, .. } => {
                            let body = String::from_utf8_lossy(&resp.body).to_string();
                            return Err(YfError::Status { status, body });
                        }
                        other => return Err(other),
                    }
                }
            }
        }
    }

    /// Perform a GET returning the raw response body as text, with cookie/
    /// consent bootstrap and retry/backoff. Used for endpoints that return HTML
    /// or non-JSON text (e.g. the Business Insider ISIN suggest endpoint).
    /// Unlike [`YfSession::get_json`], no crumb is injected and the body is not
    /// parsed.
    pub async fn get_text(&self, url: &str, params: &[(&str, String)]) -> Result<String> {
        let retries = self.inner.config.retries;
        let mut attempt: u32 = 0;
        loop {
            self.ensure_cookie().await?;
            let req = TransportRequest {
                method: TransportMethod::Get,
                url: url.to_string(),
                headers: vec![("accept".to_string(), "*/*".to_string())],
                query: params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                json_body: None,
            };
            let resp = match self.inner.transport.send(req).await {
                Ok(r) => r,
                Err(e) => match decide_transport(e, attempt, retries) {
                    Decision::Retry { backoff, .. } => {
                        attempt += 1;
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    Decision::GiveUp { error } => return Err(error),
                    Decision::Success => unreachable!("transport error cannot be Success"),
                },
            };
            let status = resp.status;
            // No crumb on text endpoints: a 401 is terminal (retry_401=false).
            let decision = decide_status(status, attempt, retries, /*retry_401=*/ false);
            match decision {
                Decision::Success => return Ok(String::from_utf8_lossy(&resp.body).to_string()),
                Decision::Retry { backoff, .. } => {
                    attempt += 1;
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                Decision::GiveUp { error } => match error {
                    YfError::Status { status, .. } => {
                        let body = String::from_utf8_lossy(&resp.body).to_string();
                        return Err(YfError::Status { status, body });
                    }
                    other => return Err(other),
                },
            }
        }
    }

    /// Perform a POST returning parsed JSON.
    pub async fn post_json(
        &self,
        url: &str,
        params: &[(&str, String)],
        body: &Value,
    ) -> Result<Value> {
        let mut qp: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let crumb = self.get_crumb().await.unwrap_or_default();
        if !crumb.is_empty() {
            qp.push(("crumb".to_string(), crumb));
        }
        let resp = self
            .inner
            .transport
            .send(TransportRequest {
                method: TransportMethod::Post,
                url: url.to_string(),
                headers: vec![
                    ("accept".to_string(), "*/*".to_string()),
                    ("content-type".to_string(), "application/json".to_string()),
                ],
                query: qp.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                json_body: Some(body.clone()),
            })
            .await?;
        let status = resp.status;
        // Single attempt (no retry loop), so retries=0 makes every non-success
        // decision terminal. Status policy is shared with get_json/get_text.
        match decide_status(status, 0, 0, /*retry_401=*/ true) {
            Decision::Success => Ok(serde_json::from_slice(&resp.body)?),
            Decision::GiveUp { error } => match error {
                YfError::Status { status, .. } => {
                    let b = String::from_utf8_lossy(&resp.body).to_string();
                    Err(YfError::Status { status, body: b })
                }
                other => Err(other),
            },
            Decision::Retry { .. } => unreachable!("retries=0 yields no Retry decision"),
        }
    }

    /// Host constants, exposed for module implementations.
    pub fn urls() -> Urls {
        Urls {
            query1: QUERY1,
            query2: QUERY2,
            root: ROOT,
        }
    }
}

/// Yahoo host URLs.
pub struct Urls {
    pub query1: &'static str,
    pub query2: &'static str,
    pub root: &'static str,
}

// --- Transport seam ---------------------------------------------------------
//
// Every endpoint method in this module ultimately needs one thing from the
// network: "send this request, give me back a status + body". That single
// step is the seam. `YfSession` still owns the *policy* — crumb injection,
// locale append, consent bootstrap, and the retry loop driven by
// `decide_status`/`decide_transport` — and only delegates the raw send to a
// `Transport`. Swapping in a `MockTransport` therefore exercises the glue
// (crumb, retry, error mapping) offline; the real `PrimpTransport` is the
// production default.

/// A request handed to a [`Transport`]. Built by the session glue after crumb
/// and locale have already been appended to `query`.
#[derive(Clone)]
pub(crate) struct TransportRequest {
    pub method: TransportMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub json_body: Option<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TransportMethod {
    Get,
    Post,
}

/// The minimal response the session glue needs: an HTTP status and a body.
/// Headers/cookies are request-side only in this module, so they are not
/// modelled here.
pub(crate) struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// The injectable network seam. Implemented by [`PrimpTransport`] in
/// production and by the test `MockTransport`.
#[async_trait]
pub(crate) trait Transport: Send + Sync {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse>;
}

/// Production transport wrapping a `primp` (reqwest-compatible) client. Builds
/// the request from a [`TransportRequest`] and returns status + body bytes.
struct PrimpTransport {
    client: Client,
}

#[async_trait]
impl Transport for PrimpTransport {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse> {
        let mut builder = match req.method {
            TransportMethod::Get => self.client.get(&req.url),
            TransportMethod::Post => self.client.post(&req.url),
        };
        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        builder = builder.query(&req.query);
        if let Some(body) = req.json_body {
            builder = builder.json(&body);
        }
        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let body = resp.bytes().await?;
        Ok(TransportResponse {
            status,
            body: body.to_vec(),
        })
    }
}

// --- Authentication (mirrors yfinance `Auth`) ---

impl YfSession {
    /// Single-shot GET of the OBI subscriptions endpoint, returning the HTTP
    /// status and parsed JSON body (or `Value::Null` if not JSON). Does not
    /// retry and does not require a crumb — used purely for login/entitlement
    /// inspection. Mirrors yfinance's `_SUBSCRIPTIONS_URL` fetch.
    async fn subscriptions(&self) -> Result<(u16, Value)> {
        let resp = self
            .inner
            .transport
            .send(TransportRequest {
                method: TransportMethod::Get,
                url: SUBSCRIPTIONS_URL.to_string(),
                headers: vec![("accept".to_string(), "*/*".to_string())],
                query: vec![],
                json_body: None,
            })
            .await?;
        let status = resp.status;
        let value: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
        Ok((status, value))
    }

    /// Inject the Yahoo `T` / `Y` login cookies, then verify they are valid.
    ///
    /// Returns `true` if the cookies correspond to a logged-in account (the
    /// cookies are stored either way). Mirrors `Auth.set_login_cookies`.
    pub async fn set_login_cookies(&self, cookie_t: &str, cookie_y: &str) -> Result<bool> {
        let url: primp::Url = ROOT.parse().expect("valid root url");
        self.inner.cookie_jar.add_cookie_str(
            &format!("T={cookie_t}; Domain=finance.yahoo.com; Path=/"),
            &url,
        );
        self.inner.cookie_jar.add_cookie_str(
            &format!("Y={cookie_y}; Domain=finance.yahoo.com; Path=/"),
            &url,
        );
        self.check_login().await
    }

    /// Check login state via the OBI subscriptions endpoint (mirrors
    /// `Auth.check_login`). A 401/403 (or a 200 without a `guid`) means not
    /// logged in; a transient error is reported as `Err`.
    pub async fn check_login(&self) -> Result<bool> {
        match self.subscriptions().await {
            Ok((_, v)) => {
                let logged_in = v
                    .get("result")
                    .and_then(|r| r.get("guid"))
                    .map(|g| !g.is_null())
                    .unwrap_or(false);
                Ok(logged_in)
            }
            Err(YfError::Status { status, .. }) if status == 401 || status == 403 => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Return the account's subscription tier (`gold`/`silver`/`bronze`/
    /// `premium`/`free`) or `None` when not logged in. Mirrors
    /// `Auth.subscription_tier`.
    pub async fn subscription_tier(&self) -> Result<Option<String>> {
        let (_status, v) = self.subscriptions().await?;
        let entitlement = v.get("result").and_then(|r| r.as_object());
        let entitlement = match entitlement {
            Some(e) if e.get("guid").is_some() => e,
            _ => return Ok(None),
        };
        let active = entitlement
            .get("subscriptionView")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|s| s.get("action").and_then(|a| a.as_str()) == Some("ACTIVE"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if active.is_empty() {
            return Ok(Some("free".to_string()));
        }
        let tier = active[0].get("tier").and_then(|t| t.as_i64()).unwrap_or(0);
        let name = match tier {
            6 => "gold",
            5 => "silver",
            3 => "bronze",
            _ => "premium",
        };
        Ok(Some(name.to_string()))
    }

    /// Return the logged-in user's `guid`, or `None` when not logged in.
    /// Mirrors `Auth.user`.
    pub async fn user(&self) -> Result<Option<String>> {
        let (_status, v) = self.subscriptions().await?;
        Ok(v.get("result")
            .and_then(|r| r.get("guid"))
            .and_then(|g| g.as_str())
            .map(String::from))
    }
}

// --- Offline test double ----------------------------------------------------

#[cfg(test)]
pub(crate) struct MockTransport {
    handler: Arc<dyn Fn(&TransportRequest) -> Result<TransportResponse> + Send + Sync>,
    recorded: Mutex<Vec<TransportRequest>>,
}

#[cfg(test)]
impl MockTransport {
    pub(crate) fn new(
        handler: impl Fn(&TransportRequest) -> Result<TransportResponse> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            handler: Arc::new(handler),
            recorded: Mutex::new(Vec::new()),
        })
    }

    /// Every request the session has driven through this transport, in order.
    pub(crate) fn requests(&self) -> Vec<TransportRequest> {
        self.recorded.lock().unwrap().clone()
    }

    /// Find the first recorded request whose URL contains `sub`.
    pub(crate) fn find(&self, sub: &str) -> Option<TransportRequest> {
        self.requests().into_iter().find(|r| r.url.contains(sub))
    }
}

#[cfg(test)]
#[async_trait]
impl Transport for MockTransport {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse> {
        self.recorded.lock().unwrap().push(req.clone());
        (self.handler)(&req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use crate::config::Config;
    use crate::error::{Result as YfResult, YfError};

    /// A throwaway cache dir per test so crumb caching never leaks between
    /// cases (the default `None` dir is shared process-wide).
    fn unique_cache_dir() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("adaq-http-test-{}-{}", std::process::id(), n))
    }

    fn resp(status: u16, body: &str) -> YfResult<TransportResponse> {
        Ok(TransportResponse {
            status,
            body: body.as_bytes().to_vec(),
        })
    }

    /// Build a session backed by `transport`, with an isolated cache.
    fn session(retries: u32, transport: Arc<MockTransport>) -> YfSession {
        let cfg = Config::default()
            .retries(retries)
            .cache_dir(unique_cache_dir());
        YfSession::with_transport(cfg, transport, None).expect("session")
    }

    /// A handler that serves the consent bootstrap + crumb, then delegates the
    /// *actual* request (anything not fc.yahoo.com / getcrumb) to `actual`.
    fn handler_with(
        actual: impl Fn(&TransportRequest) -> YfResult<TransportResponse> + Send + Sync + 'static,
    ) -> impl Fn(&TransportRequest) -> YfResult<TransportResponse> + Send + Sync + 'static {
        move |req: &TransportRequest| {
            if req.url.contains("getcrumb") {
                return resp(200, "test-crumb");
            }
            if req.url == "https://fc.yahoo.com" {
                return resp(200, "");
            }
            actual(req)
        }
    }

    fn query_map(req: &TransportRequest) -> std::collections::HashMap<String, String> {
        req.query
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[tokio::test]
    async fn get_json_parses_and_injects_crumb_locale() {
        let t = MockTransport::new(handler_with(|_| resp(200, r#"{"ok":true}"#)));
        let s = session(0, t.clone());
        let v: Value = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[("interval", "1d".into())],
            )
            .await
            .expect("ok");
        assert_eq!(v["ok"], true);

        let actual = t.find("chart/AAPL").expect("actual request recorded");
        let q = query_map(&actual);
        assert_eq!(q.get("crumb"), Some(&"test-crumb".to_string()));
        assert_eq!(q.get("lang"), Some(&"en".to_string()));
        assert_eq!(q.get("region"), Some(&"US".to_string()));
        assert_eq!(q.get("interval"), Some(&"1d".to_string()));
        assert!(
            actual
                .headers
                .iter()
                .any(|(k, v)| k == "accept-language" && v == "en-US")
        );
    }

    #[tokio::test]
    async fn get_json_retries_on_5xx_then_succeeds() {
        let calls = Arc::new(AtomicU64::new(0));
        let c = calls.clone();
        let t = MockTransport::new(handler_with(move |_| {
            let n = c.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                resp(503, "try later")
            } else {
                resp(200, r#"{"ok":true}"#)
            }
        }));
        let s = session(2, t.clone());
        let v: Value = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[],
            )
            .await
            .expect("ok after retries");
        assert_eq!(v["ok"], true);
        // 503, 503, 200 -> three actual attempts; crumb stays cached after first.
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(t.find("getcrumb").is_some(), true);
    }

    #[tokio::test]
    async fn get_json_401_retries_with_reset_auth() {
        let calls = Arc::new(AtomicU64::new(0));
        let c = calls.clone();
        let t = MockTransport::new(handler_with(move |_| {
            let n = c.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                resp(401, "expired")
            } else {
                resp(200, r#"{"ok":true}"#)
            }
        }));
        let s = session(2, t.clone());
        let v: Value = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[],
            )
            .await
            .expect("recovered after 401");
        assert_eq!(v["ok"], true);
        // 401 then 200 -> two actual attempts; reset_auth forces a crumb re-fetch.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let crumb_hits = t
            .requests()
            .iter()
            .filter(|r| r.url.contains("getcrumb"))
            .count();
        assert_eq!(crumb_hits, 2, "crumb re-fetched after reset_auth");
    }

    #[tokio::test]
    async fn get_json_429_gives_up_immediately() {
        let t = MockTransport::new(handler_with(|_| resp(429, "rate limited")));
        let s = session(3, t.clone());
        let err = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[],
            )
            .await
            .expect_err("429 must not retry");
        assert!(matches!(err, YfError::RateLimited));
    }

    #[tokio::test]
    async fn get_json_404_gives_up_with_status_body() {
        let t = MockTransport::new(handler_with(|_| resp(404, r#"{"error":"not found"}"#)));
        let s = session(0, t.clone());
        let err = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[],
            )
            .await
            .expect_err("404 is terminal");
        match err {
            YfError::Status { status, body } => {
                assert_eq!(status, 404);
                assert!(body.contains("not found"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_text_success_without_crumb() {
        let t = MockTransport::new(handler_with(|_| resp(200, "<html>hi</html>")));
        let s = session(0, t.clone());
        let body = s
            .get_text("https://query2.finance.yahoo.com/suggest/ISIN", &[])
            .await
            .expect("ok");
        assert_eq!(body, "<html>hi</html>");
        let actual = t.find("suggest/ISIN").expect("actual recorded");
        assert!(
            !query_map(&actual).contains_key("crumb"),
            "text endpoint never adds crumb"
        );
    }

    #[tokio::test]
    async fn get_text_401_not_retried() {
        let calls = Arc::new(AtomicU64::new(0));
        let c = calls.clone();
        let t = MockTransport::new(handler_with(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            resp(401, "forbidden")
        }));
        let s = session(3, t.clone());
        let err = s
            .get_text("https://query2.finance.yahoo.com/suggest/ISIN", &[])
            .await
            .expect_err("text 401 is terminal");
        assert!(matches!(err, YfError::Msg(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no retry on text 401");
    }

    #[tokio::test]
    async fn post_json_success() {
        let t = MockTransport::new(handler_with(|req| {
            assert_eq!(req.method, TransportMethod::Post);
            assert!(req.json_body.is_some(), "post body forwarded");
            resp(200, r#"{"posted":true}"#)
        }));
        let s = session(0, t.clone());
        let v: Value = s
            .post_json(
                "https://query1.finance.yahoo.com/v1/portal/data",
                &[],
                &json!({"a": 1}),
            )
            .await
            .expect("ok");
        assert_eq!(v["posted"], true);
    }

    #[tokio::test]
    async fn post_json_error_propagates() {
        let t = MockTransport::new(handler_with(|_| resp(500, "boom")));
        let s = session(0, t.clone());
        let err = s
            .post_json(
                "https://query1.finance.yahoo.com/v1/portal/data",
                &[],
                &json!({}),
            )
            .await
            .expect_err("500 terminal");
        match err {
            YfError::Status { status, .. } => assert_eq!(status, 500),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn crumb_is_cached_across_calls() {
        let t = MockTransport::new(handler_with(|_| resp(200, r#"{"ok":true}"#)));
        let s = session(0, t.clone());
        let _ = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/AAPL",
                &[],
            )
            .await
            .unwrap();
        let _ = s
            .get_json(
                "https://query1.finance.yahoo.com/v8/finance/chart/MSFT",
                &[],
            )
            .await
            .unwrap();
        let crumb_hits = t
            .requests()
            .iter()
            .filter(|r| r.url.contains("getcrumb"))
            .count();
        assert_eq!(crumb_hits, 1, "crumb fetched once then served from cache");
    }

    #[tokio::test]
    async fn check_login_true_when_guid_present() {
        let t = MockTransport::new(move |req: &TransportRequest| {
            if req.url.contains("subscriptions") {
                resp(200, r#"{"result":{"guid":"abc-123"}}"#)
            } else {
                resp(200, "")
            }
        });
        let s = session(0, t.clone());
        assert!(s.check_login().await.expect("ok"));
    }

    #[tokio::test]
    async fn check_login_false_without_guid() {
        let t = MockTransport::new(move |req: &TransportRequest| {
            if req.url.contains("subscriptions") {
                resp(200, r#"{"result":{}}"#)
            } else {
                resp(200, "")
            }
        });
        let s = session(0, t.clone());
        assert!(!s.check_login().await.expect("ok"));
    }

    #[tokio::test]
    async fn set_login_cookies_reports_validity() {
        let t = MockTransport::new(move |req: &TransportRequest| {
            if req.url.contains("subscriptions") {
                resp(200, r#"{"result":{"guid":"user-guid"}}"#)
            } else {
                resp(200, "")
            }
        });
        let s = session(0, t.clone());
        assert!(s.set_login_cookies("TVAL", "YVAL").await.expect("ok"));
    }
}
