//! Search (mirrors yfinance's `Search`).

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::http::YfSession;

const SEARCH_URL: &str = "https://query1.finance.yahoo.com/v1/finance/search";

/// A search result quote.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchQuote {
    pub symbol: Option<String>,
    #[serde(rename = "shortname")]
    pub short_name: Option<String>,
    #[serde(rename = "longname")]
    pub long_name: Option<String>,
    #[serde(rename = "exchDisp")]
    pub exchange: Option<String>,
    #[serde(rename = "typeDisp")]
    pub quote_type: Option<String>,
    pub score: Option<f64>,
}

/// A search result news item.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchNews {
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
    pub publisher: Option<String>,
}

/// Search results (mirrors `Search.quotes` / `Search.news`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResult {
    pub query: String,
    pub quotes: Vec<SearchQuote>,
    pub news: Vec<SearchNews>,
}

impl YfSession {
    /// Search Yahoo Finance for a query.
    pub async fn search(
        &self,
        query: &str,
        quotes_count: usize,
        news_count: usize,
    ) -> Result<SearchResult> {
        let params = vec![
            ("q", query.to_string()),
            ("quotesCount", quotes_count.to_string()),
            ("newsCount", news_count.to_string()),
            ("enableFuzzyQuery", "false".to_string()),
            ("quotesQueryId", "tss_match_phrase_query".to_string()),
        ];
        let v = self.get_json(SEARCH_URL, &params).await?;
        let quotes = v
            .get("quotes")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|q| serde_json::from_value::<SearchQuote>(q.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        let news = v
            .get("news")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|q| serde_json::from_value::<SearchNews>(q.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(SearchResult {
            query: query.to_string(),
            quotes,
            news,
        })
    }
}
