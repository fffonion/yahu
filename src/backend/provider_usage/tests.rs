#[cfg(test)]
mod mimo_auth_tests {
    use super::{
        grok_billing_snapshot, mimo_auth_error, mimo_headers, mimo_usage_percent,
        provider_reset_duration, provider_reset_text_local, should_skip_codex_cached_account,
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
        assert!(should_skip_codex_cached_account(
            &section, "account", now, false
        ));
        assert!(!should_skip_codex_cached_account(
            &section, "account", now, true
        ));
    }

    #[test]
    fn grok_rest_request_uses_official_endpoint_and_headers() {
        let client = reqwest::Client::new();
        let request = super::grok_cli_request(
            &client,
            "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
            "[REDACTED]",
            Some("[REDACTED]"),
        )
        .build()
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(request.url().path(), "/v1/billing");
        assert_eq!(request.url().query(), Some("format=credits"));
        assert!(
            request
                .headers()
                .get("authorization")
                .is_some_and(|v| v == "Bearer [REDACTED]")
        );
        assert!(
            request
                .headers()
                .get("x-userid")
                .is_some_and(|v| v == "[REDACTED]")
        );
        assert_eq!(request.headers()["x-xai-token-auth"], "xai-grok-cli");
        assert_eq!(request.headers()["accept"], "application/json");
        assert_eq!(request.headers()["x-grok-client-mode"], "headless");
        assert_eq!(
            request.headers()["x-grok-client-version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(request.body().is_none());
        assert!(!request.headers().contains_key("x-grpc-web"));
    }

    async fn grok_test_server(
        replies: Vec<(u16, serde_json::Value)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, payload) in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut buffer = [0; 1024];
                    let size = socket.read(&mut buffer).await.unwrap();
                    assert!(size > 0 && bytes.len() + size <= 8192);
                    bytes.extend_from_slice(&buffer[..size]);
                }
                let header_end = bytes
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                    .unwrap()
                    + 4;
                let content_length = String::from_utf8_lossy(&bytes[..header_end])
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + content_length {
                    let mut buffer = [0; 1024];
                    let size = socket.read(&mut buffer).await.unwrap();
                    assert!(size > 0 && bytes.len() + size <= 8192);
                    bytes.extend_from_slice(&buffer[..size]);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let body = payload.to_string();
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        (format!("http://{address}/v1"), task)
    }

    fn grok_test_account(user_id: Option<&str>) -> super::AuthCredentialEntry {
        super::AuthCredentialEntry {
            label: "account".into(),
            access_token: "[REDACTED]".into(),
            refresh_token: None,
            user_id: user_id.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn grok_rest_resolves_missing_identity_and_reads_json_credits() {
        let credits = serde_json::json!({"config":{"creditUsagePercent":42.5,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","end":"2028-06-08T00:00:00Z"}}});
        let (base, server) = grok_test_server(vec![
            (200, serde_json::json!({"userId":"[REDACTED]"})),
            (200, credits),
        ])
        .await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let payload = super::grok_query_billing_from_base(&client, &base, &grok_test_account(None))
            .await
            .unwrap();
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /v1/user HTTP/1.1"));
        assert!(requests[1].starts_with("GET /v1/billing?format=credits HTTP/1.1"));
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("x-userid: [redacted]\r\n")
        );
        let (window, used, _, _, reset) = grok_billing_snapshot(&payload).unwrap();
        assert_eq!(window, "周额度");
        assert_eq!(used, 42.5);
        assert!(reset.is_some());
    }

    #[tokio::test]
    async fn grok_rest_retries_once_after_unauthorized() {
        let credits = serde_json::json!({"config":{"creditUsagePercent":12.5,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","end":"2028-06-08T00:00:00Z"}}});
        let (base, server) = grok_test_server(vec![
            (401, serde_json::json!({"error":"[REDACTED]"})),
            (
                200,
                serde_json::json!({"access_token":"[REDACTED]","refresh_token":"[REDACTED]"}),
            ),
            (200, credits),
        ])
        .await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut account = grok_test_account(Some("[REDACTED]"));
        account.refresh_token = Some("[REDACTED]".into());
        let payload =
            super::grok_query_account_from_base(&client, &base, &format!("{base}/token"), &account)
                .await
                .unwrap();
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("GET /v1/billing?format=credits HTTP/1.1"));
        assert!(requests[1].starts_with("POST /v1/token HTTP/1.1"));
        assert!(requests[1].contains("grant_type=refresh_token"));
        assert!(requests[2].starts_with("GET /v1/billing?format=credits HTTP/1.1"));
        assert_eq!(grok_billing_snapshot(&payload).unwrap().1, 12.5);
    }

    #[tokio::test]
    async fn grok_rest_does_not_refresh_on_permission_limits_or_service_failure() {
        for status in [403, 429, 503] {
            let (base, server) =
                grok_test_server(vec![(status, serde_json::json!({"error":"[REDACTED]"}))]).await;
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let mut account = grok_test_account(Some("[REDACTED]"));
            account.refresh_token = Some("[REDACTED]".into());
            let error = super::grok_query_account_from_base(
                &client,
                &base,
                &format!("{base}/token"),
                &account,
            )
            .await
            .unwrap_err();
            assert!(error.starts_with(&format!("HTTP {status}：")));
            assert!(!error.contains("[REDACTED]"));
            let requests = server.await.unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("GET /v1/billing?format=credits HTTP/1.1"));
        }
    }

    #[tokio::test]
    async fn grok_rest_rejects_missing_user_id_without_guessing_identity() {
        let (base, server) = grok_test_server(vec![(200, serde_json::json!({}))]).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let error = super::grok_query_billing_from_base(&client, &base, &grok_test_account(None))
            .await
            .unwrap_err();
        assert_eq!(error, "Grok 用户信息缺少 userId");
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn grok_rest_stops_after_one_refresh_when_unauthorized_persists() {
        let (base, server) = grok_test_server(vec![
            (401, serde_json::json!({})),
            (200, serde_json::json!({"access_token":"[REDACTED]"})),
            (401, serde_json::json!({})),
        ])
        .await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut account = grok_test_account(Some("[REDACTED]"));
        account.refresh_token = Some("[REDACTED]".into());
        let error =
            super::grok_query_account_from_base(&client, &base, &format!("{base}/token"), &account)
                .await
                .unwrap_err();
        assert!(error.starts_with("HTTP 401："));
        assert_eq!(server.await.unwrap().len(), 3);
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
        assert!(
            headers
                .iter()
                .any(|(name, value)| name == "Cookie" && value == "api-platform_ph=redacted")
        );
        assert!(headers.iter().all(|(name, _)| name != "Authorization"));
    }
}

#[cfg(test)]
mod zed_pro_tests {
    use super::{json_string_or_number, zed_account_snapshot, zed_usage_limit};
    use serde_json::{Value, json};

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
