#[cfg(test)]
mod opendesign_auth_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    async fn serve(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    fn device_fixture(polls: Arc<std::sync::atomic::AtomicUsize>) -> axum::Router {
        use std::sync::atomic::Ordering;
        axum::Router::new()
            .route(
                "/api/v1/cli/device-authorizations",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    assert_eq!(body["profile"], "prod");
                    assert!(body["cliVersion"].as_str().is_some_and(|v| !v.is_empty()));
                    assert!(body["host"].as_str().is_some_and(|v| !v.is_empty()));
                    axum::Json(serde_json::json!({
                        "deviceId": "dev1",
                        "deviceSecret": "[REDACTED]",
                        "userCode": "AB12CD",
                        "activationUrl": "https://open-design.ai/cloud/cli/activate?deviceId=dev1&userCode=AB12CD",
                        "pollIntervalSeconds": 1,
                        "expiresAt": "2099-01-01T00:00:00Z"
                    }))
                }),
            )
            .route(
                "/api/v1/cli/device-authorizations/dev1/token",
                axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                    let polls = polls.clone();
                    async move {
                        assert_eq!(body["deviceSecret"], "[REDACTED]");
                        if polls.fetch_add(1, Ordering::SeqCst) == 0 {
                            axum::Json(serde_json::json!({"status": "pending"}))
                        } else {
                            axum::Json(serde_json::json!({
                                "status": "approved",
                                "controlKey": "vela_ctrl_[REDACTED]",
                                "runtimeKey": "sk-amr-[REDACTED]",
                                "user": {"email": "[REDACTED]", "id": "u1"}
                            }))
                        }
                    }
                }),
            )
    }

    #[tokio::test]
    async fn device_login_stores_the_control_key_in_a_private_file() {
        let polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let base = serve(device_fixture(polls)).await;
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        *opendesign_pending().lock().await = None;

        let started = opendesign_device_start_from_base(&state, &base).await.unwrap();
        assert_eq!(started["status"], "pending");
        assert_eq!(started["user_code"], "AB12CD");
        assert!(started["activation_url"].as_str().unwrap().contains("activate"));
        assert_eq!(started["poll_interval_seconds"], 1);

        // The first poll reaches upstream and is still pending.
        assert_eq!(opendesign_device_poll_from_base(&state, &base).await.unwrap()["status"], "pending");
        // A second call inside the poll interval must not hit upstream again.
        assert_eq!(opendesign_device_poll_from_base(&state, &base).await.unwrap()["status"], "pending");
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let connected = opendesign_device_poll_from_base(&state, &base).await.unwrap();
        assert_eq!(connected["status"], "connected");

        let path = temp.path().join("state/opendesign-control-key.json");
        let stored: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored["control_key"], "vela_ctrl_[REDACTED]");
        assert_eq!(stored["email"], "[REDACTED]");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "control key file must stay private");
        assert!(opendesign_pending().lock().await.is_none(), "pending cleared after connect");
        assert!(provider_usage_catalog(temp.path()).iter().any(|meta| meta.provider == "opendesign" && meta.query_ready));
    }

    #[tokio::test]
    async fn stored_control_key_drives_the_collector_and_reports_revoked_grants() {
        let state_checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let checks = state_checks.clone();
        let app = axum::Router::new()
            .route(
                "/api/v1/workspaces/current",
                axum::routing::get(move |headers: axum::http::HeaderMap| {
                    let checks = checks.clone();
                    async move {
                        checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        assert_eq!(headers["authorization"], "Bearer vela_ctrl_[REDACTED]");
                        assert_eq!(headers["x-vela-workspace-id"], "");
                        axum::Json(serde_json::json!({"workspaceId": "ws1", "activeWorkspaceId": "ws1"}))
                    }
                }),
            )
            .route(
                "/api/v1/workspaces/ws1/billing/preflight",
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!({
                        "codingPlan": {
                            "eligible": true,
                            "tier": "go",
                            "windows": [{
                                "policyId": "w5", "durationSeconds": 18000,
                                "usedCredits": "3", "limitCredits": "20",
                                "resetsAt": "2026-11-04T03:00:00Z"
                            }]
                        }
                    }))
                }),
            )
            .route(
                "/api/v1/workspaces/ws1/wallet/balance",
                axum::routing::get(|| async {
                    (axum::http::StatusCode::UNAUTHORIZED, "{}")
                }),
            );
        let base = serve(app).await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("state")).unwrap();
        std::fs::write(
            temp.path().join("state/opendesign-control-key.json"),
            "{\"control_key\":\"vela_ctrl_[REDACTED]\",\"email\":\"[REDACTED]\",\"created_at\":1}",
        )
        .unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_opendesign_usage_from_base(&state, &base).await;
        assert_eq!(section.windows.len(), 1);
        assert_eq!(section.windows[0].used.as_deref(), Some("15.0%"));
        assert!(section.errors.iter().any(|err| err.starts_with("钱包查询失败")));
        assert!(state_checks.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    }

    #[test]
    fn opendesign_weekly_window_feeds_the_daily_counter() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let window = |label: &str, used: &str| crate::backend::ProviderUsageWindow {
            window: label.into(),
            used: Some(used.into()),
            reset: Some("11/11 18:58".into()),
            reset_at: Some(1793836800),
        };
        let mut first = crate::backend::ProviderUsageSection {
            provider: "opendesign".into(),
            windows: vec![window("5h额度", "90.0%"), window("周额度", "15.0%")],
            ..Default::default()
        };
        provider_capture_quota_daily(&state, &mut first, 1_000).unwrap();
        assert_eq!(first.daily.len(), 1, "only the weekly window is a daily counter");
        assert_eq!(first.daily[0].account, "用量");
        assert_eq!(first.daily[0].used, 0.0);

        let mut later = crate::backend::ProviderUsageSection {
            provider: "opendesign".into(),
            windows: vec![window("5h额度", "5.0%"), window("周额度", "20.0%")],
            ..Default::default()
        };
        provider_capture_quota_daily(&state, &mut later, 1_300).unwrap();
        assert_eq!(later.daily.len(), 1);
        assert!((later.daily[0].used - 5.0).abs() < 1e-9, "weekly delta becomes today's usage");
    }

    #[tokio::test]
    async fn revoked_stored_key_asks_for_a_new_device_login() {
        let app = axum::Router::new()
            .route(
                "/api/v1/workspaces/current",
                axum::routing::get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "{}") }),
            )
            .route(
                "/api/v1/workspaces/ws1/billing/preflight",
                axum::routing::get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "{}") }),
            );
        let base = serve(app).await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("state")).unwrap();
        std::fs::write(
            temp.path().join("state/opendesign-control-key.json"),
            "{\"control_key\":\"vela_ctrl_[REDACTED]\",\"email\":\"\",\"created_at\":1}",
        )
        .unwrap();
        std::fs::write(temp.path().join(".env"), "OPENDESIGN_WORKSPACE_ID='ws1'\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_opendesign_usage_from_base(&state, &base).await;
        assert!(section.windows.is_empty());
        assert!(section.errors.iter().any(|err| err == "OpenDesign 授权已失效，请重新连接"));
        assert!(!section.errors.iter().any(|err| err.contains("[REDACTED]")));
        assert!(temp.path().join("state/opendesign-control-key.json").is_file(), "a revoked key is not silently deleted");
    }
}
