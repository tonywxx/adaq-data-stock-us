//! On-disk sqlite cache for persisted data (crumb, ticker→timezone, isin→ticker).
//!
//! yfinance caches these in three sqlite databases; we consolidate them into a
//! single sqlite file with three tables. See `docs/adr/0003-parity-mechanism.md`
//! — the cache is the "persisted data" piece of the design (Q12=B).

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

/// Thin wrapper around a sqlite connection guarded by a mutex (rusqlite
/// connections are not `Sync`).
pub struct Cache {
    conn: Mutex<Connection>,
}

impl Cache {
    /// Open (creating if needed) the cache at `dir/adaq-yfinance.db`.
    /// If `dir` is `None`, uses a temp dir.
    pub fn open(dir: Option<PathBuf>) -> crate::Result<Self> {
        let dir = dir.unwrap_or_else(|| std::env::temp_dir().join("adaq-yfinance-cache"));
        std::fs::create_dir_all(&dir)
            .map_err(|e| crate::YfError::Cache(format!("create cache dir: {e}")))?;
        let path = dir.join("adaq-yfinance.db");
        let conn = Connection::open(&path)
            .map_err(|e| crate::YfError::Cache(format!("open {}: {e}", path.display())))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS crumb (k TEXT PRIMARY KEY, v TEXT, expires INTEGER);
             CREATE TABLE IF NOT EXISTS tz (ticker TEXT PRIMARY KEY, tz TEXT);
             CREATE TABLE IF NOT EXISTS isin (isin TEXT PRIMARY KEY, ticker TEXT);",
        )
        .map_err(|e| crate::YfError::Cache(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Get a cached crumb if present and not expired.
    pub fn get_crumb(&self) -> Option<String> {
        let now = now_secs();
        let guard = self.conn.lock().ok()?;
        let r: Result<(String, i64), _> =
            guard.query_row("SELECT v, expires FROM crumb WHERE k='crumb'", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            });
        match r {
            Ok((v, exp)) if exp > now => Some(v),
            _ => None,
        }
    }

    /// Store a crumb with a TTL (seconds).
    pub fn set_crumb(&self, crumb: &str, ttl_secs: i64) {
        if let Ok(g) = self.conn.lock() {
            let _ = g.execute(
                "INSERT OR REPLACE INTO crumb (k, v, expires) VALUES ('crumb', ?, ?)",
                rusqlite::params![crumb, now_secs() + ttl_secs],
            );
        }
    }

    /// Get cached ticker→timezone.
    pub fn get_tz(&self, ticker: &str) -> Option<String> {
        let g = self.conn.lock().ok()?;
        g.query_row("SELECT tz FROM tz WHERE ticker=?", [ticker], |r| r.get(0))
            .ok()
    }

    /// Cache ticker→timezone.
    pub fn set_tz(&self, ticker: &str, tz: &str) {
        if let Ok(g) = self.conn.lock() {
            let _ = g.execute(
                "INSERT OR REPLACE INTO tz (ticker, tz) VALUES (?, ?)",
                rusqlite::params![ticker, tz],
            );
        }
    }

    /// Get cached isin→ticker.
    pub fn get_isin(&self, isin: &str) -> Option<String> {
        let g = self.conn.lock().ok()?;
        g.query_row("SELECT ticker FROM isin WHERE isin=?", [isin], |r| r.get(0))
            .ok()
    }

    /// Cache isin→ticker.
    pub fn set_isin(&self, isin: &str, ticker: &str) {
        if let Ok(g) = self.conn.lock() {
            let _ = g.execute(
                "INSERT OR REPLACE INTO isin (isin, ticker) VALUES (?, ?)",
                rusqlite::params![isin, ticker],
            );
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
