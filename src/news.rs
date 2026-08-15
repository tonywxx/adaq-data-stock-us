//! Per-ticker news feed (mirrors `Ticker.get_news`).
//!
//! Fetches the latest news articles for a ticker from Yahoo's `ncp` stream
//! endpoint. Articles carrying an `ad` payload are filtered out, mirroring
//! yfinance.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, YfError};
use crate::http::YfSession;

const NCP_URL: &str = "https://finance.yahoo.com/xhr/ncp";

/// A single news article for a ticker.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewsArticle {
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
    pub publisher: Option<String>,
    #[serde(rename = "providerPublishTime")]
    pub provider_publish_time: Option<i64>,
    #[serde(rename = "relatedTickers")]
    pub related_tickers: Vec<String>,
    /// Raw thumbnail object from Yahoo (`url` / `originalUrl` / `resolutions`).
    pub thumbnail: Option<Value>,
    #[serde(rename = "type")]
    pub article_type: Option<String>,
}

impl NewsArticle {
    /// Best-effort thumbnail URL: prefer `originalUrl`, fall back to `url`.
    pub fn thumbnail_url(&self) -> Option<String> {
        let t = self.thumbnail.as_ref()?;
        t.get("originalUrl")
            .and_then(|v| v.as_str())
            .or_else(|| t.get("url").and_then(|v| v.as_str()))
            .map(String::from)
    }
}

impl YfSession {
    /// Fetch `count` latest news articles for `ticker` (mirrors `get_news`).
    ///
    /// `tab` selects the feed, mirroring yfinance's `tab` argument:
    /// `"news"` (default), `"all"`, or `"press releases"`.
    pub async fn news(&self, ticker: &str, count: usize, tab: &str) -> Result<Vec<NewsArticle>> {
        let query_ref = match tab.to_ascii_lowercase().as_str() {
            "all" => "newsAll",
            "press releases" | "pressrelease" => "pressRelease",
            _ => "latestNews",
        };
        let url = format!("{NCP_URL}?queryRef={query_ref}&serviceKey=ncp_fin");
        let body = serde_json::json!({
            "serviceConfig": {
                "snippetCount": count,
                "s": [ticker],
            }
        });
        let v = self.post_json(&url, &[], &body).await?;
        let stream = v
            .get("data")
            .and_then(|d| d.get("tickerStream"))
            .and_then(|ts| ts.get("stream"))
            .and_then(|s| s.as_array());
        let stream = match stream {
            Some(s) => s,
            None => {
                return Err(YfError::DataMissing(format!(
                    "news stream missing for {ticker}"
                )));
            }
        };
        Ok(stream
            .iter()
            .filter(|a| {
                a.get("ad")
                    .and_then(|ad| ad.as_array())
                    .map(|x| x.is_empty())
                    .unwrap_or(true)
            })
            .filter_map(|a| serde_json::from_value::<NewsArticle>(a.clone()).ok())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_article_without_ads() {
        let stream = json!([
            {
                "uuid": "u1",
                "title": "Apple earnings",
                "link": "https://x/1",
                "publisher": "Reuters",
                "providerPublishTime": 1700000000i64,
                "type": "STORY",
                "relatedTickers": ["AAPL"],
                "thumbnail": {"originalUrl": "https://img/1", "url": "https://img/1s"}
            },
            { "ad": ["some sponsor"] }
        ]);
        let v = json!({ "data": { "tickerStream": { "stream": stream } } });
        let stream = v["data"]["tickerStream"]["stream"].as_array().unwrap();
        let articles: Vec<NewsArticle> = stream
            .iter()
            .filter(|a| {
                a.get("ad")
                    .and_then(|ad| ad.as_array())
                    .map(|x| x.is_empty())
                    .unwrap_or(true)
            })
            .filter_map(|a| serde_json::from_value::<NewsArticle>(a.clone()).ok())
            .collect();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title.as_deref(), Some("Apple earnings"));
        assert_eq!(articles[0].article_type.as_deref(), Some("STORY"));
        assert_eq!(
            articles[0].thumbnail_url().as_deref(),
            Some("https://img/1")
        );
    }
}
