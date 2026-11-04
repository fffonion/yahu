#[cfg(test)]
mod opendesign_tests {
    use super::*;

    fn opendesign_meta(root: &Path) -> ProviderUsageProvider {
        provider_usage_catalog(root)
            .into_iter()
            .find(|item| item.provider == "opendesign")
            .expect("OpenDesign usage card")
    }

    #[test]
    fn opendesign_catalog_needs_session_not_inference_key() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("config.yaml"),
            "custom_providers:\n  - name: amr-link-open-design-ai\n    api_key: '[REDACTED]'\n",
        )
        .unwrap();
        let meta = opendesign_meta(temp.path());
        assert!(meta.configured);
        assert!(!meta.query_ready);
        assert_eq!(meta.title, "OpenDesign 用量");
        std::fs::write(
            temp.path().join(".env"),
            "OPENDESIGN_COOKIE='session=[REDACTED]'\n",
        )
        .unwrap();
        let meta = opendesign_meta(temp.path());
        assert!(meta.configured && meta.query_ready);
        assert_eq!(provider_icon_url("opendesign"), Some("https://www.google.com/s2/favicons?domain=open-design.ai&sz=64".into()));
    }

    fn coding_plan() -> Value {
        serde_json::json!({
            "eligible": true,
            "tier": "go",
            "windows": [
                {"policyId": "w7", "durationSeconds": 604800, "resetMode": "anchored_recurring", "usedCredits": "35", "limitCredits": "100", "remainingCredits": "65", "windowStart": "2026-10-26T00:00:00Z", "resetsAt": "2026-11-05T00:00:00Z"},
                {"policyId": "w5", "durationSeconds": 18000, "resetMode": "activity_triggered", "usedCredits": "4", "limitCredits": "20", "remainingCredits": "16", "windowStart": null, "resetsAt": "2026-11-04T03:00:00Z"}
            ]
        })
    }

    async fn serve(routes: Vec<(&'static str, axum::Router)>) -> String {
        let mut app = axum::Router::new();
        for (path, router) in routes {
            app = app.nest(path, router);
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn opendesign_cookie_mode_reads_coding_plan_windows_and_wallet() {
        let current = axum::Router::new().route(
            "/",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                assert!(headers.get("cookie").is_some_and(|value| value == "session=[REDACTED]"), "cookie header missing");
                assert!(!headers.contains_key("authorization"));
                axum::Json(serde_json::json!({"workspaceId": "ws1", "activeWorkspaceId": "ws1", "workspaceMemberId": "m1"}))
            }),
        );
        let plan = axum::Router::new().route(
            "/",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["x-vela-workspace-id"], "ws1");
                assert!(headers.get("cookie").is_some());
                axum::Json(coding_plan())
            }),
        );
        let wallet = axum::Router::new().route(
            "/",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["x-vela-workspace-id"], "ws1");
                axum::Json(serde_json::json!({"workspaceId": "ws1", "workspaceMemberId": "m1", "balanceUsd": "0.00", "billingScopeVersion": 2}))
            }),
        );
        let base = serve(vec![
            ("/api/v1/workspaces/current", current),
            ("/api/v1/workspaces/ws1/billing/coding-plan-usage", plan),
            ("/api/v1/workspaces/ws1/wallet/balance", wallet),
        ])
        .await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "OPENDESIGN_COOKIE='Cookie: session=[REDACTED]'\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_opendesign_usage_from_base(&state, &base).await;
        assert!(section.errors.is_empty(), "{:?}", section.errors);
        assert_eq!(section.windows.len(), 2);
        assert_eq!(section.windows[0].window, "5h额度");
        assert_eq!(section.windows[0].used.as_deref(), Some("20.0%"));
        assert_eq!(section.windows[1].window, "周额度");
        assert_eq!(section.windows[1].used.as_deref(), Some("35.0%"));
        assert_eq!(section.windows[1].reset_at, Some(1793836800));
        assert_eq!(section.description, "余额 **$0.00**");
        assert!(!serde_json::to_string(&section).unwrap().contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn opendesign_control_key_uses_preflight_and_isolates_wallet_failure() {
        let preflight = axum::Router::new().route(
            "/",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["authorization"], "Bearer [REDACTED]");
                assert_eq!(headers["x-vela-workspace-id"], "ws9");
                assert!(!headers.contains_key("cookie"), "control key must not send cookies");
                axum::Json(serde_json::json!({
                    "schemaVersion": 1, "workspaceId": "ws9", "workspaceMemberId": "m9",
                    "modelId": null, "generatedAt": "2026-11-04T00:00:00Z", "balanceUsd": "12.50",
                    "modelCovered": null, "funding": "coding_plan",
                    "codingPlan": coding_plan()
                }))
            }),
        );
        let wallet = axum::Router::new().route(
            "/",
            axum::routing::get(|| async { (axum::http::StatusCode::SERVICE_UNAVAILABLE, "upstream down") }),
        );
        let base = serve(vec![
            ("/api/v1/workspaces/ws9/billing/preflight", preflight),
            ("/api/v1/workspaces/ws9/wallet/balance", wallet),
        ])
        .await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "OPENDESIGN_CONTROL_KEY='[REDACTED]'\nOPENDESIGN_WORKSPACE_ID='ws9'\n",
        )
        .unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_opendesign_usage_from_base(&state, &base).await;
        assert!(section.windows.len() == 2, "quota must survive a wallet failure");
        assert!(section.description.is_empty());
        assert!(section.errors.iter().any(|err| err.starts_with("钱包查询失败")));
        assert!(section.errors.iter().all(|err| err == "钱包查询失败：HTTP 503"));
    }

    #[tokio::test]
    async fn opendesign_reports_safe_errors_without_fabricating_quota() {
        let unauthorized = axum::Router::new().route(
            "/",
            axum::routing::get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "{\"error\":\"[REDACTED]\"}") }),
        );
        let base = serve(vec![
            ("/api/v1/workspaces/ws1/billing/coding-plan-usage", unauthorized),
            ("/api/v1/workspaces/ws1/wallet/balance", axum::Router::new().route(
                "/",
                axum::routing::get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "{\"error\":\"[REDACTED]\"}") }),
            )),
        ])
        .await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "OPENDESIGN_COOKIE='session=[REDACTED]'\nOPENDESIGN_WORKSPACE_ID='ws1'\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_opendesign_usage_from_base(&state, &base).await;
        assert!(section.windows.is_empty(), "no fabricated quota");
        assert_eq!(section.errors.len(), 2);
        assert!(section.errors.iter().all(|err| err.ends_with("HTTP 401")));
        let inactive = serde_json::json!({"eligible": false, "tier": null, "windows": []});
        assert_eq!(opendesign_quota_windows(&inactive).unwrap_err(), "当前账户没有生效的 Design Plan");
        assert!(opendesign_quota_windows(&serde_json::json!({"windows": [{"durationSeconds": 18000, "usedCredits": "x", "limitCredits": "20"}]})).is_err());
    }
}
