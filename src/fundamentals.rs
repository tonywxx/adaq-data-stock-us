//! Financial statements: income / balance-sheet / cash-flow.
//!
//! Mirrors yfinance's `scrapers/fundamentals.py`. We read the `*History`
//! modules from `quoteSummary` (the same data, simpler shape than the
//! `fundamentals-timeseries` endpoint) and reshape into a typed matrix.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Result, YfError};
use crate::http::YfSession;
use crate::json::{get_f64, get_i64};
use crate::quote::quote_summary;

/// Frequency of a financial statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Freq {
    Annual,
    Quarterly,
}

impl Freq {
    /// Human-readable frequency label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Freq::Annual => "annual",
            Freq::Quarterly => "quarterly",
        }
    }
}

/// Which statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    Income,
    BalanceSheet,
    CashFlow,
}

/// A parsed financial statement as a matrix of line-item × period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Financials {
    pub statement: &'static str,
    pub freq: &'static str,
    /// Period end dates, ascending.
    pub dates: Vec<String>,
    /// Line-item names (union across periods).
    pub items: Vec<String>,
    /// item name → date → value.
    pub values: HashMap<String, HashMap<String, Option<f64>>>,
}

impl Financials {
    /// Look up a single cell.
    pub fn get(&self, item: &str, date: &str) -> Option<f64> {
        self.values
            .get(item)
            .and_then(|m| m.get(date).copied().flatten())
    }

    /// All values for a line item, in date order.
    pub fn series(&self, item: &str) -> Vec<Option<f64>> {
        self.dates
            .iter()
            .map(|d| {
                self.values
                    .get(item)
                    .and_then(|m| m.get(d).copied().flatten())
            })
            .collect()
    }
}

impl YfSession {
    /// Fetch a financial statement.
    pub async fn financials(
        &self,
        ticker: &str,
        statement: Statement,
        freq: Freq,
    ) -> Result<Financials> {
        let module = module_name(statement, freq);
        let result = quote_summary(self, ticker, &[module]).await?;
        let history = dig_history(&result, module)
            .ok_or_else(|| YfError::DataMissing(format!("{module} for {ticker}")))?;

        let mut dates: Vec<String> = Vec::new();
        let mut per_period: Vec<(String, HashMap<String, Option<f64>>)> = Vec::new();
        let mut item_set: std::collections::BTreeSet<String> = Default::default();

        for period in history {
            let date = period_date(period).unwrap_or_default();
            if date.is_empty() {
                continue;
            }
            let mut row: HashMap<String, Option<f64>> = HashMap::new();
            if let Some(obj) = period.as_object() {
                for (k, v) in obj {
                    if k == "endDate" || k == "fiscalDateEnd" || k == "maxAge" {
                        continue;
                    }
                    if let Some(val) = get_f64(v, &[]) {
                        row.insert(k.clone(), Some(val));
                        item_set.insert(k.clone());
                    } else if v.is_null() {
                        row.insert(k.clone(), None);
                        item_set.insert(k.clone());
                    }
                }
            }
            dates.push(date.clone());
            per_period.push((date, row));
        }

        let items: Vec<String> = item_set.into_iter().collect();
        let mut values: HashMap<String, HashMap<String, Option<f64>>> = HashMap::new();
        for item in &items {
            let mut m = HashMap::new();
            for (date, row) in &per_period {
                m.insert(date.clone(), row.get(item).copied().flatten());
            }
            values.insert(item.clone(), m);
        }

        Ok(Financials {
            statement: statement_name(statement),
            freq: freq_name(freq),
            dates,
            items,
            values,
        })
    }
}

fn module_name(statement: Statement, freq: Freq) -> &'static str {
    match (statement, freq) {
        (Statement::Income, Freq::Annual) => "incomeStatementHistory",
        (Statement::Income, Freq::Quarterly) => "incomeStatementHistoryQuarterly",
        (Statement::BalanceSheet, Freq::Annual) => "balanceSheetHistory",
        (Statement::BalanceSheet, Freq::Quarterly) => "balanceSheetHistoryQuarterly",
        (Statement::CashFlow, Freq::Annual) => "cashflowStatementHistory",
        (Statement::CashFlow, Freq::Quarterly) => "cashflowStatementHistoryQuarterly",
    }
}

fn statement_name(s: Statement) -> &'static str {
    match s {
        Statement::Income => "income",
        Statement::BalanceSheet => "balance-sheet",
        Statement::CashFlow => "cash-flow",
    }
}

fn freq_name(f: Freq) -> &'static str {
    match f {
        Freq::Annual => "annual",
        Freq::Quarterly => "quarterly",
    }
}

fn dig_history<'a>(
    result: &'a serde_json::Value,
    module: &str,
) -> Option<Vec<&'a serde_json::Value>> {
    result
        .get(module)
        // yfinance nests the statement array under the module name itself
        // (single-module requests) or under "history".
        .and_then(|m| m.get(module).or_else(|| m.get("history")))
        .and_then(|h| h.as_array())
        .map(|a| a.iter().collect())
}

fn period_date(period: &serde_json::Value) -> Option<String> {
    let raw = get_i64(period, &["endDate"]).or_else(|| get_i64(period, &["fiscalDateEnd"]));
    if let Some(sec) = raw
        && let Some(d) = chrono::DateTime::from_timestamp(sec, 0)
    {
        return Some(d.date_naive().format("%Y-%m-%d").to_string());
    }
    period
        .get("endDate")
        .and_then(|e| e.get("fmt"))
        .and_then(|f| f.as_str())
        .map(|s| s.to_string())
}
