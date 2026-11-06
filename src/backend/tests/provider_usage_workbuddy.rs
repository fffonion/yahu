#[cfg(test)]
mod workbuddy_tests {
    use super::*;

    fn workbuddy_meta(root: &Path) -> ProviderUsageProvider {
        provider_usage_catalog(root)
            .into_iter()
            .find(|item| item.provider == "workbuddy")
            .expect("WorkBuddy usage card")
    }

    fn write_pool(root: &Path, entries: &str) {
        std::fs::write(
            root.join("auth.json"),
            format!("{{\"version\": 1, \"credential_pool\": {{\"workbuddy\": [{entries}]}}}}"),
        )
        .unwrap();
    }

    #[test]
    fn workbuddy_catalog_requires_the_oauth_pool() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("auth.json"), "{\"credential_pool\": {}}").unwrap();
        let meta = workbuddy_meta(temp.path());
        assert!(!meta.configured);
        assert!(!meta.query_ready);
        assert_eq!(meta.title, "WorkBuddy 用量");

        write_pool(
            temp.path(),
            r#"{"access_token": "[REDACTED]", "uid": "u1", "nickname": "131****7993", "domain": "www.codebuddy.cn", "expires_at_ms": 4102444800000}"#,
        );
        let meta = workbuddy_meta(temp.path());
        assert!(meta.configured && meta.query_ready);
        // The favicon service only returns a white-matted 40x40 JPEG, so the
        // card ships its own crisp SVG straight from the binary.
        assert!(provider_icon_url("workbuddy").is_none());
        let (_, content_type) = provider_icon_embedded_asset("workbuddy").expect("embedded icon");
        assert_eq!(content_type, "image/svg+xml");
    }

    fn meter_payload() -> Value {
        serde_json::json!({
            "code": 0, "msg": "OK", "requestId": "req-1",
            "data": { "Response": { "Data": { "TotalCount": 2, "TotalDosage": 2000, "Accounts": [
                {
                    "CapacityType": 1, "CapacityUnit": "credits",
                    "PackageName": "CodeBuddy个人版拉新权益包",
                    "CycleStartTime": "2026-11-07 04:39:12", "CycleEndTime": "2026-12-07 04:39:11",
                    "CapacityRemain": 1500, "CapacityUsed": 0, "CapacitySize": 1500,
                    "CapacitySizePrecise": 1500, "CapacityRemainPrecise": 1500, "CapacityUsedPrecise": 0,
                    "ActualCycleCapacityUsed": 0
                },
                {
                    "CapacityType": 4, "CapacityUnit": "credits",
                    "PackageName": "CodeBuddy个人体验版",
                    "CycleStartTime": "2026-11-01 00:00:00", "CycleEndTime": "2026-11-30 23:59:59",
                    "CapacityRemain": 500, "CapacityUsed": 0, "CapacitySize": 500,
                    "CapacitySizePrecise": 500, "CapacityRemainPrecise": 499.17, "CapacityUsedPrecise": 0.83,
                    "ActualCycleCapacityUsed": 0.83
                }
            ] } } }
        })
    }

    async fn serve(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn workbuddy_reads_credit_packs_from_the_billing_meter() {
        let billing = axum::Router::new().route(
            "/v2/billing/meter/get-user-resource",
            axum::routing::post(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["authorization"], "Bearer [REDACTED]");
                assert_eq!(headers["x-user-id"], "u1");
                assert_eq!(headers["x-domain"], "www.codebuddy.cn");
                assert_eq!(headers["x-no-enterprise-id"], "1");
                axum::Json(meter_payload())
            }),
        );
        let base = serve(billing).await;
        let temp = tempfile::tempdir().unwrap();
        write_pool(
            temp.path(),
            r#"{"access_token": "[REDACTED]", "uid": "u1", "nickname": "131****7993", "domain": "www.codebuddy.cn", "expires_at_ms": 4102444800000}"#,
        );
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_workbuddy_usage_from_base(&state, &base).await;
        assert!(section.errors.is_empty(), "{:?}", section.errors);
        // Nearest cycle end first: the trial pack expires before the welcome pack.
        assert_eq!(section.windows.len(), 2);
        assert_eq!(section.windows[0].window, "个人体验版");
        assert_eq!(section.windows[0].used.as_deref(), Some("0.83 / 500"));
        assert_eq!(section.windows[0].reset_at, Some(1796054399));
        assert_eq!(section.windows[1].window, "个人版拉新权益包");
        assert_eq!(section.windows[1].used.as_deref(), Some("0 / 1500"));
        assert!(section.description.is_empty());
        assert!(!serde_json::to_string(&section).unwrap().contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn workbuddy_scopes_multiple_accounts_and_keeps_errors_safe() {
        let billing = axum::Router::new().route(
            "/v2/billing/meter/get-user-resource",
            axum::routing::post(|headers: axum::http::HeaderMap| async move {
                if headers["authorization"] == "Bearer [REDACTED-2]" {
                    return (axum::http::StatusCode::UNAUTHORIZED, "{\"msg\":\"[REDACTED]\"}").into_response();
                }
                axum::Json(meter_payload()).into_response()
            }),
        );
        let base = serve(billing).await;
        let temp = tempfile::tempdir().unwrap();
        write_pool(
            temp.path(),
            concat!(
                r#"{"access_token": "[REDACTED]", "uid": "u1", "nickname": "alpha", "expires_at_ms": 4102444800000},"#,
                r#"{"access_token": "[REDACTED-2]", "uid": "u2", "nickname": "beta", "expires_at_ms": 4102444800000},"#,
                r#"{"access_token": "[REDACTED-3]", "uid": "u3", "nickname": "gamma", "expires_at_ms": 1000}"#
            ),
        );
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_workbuddy_usage_from_base(&state, &base).await;
        assert!(
            section.windows.iter().all(|window| window.window.starts_with("alpha · ")),
            "{:?}",
            section.windows.iter().map(|window| window.window.clone()).collect::<Vec<_>>()
        );
        assert_eq!(section.windows.len(), 2);
        assert!(section.errors.iter().any(|err| err == "beta：授权已失效，请在 Hermes 侧重新登录 workbuddy"));
        assert!(section.errors.iter().any(|err| err == "gamma：凭据已过期，请在 Hermes 侧重新登录 workbuddy"));
        assert!(section.errors.iter().all(|err| !err.contains("[REDACTED]")));
    }

    #[test]
    fn workbuddy_rejects_payloads_without_credit_packs() {
        assert_eq!(
            workbuddy_windows(&serde_json::json!({"code": 0}), None).unwrap_err(),
            "返回内容缺少额度包"
        );
        let empty = serde_json::json!({"data": {"Response": {"Data": {"Accounts": []}}}});
        assert_eq!(workbuddy_windows(&empty, None).unwrap_err(), "当前账号没有可用额度包");
        let zero = serde_json::json!({"data": {"Response": {"Data": {"Accounts": [
            {"PackageName": "CodeBuddy个人体验版", "CapacitySizePrecise": 0, "CapacityUsedPrecise": 0}
        ]}}}});
        assert_eq!(workbuddy_windows(&zero, None).unwrap_err(), "当前账号没有可用额度包");
    }
}
