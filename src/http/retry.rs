//! Pure retry/backoff policy for Yahoo HTTP requests.
//!
//! The decision logic that used to live inline inside `get_json`/`get_text`
//! (status → retry/sleep/give-up, transport error → retry, exponential backoff)
//! is extracted here as plain functions so it can be unit-tested offline
//! without a network. The request loops in `super` are now thin drivers that
//! call [`decide_status`] / [`decide_transport`] and act on the [`Decision`].

use crate::error::YfError;

/// Outcome of evaluating a single request attempt.
#[derive(Debug)]
pub enum Decision {
    /// Request succeeded (HTTP 2xx) — return the body.
    Success,
    /// Transient failure — sleep `backoff` then retry. When `reset_auth` is set
    /// the caller must also clear its cached crumb/cookie before the next
    /// attempt (the 401 path, only meaningful for crumb-authenticated calls).
    Retry {
        backoff: std::time::Duration,
        reset_auth: bool,
    },
    /// Permanent failure — surface `error` to the caller immediately.
    GiveUp { error: YfError },
}

/// Exponential backoff for attempt `attempt` (after it has been incremented):
/// `2^attempt` seconds. `attempt` is the count that was just consumed, so the
/// first retry (attempt == 1) waits 2s, the next 4s, etc.
pub fn backoff_for(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_secs(2u64.saturating_pow(attempt))
}

/// Classify an HTTP `status` from one attempt.
///
/// `attempt` is the number of attempts already consumed (0-based before
/// incrementing). `retries` is the maximum number of *additional* attempts
/// permitted. `retry_401` is `true` only for crumb-authenticated endpoints
/// (`get_json`): a 401 there means the crumb expired and is worth retrying
/// after [`YfSession::reset_auth`]; `get_text` never retries 401.
///
/// Behavior preserved from the original inline logic:
/// - 200/2xx → [`Decision::Success`].
/// - 429 → [`Decision::GiveUp`] with [`YfError::RateLimited`] (never retried).
/// - 401 → if `retry_401` and attempts remain, retry with `reset_auth = true`;
///   otherwise give up with a message error.
/// - other 5xx (server error) → retry if attempts remain, else give up with
///   [`YfError::Status`].
/// - other 4xx → give up with [`YfError::Status`] (not retried).
pub fn decide_status(status: u16, attempt: u32, retries: u32, retry_401: bool) -> Decision {
    if (200..=299).contains(&status) {
        return Decision::Success;
    }
    if status == 429 {
        return Decision::GiveUp {
            error: YfError::RateLimited,
        };
    }
    if status == 401 {
        if retry_401 && attempt < retries {
            return Decision::Retry {
                backoff: backoff_for(attempt + 1),
                reset_auth: true,
            };
        }
        return Decision::GiveUp {
            error: YfError::msg("unauthorized (401) after retries"),
        };
    }
    if (500..600).contains(&status) {
        if attempt < retries {
            return Decision::Retry {
                backoff: backoff_for(attempt + 1),
                reset_auth: false,
            };
        }
        return Decision::GiveUp {
            error: YfError::Status {
                status,
                body: String::new(),
            },
        };
    }
    // Any other non-success status (e.g. 4xx other than 401).
    Decision::GiveUp {
        error: YfError::Status {
            status,
            body: String::new(),
        },
    }
}

/// Classify a transport (connection) error. Retried while attempts remain;
/// otherwise surfaced as the converted error. `err` converts into
/// [`YfError`] (e.g. `primp::Error` via the `#[from]` derive).
pub fn decide_transport<E>(err: E, attempt: u32, retries: u32) -> Decision
where
    E: Into<YfError>,
{
    if attempt < retries {
        Decision::Retry {
            backoff: backoff_for(attempt + 1),
            reset_auth: false,
        }
    } else {
        Decision::GiveUp { error: err.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_success(status: u16, attempt: u32, retries: u32, retry_401: bool) -> bool {
        matches!(
            decide_status(status, attempt, retries, retry_401),
            Decision::Success
        )
    }

    #[test]
    fn success_on_2xx() {
        assert!(is_success(200, 0, 3, true));
        assert!(is_success(204, 2, 3, true));
        assert!(is_success(299, 0, 0, false));
        assert!(!is_success(199, 0, 3, true));
        assert!(!is_success(300, 0, 3, true));
    }

    #[test]
    fn rate_limited_never_retries() {
        // 429 gives up even when attempts remain and even for json (retry_401).
        assert!(matches!(
            decide_status(429, 0, 5, true),
            Decision::GiveUp {
                error: YfError::RateLimited
            }
        ));
        assert!(matches!(
            decide_status(429, 0, 5, false),
            Decision::GiveUp {
                error: YfError::RateLimited
            }
        ));
    }

    #[test]
    fn unauthorized_json_retries_then_gives_up() {
        // json path retries 401 while attempts remain, with reset_auth.
        let d = decide_status(401, 0, 3, true);
        assert!(matches!(
            d,
            Decision::Retry {
                backoff: _,
                reset_auth: true
            }
        ));
        if let Decision::Retry { backoff, .. } = d {
            assert_eq!(backoff, backoff_for(1));
        }
        // out of attempts → give up with message.
        let d = decide_status(401, 3, 3, true);
        assert!(matches!(
            d,
            Decision::GiveUp {
                error: YfError::Msg(_)
            }
        ));
    }

    #[test]
    fn unauthorized_text_never_retries() {
        // get_text (no crumb) gives up on 401 immediately.
        assert!(matches!(
            decide_status(401, 0, 5, false),
            Decision::GiveUp {
                error: YfError::Msg(_)
            }
        ));
    }

    #[test]
    fn server_error_retries_within_budget() {
        let d = decide_status(503, 1, 3, false);
        assert!(matches!(
            d,
            Decision::Retry {
                backoff: _,
                reset_auth: false
            }
        ));
        if let Decision::Retry { backoff, .. } = d {
            assert_eq!(backoff, backoff_for(2));
        }
        // exhausted → Status (empty body).
        let d = decide_status(500, 3, 3, false);
        assert!(matches!(
            d,
            Decision::GiveUp {
                error: YfError::Status { status: 500, body }
            } if body.is_empty()
        ));
    }

    #[test]
    fn client_error_gives_up() {
        // 403 / 404 etc. are not retried (not 401, not 5xx).
        assert!(matches!(
            decide_status(403, 0, 5, true),
            Decision::GiveUp {
                error: YfError::Status { status: 403, body }
            } if body.is_empty()
        ));
        assert!(matches!(
            decide_status(404, 2, 3, false),
            Decision::GiveUp {
                error: YfError::Status { status: 404, body }
            } if body.is_empty()
        ));
    }

    #[test]
    fn transport_retries_then_gives_up() {
        // A synthetic transport error: retry while budget remains.
        let d = decide_transport(YfError::msg("boom"), 0, 3);
        assert!(matches!(
            d,
            Decision::Retry {
                reset_auth: false,
                ..
            }
        ));
        // no budget → GiveUp with the converted error.
        let d = decide_transport(YfError::msg("boom"), 3, 3);
        assert!(matches!(d, Decision::GiveUp { .. }));
    }

    #[test]
    fn backoff_grows_exponentially() {
        assert_eq!(backoff_for(0), std::time::Duration::from_secs(1));
        assert_eq!(backoff_for(1), std::time::Duration::from_secs(2));
        assert_eq!(backoff_for(2), std::time::Duration::from_secs(4));
        assert_eq!(backoff_for(3), std::time::Duration::from_secs(8));
    }

    #[test]
    fn backoff_saturates() {
        // saturating_pow keeps this finite instead of overflowing.
        assert_eq!(
            backoff_for(u32::MAX),
            std::time::Duration::from_secs(u64::MAX)
        );
    }
}
