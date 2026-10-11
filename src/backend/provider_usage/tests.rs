#[cfg(test)]
mod mimo_auth_tests {
    use chrono::Datelike;

    use super::{
        grok_billing_snapshot, mimo_auth_error, mimo_headers, mimo_usage_percent,
        provider_reset_duration, provider_reset_text_local, should_skip_codex_cached_account,
        grok_web_billing_snapshot,
    };

    #[test]
    fn recognizes_expired_mimo_session_responses() {
        assert!(mimo_auth_error("HTTP 401 <html>Unauthorized</html>"));
        assert!(mimo_auth_error("HTTP 403 forbidden"));
        assert!(!mimo_auth_error("HTTP 500 upstream failure"));
    }

    #[test]
    fn reset_durations_switch_to_days_after_24_hours() {
        assert_eq!(provider_reset_duration(23.0 * 3600.0), "23小时");
        assert_eq!(provider_reset_duration(24.0 * 3600.0), "24小时");
        assert_eq!(provider_reset_duration(142.0 * 3600.0), "6天");
    }

    #[test]
    fn reset_text_local_accepts_millisecond_timestamps() {
        let reset_at_millis = (chrono::Utc::now().timestamp() + 2 * 24 * 3600) * 1000;
        assert_eq!(provider_reset_text_local(reset_at_millis), "2天");
    }

    #[test]
    fn forced_codex_refresh_does_not_reuse_quota_wall_cache() {
        let now = chrono::Utc::now().timestamp();
        let section = super::ProviderUsageSection {
            provider: "codex".into(),
            windows: vec![super::ProviderUsageWindow {
                window: "account 周额度".into(),
                used: Some("100%".into()),
                reset: Some("60分钟".into()),
                reset_at: Some(now + 3600),
            }],
            ..Default::default()
        };
        assert!(should_skip_codex_cached_account(&section, "account", now, false));
        assert!(!should_skip_codex_cached_account(&section, "account", now, true));
    }

    #[test]
    fn grok_web_frame_ratio_uses_percentage_points() {
        let body = b"\x00\x00\x00\x00\x15\x0a\x13\x0d\x00\x00\x80\x3f\x2a\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01";
        let payload = grok_web_billing_snapshot(body).unwrap();
        let (_, used, _, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert!((used - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn grok_web_frame_with_omitted_ratio_preserves_reset() {
        let body = b"\x00\x00\x00\x00\x48\x0a\x46\x12\x00\x1a\x00\x22\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01\x2a\x0c\x08\xff\xbd\xec\xd4\x06\x10\x90\x99\x88\x9c\x01\x42\x1e\x08\x02\x12\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01\x1a\x0c\x08\xff\xbd\xec\xd4\x06\x10\x90\x99\x88\x9c\x01\x58\x01\x62\x00\x68\x01\x80\x00\x00";
        let payload = grok_web_billing_snapshot(body).unwrap();
        let (window, used, reset, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert_eq!(window, "周额度");
        assert_eq!(used, 0.0);
        let expected_reset = payload
            .pointer("/config/currentPeriod/end")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|date| {
                let local = date.with_timezone(&chrono::Local);
                format!("{}/{} {}", local.month(), local.day(), local.format("%H:%M"))
            })
            .unwrap();
        assert_eq!(reset, expected_reset);
    }

    #[test]
    fn grok_missing_usage_percent_is_not_reported_as_zero() {
        let payload = serde_json::json!({
            "config": {
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "end": "2026-09-04T19:41:51.327290+00:00"
                },
                "onDemandCap": {"val": 0},
                "onDemandUsed": {"val": 0}
            }
        });
        let error = grok_billing_snapshot(&payload).unwrap_err();
        assert!(error.contains("未提供周额度使用比例"));
    }

    #[test]
    fn grok_product_usage_percent_can_fill_missing_combined_percent() {
        let payload = serde_json::json!({
            "config": {
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "end": "2026-09-04T19:41:51.327290+00:00"
                },
                "productUsage": [
                    {"product": "GrokBuild", "usagePercent": 37.5},
                    {"product": "GrokChat", "usagePercent": 12.0}
                ]
            }
        });
        let (_, used_percent, _, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert!((used_percent - 37.5).abs() < f64::EPSILON);
    }

    #[test]
    fn mimo_monthly_percent_uses_plan_token_usage_and_limit() {
        let percent = mimo_usage_percent(Some(8469.0), Some(10000.0)).unwrap();
        assert!((percent - 84.69).abs() < 0.000_001);
    }

    #[test]
    fn mimo_monthly_percent_preserves_a_real_overage_from_the_plan_fields() {
        let percent = mimo_usage_percent(Some(10_948.0), Some(10_000.0)).unwrap();
        assert!((percent - 109.48).abs() < 0.000_001);
    }
    #[test]
    fn builds_mimo_headers_without_extra_credentials() {
        let headers = mimo_headers(
            "api-platform_ph=redacted",
            "https://platform.xiaomimimo.com/",
        );
        assert!(headers
            .iter()
            .any(|(name, value)| name == "Cookie" && value == "api-platform_ph=redacted"));
        assert!(headers.iter().all(|(name, _)| name != "Authorization"));
    }
}

#[cfg(test)]
mod zed_pro_tests {
    use super::{json_string_or_number, zed_account_snapshot, zed_usage_limit};
    use serde_json::{json, Value};

    #[test]
    fn student_plan_does_not_invent_a_monthly_percent() {
        let payload = json!({
            "plan": {
                "plan": "zed_free",
                "plan_v2": "zed_free",
                "plan_v3": "zed_student",
                "subscription_period": {
                    "started_at": "2026-08-30T00:00:00Z",
                    "ended_at": "2026-09-30T00:00:00Z"
                },
                "usage": {
                    "model_requests": { "used": 0, "limit": { "limited": 0 } },
                    "edit_predictions": { "used": 0, "limit": "unlimited" }
                }
            }
        });
        let (description, windows) = zed_account_snapshot(&payload).unwrap();
        assert!(description.contains("Student"));
        assert!(description.contains("账期 08/30–09/30"));
        assert!(description.contains("模型请求 0"));
        assert!(description.contains("补全 0/无限"));
        assert!(windows.is_empty());
    }

    #[test]
    fn pro_plan_reports_monthly_percent_from_model_requests() {
        let payload = json!({
            "plan": {
                "plan_v3": "zed_pro",
                "subscription_period": {
                    "started_at": "2026-08-01T00:00:00Z",
                    "ended_at": "2026-09-01T00:00:00Z"
                },
                "usage": {
                    "model_requests": { "used": 50, "limit": { "limited": 500 } },
                    "edit_predictions": { "used": 12, "limit": { "limited": 100 } }
                }
            }
        });
        let (description, windows) = zed_account_snapshot(&payload).unwrap();
        assert!(description.contains("Pro"));
        assert!(description.contains("模型请求 50/500"));
        assert!(description.contains("补全 12/100"));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window, "月额度");
        assert_eq!(windows[0].used.as_deref(), Some("10.0%"));
        assert!(windows[0].reset_at.is_some());
    }

    #[test]
    fn missing_plan_is_an_error() {
        let err = zed_account_snapshot(&json!({})).unwrap_err();
        assert!(err.contains("套餐"));
    }

    #[test]
    fn usage_limit_treats_unlimited_and_zero_as_non_percent() {
        assert_eq!(zed_usage_limit(Some(&json!("unlimited"))), None);
        assert_eq!(zed_usage_limit(Some(&json!({"unlimited": {}}))), None);
        assert_eq!(zed_usage_limit(Some(&json!({"limited": 0}))), Some(0.0));
        assert_eq!(zed_usage_limit(Some(&json!({"limited": 80}))), Some(80.0));
    }

    #[test]
    fn user_id_accepts_number_or_string() {
        assert_eq!(json_string_or_number(Some(&json!(42))), Some("42".into()));
        assert_eq!(json_string_or_number(Some(&json!("42"))), Some("42".into()));
        assert_eq!(json_string_or_number(Some(&Value::Null)), None);
    }
}
