use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use reqwest::Client;
use serde_json::Value;

const NEWAPI_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const SHANGHAI_OFFSET_SECONDS: i32 = 8 * 60 * 60;

#[derive(Clone, Debug)]
pub enum NewApiAuth {
    Bearer { token: String, user_id: String },
    SessionCookie { cookie: String, user_id: String },
}

impl NewApiAuth {
    pub fn headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("Cache-Control".to_string(), "no-store".to_string()),
        ];
        match self {
            Self::Bearer { token, user_id } => {
                headers.push(("Authorization".to_string(), format!("Bearer {token}")));
                headers.push(("New-API-User".to_string(), user_id.clone()));
            }
            Self::SessionCookie { cookie, user_id } => {
                headers.push(("Cookie".to_string(), cookie.clone()));
                headers.push(("New-API-User".to_string(), user_id.clone()));
            }
        }
        headers
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewApiTimeRange {
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub default_time: String,
}

impl NewApiTimeRange {
    pub fn shanghai_days(now: DateTime<Utc>, days: i64) -> Self {
        let timezone =
            FixedOffset::east_opt(SHANGHAI_OFFSET_SECONDS).expect("valid Shanghai offset");
        let local_now = now.with_timezone(&timezone);
        let start_date = local_now.date_naive() - Duration::days(days.max(1));
        let start_local = timezone
            .from_local_datetime(&start_date.and_hms_opt(0, 0, 0).expect("valid midnight"))
            .single()
            .expect("fixed offset has one local time");
        Self {
            start_timestamp: start_local.timestamp(),
            end_timestamp: now.timestamp(),
            default_time: "day".to_string(),
        }
    }

    fn query_string(&self) -> String {
        format!(
            "start_timestamp={}&end_timestamp={}&default_time={}",
            self.start_timestamp, self.end_timestamp, self.default_time
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewApiStatus {
    pub quota_per_unit: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewApiUsageRecord {
    pub model_name: String,
    pub count: f64,
    pub quota: f64,
    pub token_used: f64,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewApiUsageAggregate {
    pub model_name: String,
    pub count: f64,
    pub quota: f64,
    pub token_used: f64,
}

pub struct NewApiClient<'a> {
    client: &'a Client,
    base_url: String,
    auth: NewApiAuth,
}

impl<'a> NewApiClient<'a> {
    pub fn new(client: &'a Client, base_url: impl Into<String>, auth: NewApiAuth) -> Self {
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth,
        }
    }

    async fn get_json(&self, path: &str, query: Option<&str>) -> Result<Value, String> {
        let url = match query {
            Some(query) if !query.is_empty() => format!("{}{path}?{query}", self.base_url),
            _ => format!("{}{path}", self.base_url),
        };
        let mut request = self.client.get(url).timeout(NEWAPI_TIMEOUT);
        for (name, value) in self.auth.headers() {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(|err| err.to_string())?;
        let status = response.status();
        let body = response.text().await.map_err(|err| err.to_string())?;
        if !status.is_success() {
            let snippet: String = body.chars().take(160).collect();
            return Err(format!("HTTP {status} {snippet}"));
        }
        serde_json::from_str(&body).map_err(|err| format!("响应不是 JSON：{err}"))
    }

    pub async fn fetch_status(&self) -> Result<NewApiStatus, String> {
        let payload = self.get_json("/api/status", None).await?;
        parse_status_payload(&payload)
    }

    pub async fn fetch_usage(
        &self,
        range: &NewApiTimeRange,
    ) -> Result<Vec<NewApiUsageRecord>, String> {
        let payload = self
            .get_json("/api/data/self", Some(&range.query_string()))
            .await?;
        parse_usage_payload(&payload)
    }
}

fn numeric(value: Option<&Value>) -> f64 {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        })
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn integer(value: Option<&Value>) -> i64 {
    numeric(value).round() as i64
}

fn response_data(payload: &Value) -> Result<&Value, String> {
    if payload.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(payload
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "NewAPI 响应失败".to_string()));
    }
    payload
        .get("data")
        .ok_or_else(|| "NewAPI 响应缺少 data".to_string())
}

pub fn parse_status_payload(payload: &Value) -> Result<NewApiStatus, String> {
    let data = response_data(payload)?;
    let quota_per_unit = numeric(data.get("quota_per_unit"));
    if quota_per_unit <= 0.0 {
        return Err("NewAPI status 缺少有效 quota_per_unit".to_string());
    }
    Ok(NewApiStatus { quota_per_unit })
}

pub fn parse_usage_payload(payload: &Value) -> Result<Vec<NewApiUsageRecord>, String> {
    let data = response_data(payload)?;
    let rows = data
        .as_array()
        .ok_or_else(|| "NewAPI 用量 data 不是数组".to_string())?;
    Ok(rows
        .iter()
        .map(|row| NewApiUsageRecord {
            model_name: row
                .get("model_name")
                .or_else(|| row.get("model"))
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("?")
                .to_string(),
            count: numeric(row.get("count")),
            quota: numeric(row.get("quota")),
            token_used: numeric(row.get("token_used")),
            created_at: integer(row.get("created_at")),
        })
        .collect())
}

pub fn aggregate_usage(records: &[NewApiUsageRecord]) -> Vec<NewApiUsageAggregate> {
    let mut grouped: BTreeMap<String, NewApiUsageAggregate> = BTreeMap::new();
    for record in records {
        let entry =
            grouped
                .entry(record.model_name.clone())
                .or_insert_with(|| NewApiUsageAggregate {
                    model_name: record.model_name.clone(),
                    ..Default::default()
                });
        entry.count += record.count;
        entry.quota += record.quota;
        entry.token_used += record.token_used;
    }
    grouped.into_values().collect()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::{
        NewApiAuth, NewApiTimeRange, aggregate_usage, parse_status_payload, parse_usage_payload,
    };

    #[test]
    fn session_cookie_auth_builds_newapi_headers_without_bearer() {
        let headers = NewApiAuth::SessionCookie {
            cookie: "session=[REDACTED]".into(),
            user_id: "641019".into(),
        }
        .headers();

        assert!(
            headers
                .iter()
                .any(|(name, value)| name == "Cookie" && value == "session=[REDACTED]")
        );
        assert!(
            headers
                .iter()
                .any(|(name, value)| name == "New-API-User" && value == "641019")
        );
        assert!(headers.iter().all(|(name, _)| name != "Authorization"));
    }

    #[test]
    fn status_parser_reads_quota_per_unit_from_newapi_status_payload() {
        let status = parse_status_payload(&json!({
            "success": true,
            "data": {"quota_per_unit": 500_000}
        }))
        .unwrap();
        assert_eq!(status.quota_per_unit, 500_000.0);
    }

    #[test]
    fn usage_parser_accepts_numeric_strings_and_aggregates_by_model() {
        let records = parse_usage_payload(&json!({
            "success": true,
            "data": [
                {"model_name": "gpt-a", "count": "2", "quota": 1500, "token_used": "100"},
                {"model_name": "gpt-a", "count": 3, "quota": "2500", "token_used": 200},
                {"model_name": "gpt-b", "count": 1, "quota": 900, "token_used": 50}
            ]
        }))
        .unwrap();
        let rows = aggregate_usage(&records);

        assert_eq!(rows[0].model_name, "gpt-a");
        assert_eq!(rows[0].count, 5.0);
        assert_eq!(rows[0].quota, 4000.0);
        assert_eq!(rows[0].token_used, 300.0);
        assert_eq!(rows[1].model_name, "gpt-b");
    }

    #[test]
    fn shanghai_range_starts_at_local_midnight() {
        let now = Utc.with_ymd_and_hms(2026, 9, 12, 4, 30, 0).unwrap();
        let range = NewApiTimeRange::shanghai_days(now, 7);
        assert_eq!(range.default_time, "day");
        assert_eq!(range.start_timestamp, 1_788_537_600);
        assert_eq!(range.end_timestamp, 1_789_187_400);
    }
}
