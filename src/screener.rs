//! Screener (mirrors yfinance's `screener` package): the `EquityQuery` /
//! `FundQuery` / `ETFQuery` query-builder DSL plus `screen()` against Yahoo's
//! screener endpoints.
//!
//! ```no_run
//! use adaq_data_stock_us::screener::*;
//! # async fn run() -> adaq_data_stock_us::Result<()> {
//! # let client = adaq_data_stock_us::Client::new()?;
//! // Predefined screen
//! let res = client.screen("day_gainers", &ScreenOptions::default()).await?;
//!
//! // Custom screen
//! let q = Query::equity(Operator::And, vec![
//!     Operand::query(Query::equity(Operator::Gt, vec![
//!         Operand::field("percentchange"), Operand::value(3.0),
//!     ])),
//!     Operand::query(Query::equity(Operator::Eq, vec![
//!         Operand::field("region"), Operand::value("us"),
//!     ])),
//! ]);
//! let res = client.screen(q, &ScreenOptions::default()).await?;
//! # Ok(()) }
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Result, YfError};
use crate::http::YfSession;

const SCREENER_URL: &str = "https://query1.finance.yahoo.com/v1/finance/screener";
const PREDEFINED_URL: &str =
    "https://query1.finance.yahoo.com/v1/finance/screener/predefined/saved";

// Embedded from yfinance==1.6.0 `PREDEFINED_SCREENER_QUERIES` (each `query`
// pre-serialized via `.to_dict()`), so `screen("<name>")` is faithful offline.
const PREDEFINED_SCREENER_QUERIES: &str = r#"{"aggressive_small_caps":{"sortField":"eodvolume","sortType":"desc","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"OR","operands":[{"operator":"EQ","operands":["exchange","NMS"]},{"operator":"EQ","operands":["exchange","NYQ"]}]},{"operator":"LT","operands":["epsgrowth.lasttwelvemonths",15]}]}},"day_gainers":{"sortField":"percentchange","sortType":"DESC","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"GT","operands":["percentchange",3]},{"operator":"EQ","operands":["region","us"]},{"operator":"GTE","operands":["intradaymarketcap",2000000000]},{"operator":"GTE","operands":["intradayprice",5]},{"operator":"GT","operands":["dayvolume",15000]}]}},"day_losers":{"sortField":"percentchange","sortType":"ASC","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"LT","operands":["percentchange",-2.5]},{"operator":"EQ","operands":["region","us"]},{"operator":"GTE","operands":["intradaymarketcap",2000000000]},{"operator":"GTE","operands":["intradayprice",5]},{"operator":"GT","operands":["dayvolume",20000]}]}},"growth_technology_stocks":{"sortField":"eodvolume","sortType":"desc","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"GTE","operands":["quarterlyrevenuegrowth.quarterly",25]},{"operator":"GTE","operands":["epsgrowth.lasttwelvemonths",25]},{"operator":"EQ","operands":["sector","Technology"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["exchange","NMS"]},{"operator":"EQ","operands":["exchange","NYQ"]}]}]}},"most_actives":{"sortField":"dayvolume","sortType":"DESC","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["region","us"]},{"operator":"GTE","operands":["intradaymarketcap",2000000000]},{"operator":"GT","operands":["dayvolume",5000000]}]}},"most_shorted_stocks":{"sortField":"short_percentage_of_shares_outstanding.value","sortType":"DESC","quoteType":"EQUITY","count":25,"offset":0,"query":{"operator":"AND","operands":[{"operator":"EQ","operands":["region","us"]},{"operator":"GT","operands":["intradayprice",1]},{"operator":"GT","operands":["avgdailyvol3m",200000]}]}},"small_cap_gainers":{"sortField":"eodvolume","sortType":"desc","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"LT","operands":["intradaymarketcap",2000000000]},{"operator":"OR","operands":[{"operator":"EQ","operands":["exchange","NMS"]},{"operator":"EQ","operands":["exchange","NYQ"]}]}]}},"undervalued_growth_stocks":{"sortField":"eodvolume","sortType":"DESC","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"BTWN","operands":["peratio.lasttwelvemonths",0,20]},{"operator":"LT","operands":["pegratio_5y",1]},{"operator":"GTE","operands":["epsgrowth.lasttwelvemonths",25]},{"operator":"OR","operands":[{"operator":"EQ","operands":["exchange","NMS"]},{"operator":"EQ","operands":["exchange","NYQ"]}]}]}},"undervalued_large_caps":{"sortField":"eodvolume","sortType":"desc","quoteType":"EQUITY","query":{"operator":"AND","operands":[{"operator":"BTWN","operands":["peratio.lasttwelvemonths",0,20]},{"operator":"LT","operands":["pegratio_5y",1]},{"operator":"BTWN","operands":["intradaymarketcap",10000000000,100000000000]},{"operator":"OR","operands":[{"operator":"EQ","operands":["exchange","NMS"]},{"operator":"EQ","operands":["exchange","NYQ"]}]}]}},"conservative_foreign_funds":{"sortField":"fundnetassets","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"OR","operands":[{"operator":"EQ","operands":["categoryname","Foreign Large Value"]},{"operator":"EQ","operands":["categoryname","Foreign Large Blend"]},{"operator":"EQ","operands":["categoryname","Foreign Large Growth"]},{"operator":"EQ","operands":["categoryname","Foreign Small/Mid Growth"]},{"operator":"EQ","operands":["categoryname","Foreign Small/Mid Blend"]},{"operator":"EQ","operands":["categoryname","Foreign Small/Mid Value"]}]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"LT","operands":["initialinvestment",100001]},{"operator":"LT","operands":["annualreturnnavy1categoryrank",50]},{"operator":"OR","operands":[{"operator":"EQ","operands":["riskratingoverall",1]},{"operator":"EQ","operands":["riskratingoverall",2]},{"operator":"EQ","operands":["riskratingoverall",3]}]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"high_yield_bond":{"sortField":"fundnetassets","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"LT","operands":["initialinvestment",100001]},{"operator":"LT","operands":["annualreturnnavy1categoryrank",50]},{"operator":"OR","operands":[{"operator":"EQ","operands":["riskratingoverall",1]},{"operator":"EQ","operands":["riskratingoverall",2]},{"operator":"EQ","operands":["riskratingoverall",3]}]},{"operator":"EQ","operands":["categoryname","High Yield Bond"]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"portfolio_anchors":{"sortField":"fundnetassets","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["categoryname","Large Blend"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"LT","operands":["initialinvestment",100001]},{"operator":"LT","operands":["annualreturnnavy1categoryrank",50]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"solid_large_growth_funds":{"sortField":"fundnetassets","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["categoryname","Large Growth"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"LT","operands":["initialinvestment",100001]},{"operator":"LT","operands":["annualreturnnavy1categoryrank",50]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"solid_midcap_growth_funds":{"sortField":"fundnetassets","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["categoryname","Mid-Cap Growth"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"LT","operands":["initialinvestment",100001]},{"operator":"LT","operands":["annualreturnnavy1categoryrank",50]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"top_mutual_funds":{"sortField":"percentchange","sortType":"DESC","quoteType":"MUTUALFUND","query":{"operator":"AND","operands":[{"operator":"GT","operands":["intradayprice",15]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"GT","operands":["initialinvestment",1000]},{"operator":"EQ","operands":["exchange","NAS"]}]}},"top_etfs_us":{"sortField":"percentchange","sortType":"DESC","quoteType":"ETF","query":{"operator":"AND","operands":[{"operator":"GT","operands":["intradayprice",10]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"EQ","operands":["region","us"]}]}},"top_performing_etfs":{"sortField":"annualreportnetexpenseratio","sortType":"ASC","quoteType":"ETF","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["region","us"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["performanceratingoverall",4]},{"operator":"EQ","operands":["performanceratingoverall",5]}]},{"operator":"GT","operands":["intradayprice",10]}]}},"technology_etfs":{"sortField":"annualreportnetexpenseratio","sortType":"ASC","quoteType":"ETF","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["region","us"]},{"operator":"EQ","operands":["categoryname","Technology"]}]}},"bond_etfs":{"sortField":"annualreportnetexpenseratio","sortType":"ASC","quoteType":"ETF","query":{"operator":"AND","operands":[{"operator":"EQ","operands":["region","us"]},{"operator":"OR","operands":[{"operator":"EQ","operands":["categoryname","Corporate Bond"]},{"operator":"EQ","operands":["categoryname","Emerging Markets Bond"]},{"operator":"EQ","operands":["categoryname","Emerging-Markets Local-Currency Bond"]},{"operator":"EQ","operands":["categoryname","High Yield Bond"]},{"operator":"EQ","operands":["categoryname","Intermediate-Term Bond"]},{"operator":"EQ","operands":["categoryname","Long-Term Bond"]},{"operator":"EQ","operands":["categoryname","Inflation-Protected Bond"]},{"operator":"EQ","operands":["categoryname","Multisector Bond"]},{"operator":"EQ","operands":["categoryname","Nontraditional Bond"]},{"operator":"EQ","operands":["categoryname","Short-Term Bond"]},{"operator":"EQ","operands":["categoryname","Ultrashort Bond"]},{"operator":"EQ","operands":["categoryname","World Bond"]}]}]}}}"#;

fn predefined_map() -> &'static HashMap<String, Value> {
    static M: OnceLock<HashMap<String, Value>> = OnceLock::new();
    M.get_or_init(|| {
        serde_json::from_str(PREDEFINED_SCREENER_QUERIES)
            .expect("embedded PREDEFINED_SCREENER_QUERIES is valid JSON")
    })
}

/// Logical/comparison operators for a screener [`Query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Eq,
    IsIn,
    Btwn,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
}

impl Operator {
    fn as_str(self) -> &'static str {
        match self {
            Operator::Eq => "EQ",
            Operator::IsIn => "IS-IN",
            Operator::Btwn => "BTWN",
            Operator::Gt => "GT",
            Operator::Lt => "LT",
            Operator::Gte => "GTE",
            Operator::Lte => "LTE",
            Operator::And => "AND",
            Operator::Or => "OR",
        }
    }
}

/// Which asset class a [`Query`] targets; maps to the screener `quoteType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Equity,
    Fund,
    Etf,
}

impl QueryKind {
    fn quote_type(self) -> &'static str {
        match self {
            QueryKind::Equity => "EQUITY",
            QueryKind::Fund => "MUTUALFUND",
            QueryKind::Etf => "ETF",
        }
    }
}

/// A scalar operand value (string or number).
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenerValue {
    Str(String),
    Num(f64),
}

impl From<&str> for ScreenerValue {
    fn from(s: &str) -> Self {
        ScreenerValue::Str(s.to_string())
    }
}
impl From<String> for ScreenerValue {
    fn from(s: String) -> Self {
        ScreenerValue::Str(s)
    }
}
impl From<f64> for ScreenerValue {
    fn from(n: f64) -> Self {
        ScreenerValue::Num(n)
    }
}
impl From<i64> for ScreenerValue {
    fn from(n: i64) -> Self {
        ScreenerValue::Num(n as f64)
    }
}

impl ScreenerValue {
    fn to_json(&self) -> Value {
        match self {
            ScreenerValue::Str(s) => json!(s),
            ScreenerValue::Num(n) => json!(n),
        }
    }
}

/// A single operand of a [`Query`]: a nested sub-query, a field name, or a
/// literal value.
#[derive(Debug, Clone)]
pub enum Operand {
    Query(Box<Query>),
    Field(String),
    Value(ScreenerValue),
}

impl Operand {
    /// A nested sub-query operand (used by `AND`/`OR`).
    pub fn query(q: impl Into<Query>) -> Operand {
        Operand::Query(Box::new(q.into()))
    }
    /// A field-name operand (the first operand of value comparisons).
    pub fn field(s: impl Into<String>) -> Operand {
        Operand::Field(s.into())
    }
    /// A literal value operand.
    pub fn value(v: impl Into<ScreenerValue>) -> Operand {
        Operand::Value(v.into())
    }

    fn to_json(&self) -> Value {
        match self {
            Operand::Query(q) => q.to_value(),
            Operand::Field(s) => json!(s),
            Operand::Value(v) => v.to_json(),
        }
    }
}

/// A screener query tree. Built via [`Query::equity`]/[`Query::fund`]/[`Query::etf`]
/// (or the [`EquityQuery`]/[`FundQuery`]/[`ETFQuery`] newtypes) and passed to
/// [`YfSession::screen`].
#[derive(Debug, Clone)]
pub struct Query {
    pub kind: QueryKind,
    pub operator: Operator,
    pub operands: Vec<Operand>,
}

impl Query {
    /// Build an equity (stock) query.
    pub fn equity(operator: Operator, operands: Vec<Operand>) -> Query {
        Query {
            kind: QueryKind::Equity,
            operator,
            operands,
        }
    }
    /// Build a mutual-fund query.
    pub fn fund(operator: Operator, operands: Vec<Operand>) -> Query {
        Query {
            kind: QueryKind::Fund,
            operator,
            operands,
        }
    }
    /// Build an ETF query.
    pub fn etf(operator: Operator, operands: Vec<Operand>) -> Query {
        Query {
            kind: QueryKind::Etf,
            operator,
            operands,
        }
    }

    /// Serialize to the Yahoo `"operator"`/`"operands"` JSON shape.
    ///
    /// `IS-IN` is expanded into an `OR` of `EQ` queries, exactly as yfinance
    /// does in `QueryBase.to_dict`.
    pub fn to_value(&self) -> Value {
        if self.operator == Operator::IsIn {
            let field_json = match &self.operands.first() {
                Some(Operand::Field(f)) => json!(f.clone()),
                _ => Value::Null,
            };
            let eqs: Vec<Value> = self
                .operands
                .iter()
                .skip(1)
                .map(|o| {
                    let v = match o {
                        Operand::Value(v) => v.to_json(),
                        Operand::Field(f) => json!(f.clone()),
                        Operand::Query(_) => Value::Null,
                    };
                    json!({"operator": "EQ", "operands": [field_json, v]})
                })
                .collect();
            return json!({"operator": "OR", "operands": eqs});
        }
        let operands: Vec<Value> = self.operands.iter().map(|o| o.to_json()).collect();
        json!({"operator": self.operator.as_str(), "operands": operands})
    }
}

macro_rules! screener_newtype {
    ($name:ident, $kind:ident) => {
        /// Thin newtype for API parity with `yfinance.$name`.
        #[derive(Debug, Clone)]
        pub struct $name(pub Query);
        impl $name {
            /// Construct a query of this asset class.
            pub fn new(operator: Operator, operands: Vec<Operand>) -> Self {
                $name(Query::$kind(operator, operands))
            }
        }
        impl From<$name> for Query {
            fn from(q: $name) -> Query {
                q.0
            }
        }
    };
}

screener_newtype!(EquityQuery, equity);
screener_newtype!(FundQuery, fund);
screener_newtype!(ETFQuery, etf);

/// What to screen: either a predefined query by name, or a custom [`Query`].
#[derive(Debug, Clone)]
pub enum ScreenerQuery {
    Predefined(String),
    Custom(Query),
}

impl From<&str> for ScreenerQuery {
    fn from(s: &str) -> Self {
        ScreenerQuery::Predefined(s.to_string())
    }
}
impl From<String> for ScreenerQuery {
    fn from(s: String) -> Self {
        ScreenerQuery::Predefined(s)
    }
}
impl From<Query> for ScreenerQuery {
    fn from(q: Query) -> Self {
        ScreenerQuery::Custom(q)
    }
}
impl From<EquityQuery> for ScreenerQuery {
    fn from(q: EquityQuery) -> Self {
        ScreenerQuery::Custom(q.into())
    }
}
impl From<FundQuery> for ScreenerQuery {
    fn from(q: FundQuery) -> Self {
        ScreenerQuery::Custom(q.into())
    }
}
impl From<ETFQuery> for ScreenerQuery {
    fn from(q: ETFQuery) -> Self {
        ScreenerQuery::Custom(q.into())
    }
}

/// Options for [`YfSession::screen`]. `None` fields fall back to the yfinance
/// defaults (offset 0, count 25, sortField "ticker", sort ascending false).
#[derive(Debug, Clone, Default)]
pub struct ScreenOptions {
    pub offset: Option<u32>,
    pub size: Option<u32>,
    pub count: Option<u32>,
    pub sort_field: Option<String>,
    pub sort_asc: Option<bool>,
    pub user_id: Option<String>,
    pub user_id_type: Option<String>,
}

impl ScreenOptions {
    /// yfinance defaults for a custom screen.
    pub fn custom_defaults() -> Self {
        ScreenOptions {
            offset: Some(0),
            size: None,
            count: Some(25),
            sort_field: Some("ticker".to_string()),
            sort_asc: Some(false),
            user_id: Some(String::new()),
            user_id_type: Some("guid".to_string()),
        }
    }
}

/// One row returned by a screen.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenerQuote {
    pub symbol: Option<String>,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub exchange: Option<String>,
    #[serde(rename = "quoteType")]
    pub quote_type: Option<String>,
    pub price: Option<f64>,
    pub market_cap: Option<f64>,
    pub percent_change: Option<f64>,
    pub volume: Option<u64>,
    /// Full raw quote object, for fields not surfaced above.
    pub raw: Value,
}

/// A screen result (`finance.result[0]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenerResult {
    pub total: Option<u64>,
    pub quotes: Vec<ScreenerQuote>,
    /// Full raw result object.
    pub raw: Value,
}

fn dig_f64(v: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    // Yahoo returns either a plain number or a `{ "raw": <num>, "fmt": ... }`.
    if let Some(n) = cur.as_f64() {
        return Some(n);
    }
    if let Some(raw) = cur.get("raw") {
        return raw.as_f64();
    }
    None
}

fn parse_quote(q: &Value) -> ScreenerQuote {
    ScreenerQuote {
        symbol: q.get("symbol").and_then(|x| x.as_str()).map(String::from),
        short_name: q
            .get("shortname")
            .or_else(|| q.get("shortName"))
            .and_then(|x| x.as_str())
            .map(String::from),
        long_name: q
            .get("longname")
            .or_else(|| q.get("longName"))
            .and_then(|x| x.as_str())
            .map(String::from),
        exchange: q.get("exchange").and_then(|x| x.as_str()).map(String::from),
        quote_type: q
            .get("quoteType")
            .and_then(|x| x.as_str())
            .map(String::from),
        price: dig_f64(q, &["regularMarketPrice"]),
        market_cap: dig_f64(q, &["marketCap"]),
        percent_change: dig_f64(q, &["regularMarketChangePercent"]),
        volume: q
            .get("regularMarketVolume")
            .and_then(|x| x.get("raw"))
            .and_then(|x| x.as_u64())
            .or_else(|| q.get("regularMarketVolume").and_then(|x| x.as_u64())),
        raw: q.clone(),
    }
}

fn parse_result(v: &Value) -> Result<ScreenerResult> {
    let r = v
        .get("finance")
        .and_then(|f| f.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| YfError::DataMissing("screener result".to_string()))?;
    let total = r.get("total").and_then(|t| t.as_u64());
    let quotes = r
        .get("quotes")
        .and_then(|q| q.as_array())
        .map(|a| a.iter().map(parse_quote).collect())
        .unwrap_or_default();
    Ok(ScreenerResult {
        total,
        quotes,
        raw: r.clone(),
    })
}

impl YfSession {
    /// Run a screen: a predefined query name, or a custom [`Query`] (or one of
    /// the newtype queries). Mirrors `yfinance.screen`.
    pub async fn screen(
        &self,
        query: impl Into<ScreenerQuery>,
        opts: &ScreenOptions,
    ) -> Result<ScreenerResult> {
        match query.into() {
            ScreenerQuery::Predefined(name) => self.screen_predefined(&name, opts).await,
            ScreenerQuery::Custom(q) => self.screen_custom(q, opts).await,
        }
    }

    async fn screen_predefined(&self, name: &str, opts: &ScreenOptions) -> Result<ScreenerResult> {
        let pq = predefined_map().get(name).ok_or_else(|| {
            YfError::NotSupported(format!("unknown predefined screener '{name}'"))
        })?;

        // yfinance switches to the custom POST endpoint when `offset` is given
        // (the predefined endpoint is believed to ignore offset).
        if opts.offset.is_some() {
            let q = pq.get("query").cloned().unwrap_or(Value::Null);
            let quote_type = pq
                .get("quoteType")
                .and_then(|v| v.as_str())
                .unwrap_or("EQUITY");
            return self.screen_custom_body(&q, quote_type, opts).await;
        }

        let sort_field = opts
            .sort_field
            .clone()
            .or_else(|| {
                pq.get("sortField")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "ticker".to_string());
        let sort_asc = opts.sort_asc.unwrap_or_else(|| {
            pq.get("sortType")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("asc"))
                .unwrap_or(false)
        });
        let offset = opts.offset.unwrap_or(0);
        let count = opts
            .count
            .or_else(|| pq.get("count").and_then(|v| v.as_u64()).map(|n| n as u32))
            .unwrap_or(25);

        let cfg = self.config();
        let params: Vec<(&str, String)> = vec![
            ("corsDomain", "finance.yahoo.com".to_string()),
            ("formatted", "false".to_string()),
            ("lang", cfg.locale.lang.clone()),
            ("region", cfg.locale.region.clone()),
            ("scrIds", name.to_string()),
            ("offset", offset.to_string()),
            ("count", count.to_string()),
            ("sortField", sort_field),
            (
                "sortAsc",
                if sort_asc {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
            ),
            ("userId", opts.user_id.clone().unwrap_or_default()),
            (
                "userIdType",
                opts.user_id_type
                    .clone()
                    .unwrap_or_else(|| "guid".to_string()),
            ),
        ];
        let v = self.get_json(PREDEFINED_URL, &params).await?;
        parse_result(&v)
    }

    async fn screen_custom(&self, q: Query, opts: &ScreenOptions) -> Result<ScreenerResult> {
        let body_q = q.to_value();
        let quote_type = q.kind.quote_type();
        self.screen_custom_body(&body_q, quote_type, opts).await
    }

    async fn screen_custom_body(
        &self,
        query_json: &Value,
        quote_type: &str,
        opts: &ScreenOptions,
    ) -> Result<ScreenerResult> {
        // yfinance defaults applied only when not explicitly provided.
        let offset = opts.offset.unwrap_or(0);
        let count = opts.count.unwrap_or(25);
        let sort_field = opts
            .sort_field
            .clone()
            .unwrap_or_else(|| "ticker".to_string());
        let sort_asc = opts.sort_asc.unwrap_or(false);

        let mut body = json!({
            "offset": offset,
            "count": count,
            "sortField": sort_field,
            "sortType": if sort_asc { "ASC" } else { "DESC" },
            "userId": opts.user_id.clone().unwrap_or_default(),
            "userIdType": opts.user_id_type.clone().unwrap_or_else(|| "guid".to_string()),
            "query": query_json,
            "quoteType": quote_type,
        });
        if let Some(s) = opts.size {
            body["size"] = json!(s);
        }

        let cfg = self.config();
        let params: Vec<(&str, String)> = vec![
            ("corsDomain", "finance.yahoo.com".to_string()),
            ("formatted", "false".to_string()),
            ("lang", cfg.locale.lang.clone()),
            ("region", cfg.locale.region.clone()),
        ];
        let v = self.post_json(SCREENER_URL, &params, &body).await?;
        parse_result(&v)
    }
}
