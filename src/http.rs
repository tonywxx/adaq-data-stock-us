//! HTTP session: the `YfSession` is the shared, thread-safe core that owns the
//! impersonating HTTP client, cookie/consent bootstrap, crumb, retry/backoff,
//! and the sqlite cache. Mirrors yfinance's `YfData` singleton.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use primp::{Client, Impersonate};
use serde_json::Value;

use crate::cache::Cache;
use crate::config::Config;
use crate::error::{Result, YfError};

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
    client: Client,
    config: Config,
    cache: Cache,
    cookie_jar: Arc<primp::cookie::Jar>,
    crumb: Mutex<Option<String>>,
    cookie_ready: Mutex<bool>,
}

impl YfSession {
    /// Build a new session from config.
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
        let cache = Cache::open(config.cache_dir.clone())?;
        let session = Self {
            inner: Arc::new(SessionInner {
                client,
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
            .client
            .get("https://fc.yahoo.com")
            .header("accept", "*/*")
            .send()
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
            .client
            .get(&url)
            .header("accept", "*/*")
            .send()
            .await?;
        let crumb = resp.text().await?.trim().to_string();
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
        let mut qp: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let crumb = self.get_crumb().await.unwrap_or_default();
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

        let retries = self.inner.config.retries;
        let mut attempt: u32 = 0;
        loop {
            self.ensure_cookie().await?;
            let req = self
                .inner
                .client
                .get(url)
                .header("accept", "*/*")
                .header(
                    "accept-language",
                    format!(
                        "{}-{}",
                        self.inner.config.locale.lang, self.inner.config.locale.region
                    ),
                )
                .query(&qp);
            let resp = req.send().await;
            let resp = match resp {
                Ok(r) => r,
                Err(_e) if attempt < retries => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
                    continue;
                }
                Err(e) => return Err(YfError::Http(e)),
            };
            let status = resp.status();
            if status == 401 {
                self.reset_auth();
                if attempt < retries {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
                    continue;
                }
                return Err(YfError::msg("unauthorized (401) after retries"));
            }
            if status == 429 {
                return Err(YfError::RateLimited);
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                if attempt < retries && status.is_server_error() {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
                    continue;
                }
                return Err(YfError::Status {
                    status: status.as_u16(),
                    body,
                });
            }
            let value: Value = resp.json().await?;
            return Ok(value);
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
            let req = self
                .inner
                .client
                .get(url)
                .header("accept", "*/*")
                .query(&params);
            let resp = match req.send().await {
                Ok(r) => r,
                Err(_e) if attempt < retries => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
                    continue;
                }
                Err(e) => return Err(YfError::Http(e)),
            };
            let status = resp.status();
            if status == 429 {
                return Err(YfError::RateLimited);
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                if attempt < retries && status.is_server_error() {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
                    continue;
                }
                return Err(YfError::Status {
                    status: status.as_u16(),
                    body,
                });
            }
            return Ok(resp.text().await?);
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
            .client
            .post(url)
            .header("accept", "*/*")
            .header("content-type", "application/json")
            .query(&qp)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if status == 429 {
            return Err(YfError::RateLimited);
        }
        if !status.is_success() {
            let b = resp.text().await.unwrap_or_default();
            return Err(YfError::Status {
                status: status.as_u16(),
                body: b,
            });
        }
        Ok(resp.json().await?)
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

// --- Authentication (mirrors yfinance `Auth`) ---

impl YfSession {
    /// Single-shot GET of the OBI subscriptions endpoint, returning the HTTP
    /// status and parsed JSON body (or `Value::Null` if not JSON). Does not
    /// retry and does not require a crumb — used purely for login/entitlement
    /// inspection. Mirrors yfinance's `_SUBSCRIPTIONS_URL` fetch.
    async fn subscriptions(&self) -> Result<(u16, Value)> {
        let resp = self
            .inner
            .client
            .get(SUBSCRIPTIONS_URL)
            .header("accept", "*/*")
            .send()
            .await?;
        let status = resp.status().as_u16();
        let value: Value = resp.json().await.unwrap_or(Value::Null);
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
