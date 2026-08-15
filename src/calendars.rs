//! Calendars (mirrors yfinance's `Calendars`): earnings / IPO / economic /
//! splits, served by the `v1/finance/visualization` endpoint.
//!
//! yfinance 1.6.0 builds a `CalendarQuery` (`operator` + `operands`) and POSTs
//! it with an `entityIdType` identifying the calendar; the response comes back
//! as `finance.result[0].documents[0]` with parallel `columns` / `rows`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, YfError};
use crate::http::YfSession;
use crate::json::yf_result_first;

const VIZ_URL: &str = "https://query1.finance.yahoo.com/v1/finance/visualization";

/// Predefined calendar metadata (mirrors `yfinance.calendars.PREDEFINED_CALENDARS`).
struct Predef {
    sort_field: &'static str,
    include_fields: &'static [&'static str],
}

const PREDEF: &[(&str, Predef)] = &[
    (
        "sp_earnings",
        Predef {
            sort_field: "intradaymarketcap",
            include_fields: &[
                "ticker",
                "companyshortname",
                "intradaymarketcap",
                "eventname",
                "startdatetime",
                "startdatetimetype",
                "epsestimate",
                "epsactual",
                "epssurprisepct",
            ],
        },
    ),
    (
        "ipo_info",
        Predef {
            sort_field: "startdatetime",
            include_fields: &[
                "ticker",
                "companyshortname",
                "exchange_short_name",
                "filingdate",
                "startdatetime",
                "amendeddate",
                "pricefrom",
                "priceto",
                "offerprice",
                "currencyname",
                "shares",
                "dealtype",
            ],
        },
    ),
    (
        "economic_event",
        Predef {
            sort_field: "startdatetime",
            include_fields: &[
                "econ_release",
                "country_code",
                "startdatetime",
                "period",
                "after_release_actual",
                "consensus_estimate",
                "prior_release_actual",
                "originally_reported_actual",
            ],
        },
    ),
    (
        "splits",
        Predef {
            sort_field: "startdatetime",
            include_fields: &[
                "ticker",
                "companyshortname",
                "startdatetime",
                "optionable",
                "old_share_worth",
                "share_worth",
            ],
        },
    ),
];

fn predef(cal_type: &str) -> Result<&'static Predef> {
    PREDEF
        .iter()
        .find(|(k, _)| *k == cal_type)
        .map(|(_, p)| p)
        .ok_or_else(|| YfError::NotSupported(format!("unknown calendar type '{cal_type}'")))
}

/// One operand of a [`CalendarQuery`]: a nested sub-query or a literal value
/// (which may be a field name or a comparison value).
#[derive(Debug, Clone)]
pub enum CalendarOperand {
    Query(Box<CalendarQuery>),
    Value(Value),
}

impl CalendarOperand {
    /// A literal value / field-name operand.
    pub fn value(v: impl Into<Value>) -> CalendarOperand {
        CalendarOperand::Value(v.into())
    }
    /// A nested sub-query operand.
    pub fn query(q: CalendarQuery) -> CalendarOperand {
        CalendarOperand::Query(Box::new(q))
    }
}

/// Mirror of yfinance's `CalendarQuery` (operator + operands).
#[derive(Debug, Clone)]
pub struct CalendarQuery {
    pub operator: String,
    pub operands: Vec<CalendarOperand>,
}

impl CalendarQuery {
    /// Build a query. Operator is normalized to uppercase.
    pub fn new(operator: &str, operands: Vec<CalendarOperand>) -> CalendarQuery {
        CalendarQuery {
            operator: operator.to_uppercase(),
            operands,
        }
    }

    /// Serialize to the Yahoo `{"operator", "operands"}` shape.
    pub fn to_dict(&self) -> Value {
        let operands: Vec<Value> = self
            .operands
            .iter()
            .map(|o| match o {
                CalendarOperand::Query(q) => q.to_dict(),
                CalendarOperand::Value(v) => v.clone(),
            })
            .collect();
        serde_json::json!({"operator": self.operator, "operands": operands})
    }
}

/// A calendar column descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarColumn {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub col_type: Option<String>,
}

/// A parsed calendar response: parallel `columns` and `rows` (mirrors the
/// `documents[0]` structure yfinance turns into a DataFrame).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarResult {
    pub calendar_type: String,
    pub columns: Vec<CalendarColumn>,
    pub rows: Vec<Vec<Value>>,
}

impl CalendarResult {
    /// Zip columns with each row into a map keyed by column label.
    pub fn records(&self) -> Vec<HashMap<String, Value>> {
        self.rows
            .iter()
            .map(|row| {
                let mut m = HashMap::new();
                for (i, col) in self.columns.iter().enumerate() {
                    if let Some(v) = row.get(i) {
                        m.insert(col.label.clone(), v.clone());
                    }
                }
                m
            })
            .collect()
    }
}

/// An earnings-calendar event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EarningsEvent {
    pub ticker: Option<String>,
    pub companies: Option<String>,
    pub start_date: Option<String>,
    pub time: Option<String>,
    pub eps_estimate: Option<f64>,
    pub eps_actual: Option<f64>,
}

/// An IPO-calendar event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IpoEvent {
    pub ticker: Option<String>,
    pub companies: Option<String>,
    pub start_date: Option<String>,
    pub ipo_price: Option<f64>,
    pub currency: Option<String>,
}

/// An economic-calendar event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EconomicEvent {
    pub ticker: Option<String>,
    pub companies: Option<String>,
    pub start_date: Option<String>,
    pub time: Option<String>,
}

/// A splits-calendar event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SplitEvent {
    pub ticker: Option<String>,
    pub companies: Option<String>,
    pub start_date: Option<String>,
    pub time: Option<String>,
}

fn str_field(rec: &HashMap<String, Value>, key: &str) -> Option<String> {
    rec.get(key).and_then(|x| x.as_str()).map(String::from)
}

fn num_field(rec: &HashMap<String, Value>, key: &str) -> Option<f64> {
    rec.get(key).and_then(|x| x.as_f64())
}

/// Zip a row with its `include_fields` (Yahoo returns columns in request order)
/// so we can map by stable internal field names rather than human labels.
fn zip_fields(row: &[Value], fields: &[&str]) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    for (i, f) in fields.iter().enumerate() {
        if let Some(v) = row.get(i) {
            m.insert((*f).to_string(), v.clone());
        }
    }
    m
}

fn parse_documents(cal_type: &str, v: &Value) -> Result<CalendarResult> {
    let doc = yf_result_first(v, "finance")
        .map_err(|_| YfError::DataMissing(format!("calendar {cal_type}")))?
        .get("documents")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| YfError::DataMissing(format!("calendar {cal_type}")))?;

    let columns: Vec<CalendarColumn> = doc
        .get("columns")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    let raw_label = c.get("label").and_then(|x| x.as_str()).unwrap_or("");
                    // yfinance renames a duplicate "Event Start Date" (STRING) to "Timing".
                    let label = if raw_label == "Event Start Date"
                        && c.get("type").and_then(|x| x.as_str()) == Some("STRING")
                    {
                        "Timing"
                    } else {
                        raw_label
                    };
                    CalendarColumn {
                        name: c
                            .get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or(label)
                            .to_string(),
                        label: label.to_string(),
                        col_type: c.get("type").and_then(|x| x.as_str()).map(String::from),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<Vec<Value>> = doc
        .get("rows")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| row.as_array())
                .map(|a| a.to_vec())
                .collect()
        })
        .unwrap_or_default();

    Ok(CalendarResult {
        calendar_type: cal_type.to_string(),
        columns,
        rows,
    })
}

impl YfSession {
    /// Low-level calendar fetch (mirrors `Calendars._get_data`). `limit` is
    /// capped at 100 by Yahoo.
    pub async fn calendar(
        &self,
        cal_type: &str,
        query: &CalendarQuery,
        limit: usize,
        offset: usize,
    ) -> Result<CalendarResult> {
        let p = predef(cal_type)?;
        let body = serde_json::json!({
            "sortType": "DESC",
            "entityIdType": cal_type,
            "sortField": p.sort_field,
            "includeFields": p.include_fields,
            "size": limit.min(100),
            "offset": offset,
            "query": query.to_dict(),
        });
        let params: Vec<(&str, String)> =
            vec![("lang", "en-US".to_string()), ("region", "US".to_string())];
        let v = self.post_json(VIZ_URL, &params, &body).await?;
        parse_documents(cal_type, &v)
    }

    /// Earnings calendar between two `YYYY-MM-DD` dates.
    pub async fn earnings_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<EarningsEvent>> {
        let query = CalendarQuery::new(
            "and",
            vec![
                CalendarOperand::query(CalendarQuery::new(
                    "eq",
                    vec![
                        CalendarOperand::value("region"),
                        CalendarOperand::value("us"),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "or",
                    vec![
                        CalendarOperand::query(CalendarQuery::new(
                            "eq",
                            vec![
                                CalendarOperand::value("eventtype"),
                                CalendarOperand::value("EAD"),
                            ],
                        )),
                        CalendarOperand::query(CalendarQuery::new(
                            "eq",
                            vec![
                                CalendarOperand::value("eventtype"),
                                CalendarOperand::value("ERA"),
                            ],
                        )),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "gte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(start),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "lte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(end),
                    ],
                )),
            ],
        );
        let res = self.calendar("sp_earnings", &query, limit, 0).await?;
        let p = predef("sp_earnings")?;
        Ok(res
            .rows
            .iter()
            .map(|row| {
                let r = zip_fields(row, p.include_fields);
                EarningsEvent {
                    ticker: str_field(&r, "ticker"),
                    companies: str_field(&r, "companyshortname"),
                    start_date: str_field(&r, "startdatetime"),
                    time: str_field(&r, "startdatetimetype"),
                    eps_estimate: num_field(&r, "epsestimate"),
                    eps_actual: num_field(&r, "epsactual"),
                }
            })
            .collect())
    }

    /// IPO calendar between two `YYYY-MM-DD` dates.
    pub async fn ipo_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<IpoEvent>> {
        let query = CalendarQuery::new(
            "or",
            vec![
                CalendarOperand::query(CalendarQuery::new(
                    "gtelt",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(start),
                        CalendarOperand::value(end),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "gtelt",
                    vec![
                        CalendarOperand::value("filingdate"),
                        CalendarOperand::value(start),
                        CalendarOperand::value(end),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "gtelt",
                    vec![
                        CalendarOperand::value("amendeddate"),
                        CalendarOperand::value(start),
                        CalendarOperand::value(end),
                    ],
                )),
            ],
        );
        let res = self.calendar("ipo_info", &query, limit, 0).await?;
        let p = predef("ipo_info")?;
        Ok(res
            .rows
            .iter()
            .map(|row| {
                let r = zip_fields(row, p.include_fields);
                IpoEvent {
                    ticker: str_field(&r, "ticker"),
                    companies: str_field(&r, "companyshortname"),
                    start_date: str_field(&r, "startdatetime"),
                    ipo_price: num_field(&r, "offerprice"),
                    currency: str_field(&r, "currencyname"),
                }
            })
            .collect())
    }

    /// Economic calendar between two `YYYY-MM-DD` dates.
    pub async fn economic_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<EconomicEvent>> {
        let query = CalendarQuery::new(
            "and",
            vec![
                CalendarOperand::query(CalendarQuery::new(
                    "gte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(start),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "lte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(end),
                    ],
                )),
            ],
        );
        let res = self.calendar("economic_event", &query, limit, 0).await?;
        let p = predef("economic_event")?;
        Ok(res
            .rows
            .iter()
            .map(|row| {
                let r = zip_fields(row, p.include_fields);
                EconomicEvent {
                    ticker: str_field(&r, "econ_release"),
                    companies: str_field(&r, "country_code"),
                    start_date: str_field(&r, "startdatetime"),
                    time: str_field(&r, "period"),
                }
            })
            .collect())
    }

    /// Splits calendar between two `YYYY-MM-DD` dates.
    pub async fn splits_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<SplitEvent>> {
        let query = CalendarQuery::new(
            "and",
            vec![
                CalendarOperand::query(CalendarQuery::new(
                    "gte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(start),
                    ],
                )),
                CalendarOperand::query(CalendarQuery::new(
                    "lte",
                    vec![
                        CalendarOperand::value("startdatetime"),
                        CalendarOperand::value(end),
                    ],
                )),
            ],
        );
        let res = self.calendar("splits", &query, limit, 0).await?;
        let p = predef("splits")?;
        Ok(res
            .rows
            .iter()
            .map(|row| {
                let r = zip_fields(row, p.include_fields);
                SplitEvent {
                    ticker: str_field(&r, "ticker"),
                    companies: str_field(&r, "companyshortname"),
                    start_date: str_field(&r, "startdatetime"),
                    time: None,
                }
            })
            .collect())
    }
}
