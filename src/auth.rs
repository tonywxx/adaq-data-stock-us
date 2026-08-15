//! Authentication (mirrors yfinance's `Auth`): inject the Yahoo `T`/`Y` login
//! cookies and inspect the resulting subscription entitlement.

use crate::error::Result;
use crate::http::YfSession;

/// Login/entitlement helper. Cheap to clone (shares the underlying session).
#[derive(Clone)]
pub struct Auth {
    session: YfSession,
}

impl Auth {
    /// Wrap a session.
    pub fn new(session: YfSession) -> Self {
        Self { session }
    }

    /// Inject the `T`/`Y` login cookies and verify they are valid. Mirrors
    /// `Auth.set_login_cookies`.
    pub async fn set_login_cookies(&self, cookie_t: &str, cookie_y: &str) -> Result<bool> {
        self.session.set_login_cookies(cookie_t, cookie_y).await
    }

    /// Check whether the session is logged in. Mirrors `Auth.check_login`.
    pub async fn check_login(&self) -> Result<bool> {
        self.session.check_login().await
    }

    /// Return the subscription tier (`gold`/`silver`/`bronze`/`premium`/`free`)
    /// or `None` when not logged in. Mirrors `Auth.subscription_tier`.
    pub async fn subscription_tier(&self) -> Result<Option<String>> {
        self.session.subscription_tier().await
    }

    /// Return the logged-in user's `guid`, or `None` when not logged in.
    /// Mirrors `Auth.user`.
    pub async fn user(&self) -> Result<Option<String>> {
        self.session.user().await
    }
}
