//! Runtime configuration for sessions and clients.
//!
//! Mirrors yfinance's `YfConfig` (network proxy/retries, debug hide_exceptions,
//! locale lang/region) as a plain builder-style struct.

/// Locale sent with every `quoteSummary` / visualization request.
#[derive(Debug, Clone)]
pub struct Locale {
    /// Language tag, e.g. `"en"`.
    pub lang: String,
    /// Region tag, e.g. `"US"`.
    pub region: String,
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            lang: "en".into(),
            region: "US".into(),
        }
    }
}

/// Configuration for a [`crate::http::YfSession`] / [`crate::Client`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Optional HTTP proxy (e.g. `"http://127.0.0.1:7890"`).
    pub proxy: Option<String>,
    /// Number of retries on transient failures (yfinance default: 0).
    pub retries: u32,
    /// Per-request timeout in seconds.
    pub timeout_secs: u64,
    /// User-Agent header sent with every request.
    pub user_agent: String,
    /// Locale for summary/visualization requests.
    pub locale: Locale,
    /// When true, bulk [`crate::download`] swallows per-ticker errors instead of
    /// aborting (mirrors yfinance's `hide_exceptions`).
    pub lenient: bool,
    /// Optional on-disk cache directory. Defaults to a temp dir when `None`.
    pub cache_dir: Option<std::path::PathBuf>,
    /// Optional Yahoo login `T` cookie (mirrors yfinance's `Auth` cookies).
    pub cookie_t: Option<String>,
    /// Optional Yahoo login `Y` cookie.
    pub cookie_y: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy: None,
            retries: 0,
            timeout_secs: 30,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
                .into(),
            locale: Locale::default(),
            lenient: true,
            cache_dir: None,
            cookie_t: None,
            cookie_y: None,
        }
    }
}

impl Config {
    /// Builder: set proxy.
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Builder: set retries.
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Builder: set request timeout.
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Builder: set locale.
    pub fn locale(mut self, lang: impl Into<String>, region: impl Into<String>) -> Self {
        self.locale = Locale {
            lang: lang.into(),
            region: region.into(),
        };
        self
    }

    /// Builder: toggle lenient bulk mode.
    pub fn lenient(mut self, lenient: bool) -> Self {
        self.lenient = lenient;
        self
    }

    /// Builder: set cache directory.
    pub fn cache_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.cache_dir = Some(dir);
        self
    }

    /// Builder: inject Yahoo login `T`/`Y` cookies at session start
    /// (mirrors yfinance's `Auth.set_login_cookies`).
    pub fn cookies(mut self, cookie_t: impl Into<String>, cookie_y: impl Into<String>) -> Self {
        self.cookie_t = Some(cookie_t.into());
        self.cookie_y = Some(cookie_y.into());
        self
    }
}
