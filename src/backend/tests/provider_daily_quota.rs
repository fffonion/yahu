#[cfg(test)]
mod provider_daily_quota_tests {
    use super::*;

    fn quota_section(provider: &str, a: f64, b: f64, epoch: i64) -> ProviderUsageSection {
        let (first, second) = if provider == "opencode" {
            ("5h额度", "月额度")
        } else {
            ("alpha 周额度", "beta 周额度")
        };
        ProviderUsageSection {
            provider: provider.into(),
            windows: vec![ProviderUsageWindow { window: first.into(), used: Some(format!("{a}%")), reset: None, reset_at: Some(epoch) },
                          ProviderUsageWindow { window: second.into(), used: Some(format!("{b}%")), reset: None, reset_at: Some(epoch) }],
            ..Default::default()
        }
    }

    #[test]
    fn quota_daily_capture_selects_one_long_window_per_account() {
        for provider in ["codex", "grok", "opencode"] {
            let temp = tempfile::tempdir().unwrap();
            let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
            let now = 1_793_325_600;
            let mut first = quota_section(provider, 20.0, 40.0, now + 604800);
            provider_capture_quota_daily(&state, &mut first, now).unwrap();
            let mut next = quota_section(provider, 25.0, 42.0, now + 604800);
            provider_capture_quota_daily(&state, &mut next, now + 300).unwrap();
            let daily = serde_json::to_value(&next).unwrap()["daily"].clone();
            if provider == "opencode" {
                assert_eq!(daily.as_array().unwrap().len(), 1);
                assert_eq!(daily[0]["used"], 2.0);
            } else {
                assert_eq!(daily[0]["used"], 5.0);
                assert_eq!(daily[1]["used"], 2.0);
            }
            assert_eq!(next.windows.len(), 2);
            assert!(provider_daily_snapshot_section(&state, provider, now + 301).is_some());
            assert!(provider_daily_snapshot_section(&state, provider, now + 601).is_none());
        }
    }

    #[test]
    fn quota_daily_capture_handles_epoch_reset_and_new_day_without_history_replay() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut first = quota_section("codex", 20.0, 40.0, now + 604800);
        provider_capture_quota_daily(&state, &mut first, now).unwrap();
        let mut next = quota_section("codex", 25.0, 42.0, now + 604800);
        provider_capture_quota_daily(&state, &mut next, now + 300).unwrap();
        next.windows[0].used = Some("2%".into());
        next.windows[0].reset_at = Some(now + 1209600);
        provider_capture_quota_daily(&state, &mut next, now + 600).unwrap();
        assert_eq!(serde_json::to_value(&next).unwrap()["daily"][0]["used"], 7.0);
        next.windows[0].used = Some("1.9%".into());
        provider_capture_quota_daily(&state, &mut next, now + 900).unwrap();
        assert_eq!(serde_json::to_value(&next).unwrap()["daily"][0]["used"], 7.0);
        provider_capture_quota_daily(&state, &mut next, now + 86400).unwrap();
        assert_eq!(serde_json::to_value(&next).unwrap()["daily"][0]["used"], 0.0);
    }

    #[test]
    fn quota_daily_epoch_changes_at_full_usage_do_not_replay_quota() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut section = ProviderUsageSection { provider: "codex".into(), title: "Codex 额度".into(), ..Default::default() };
        let counters = HashMap::from([("alpha".into(), (now + 604800, 100.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now).unwrap();
        for observation in 1..=4 {
            let counters = HashMap::from([("alpha".into(), (now + 604800 + observation * 300, 100.0))]);
            provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + observation * 300).unwrap();
        }
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 0.0);
        let counters = HashMap::from([("alpha".into(), (now + 606000, 0.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 1500).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 0.0);
        let counters = HashMap::from([("alpha".into(), (now + 606000, 5.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 1800).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 5.0);
    }

    #[test]
    fn quota_daily_epoch_change_without_reset_counts_later_increment() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut section = ProviderUsageSection { provider: "codex".into(), title: "Codex 额度".into(), ..Default::default() };
        let counters = HashMap::from([("alpha".into(), (now + 604800, 20.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now).unwrap();
        let counters = HashMap::from([("alpha".into(), (now + 1209600, 20.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 300).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 0.0);
        let counters = HashMap::from([("alpha".into(), (now + 1209600, 25.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 600).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 5.0);
        let counters = HashMap::from([("alpha".into(), (now + 1814400, 2.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 900).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 7.0);
    }

    #[test]
    fn quota_daily_elapsed_reset_counts_usage_in_the_new_window() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut section = ProviderUsageSection { provider: "codex".into(), title: "Codex 额度".into(), ..Default::default() };
        let counters = HashMap::from([("alpha".into(), (now + 300, 95.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now).unwrap();
        let counters = HashMap::from([("alpha".into(), (now + 86400, 25.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 600).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"][0]["used"], 25.0);
    }

    #[test]
    fn quota_daily_partial_account_failure_preserves_other_account_and_disk_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut first = quota_section("grok", 20.0, 40.0, now + 604800);
        provider_capture_quota_daily(&state, &mut first, now).unwrap();
        let mut next = quota_section("grok", 25.0, 99.0, now + 604800);
        next.errors = vec!["beta：查询失败：HTTP 503".into()];
        provider_capture_quota_daily(&state, &mut next, now + 300).unwrap();
        let daily = serde_json::to_value(&next).unwrap()["daily"].clone();
        assert_eq!(daily.as_array().unwrap().len(), 1);
        assert_eq!(daily[0]["account"], "alpha");
        assert_eq!(daily[0]["used"], 5.0);
        let mut recovered = quota_section("grok", 25.0, 42.0, now + 604800);
        provider_capture_quota_daily(&state, &mut recovered, now + 600).unwrap();
        assert_eq!(serde_json::to_value(&recovered).unwrap()["daily"][1]["used"], 2.0);
    }

    #[tokio::test]
    async fn quota_daily_catalog_and_report_share_the_persisted_section() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".into(), temp.path()));
        let now = chrono::Utc::now().timestamp();
        let mut section = quota_section("opencode", 20.0, 40.0, now + 604800);
        provider_capture_quota_daily(&state, &mut section, now - 60).unwrap();
        section.windows[1].used = Some("45%".into());
        provider_capture_quota_daily(&state, &mut section, now).unwrap();
        let response = provider_usage_handler(State(state.clone()), Query(ProviderUsageQuery { provider: None, refresh: Some(false) })).await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&bytes).unwrap();
        let served = payload["sections"].as_array().unwrap().iter().find(|s| s["provider"] == "opencode").unwrap();
        assert_eq!(served["daily"][0]["used"], 5.0);
        let stored = provider_daily_snapshot_section(&state, "opencode", now).unwrap();
        assert_eq!(serde_json::to_value(stored).unwrap()["daily"], served["daily"]);
    }

    #[test]
    fn quota_daily_snapshot_keeps_separate_accounts_and_repeatable_day_totals() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = 1_793_325_600;
        let mut section = ProviderUsageSection { provider: "codex".into(), title: "Codex 额度".into(), ..Default::default() };
        let counters = HashMap::from([("alpha".into(), (100, 20.0)), ("beta".into(), (200, 40.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now).unwrap();
        let counters = HashMap::from([("alpha".into(), (100, 25.0)), ("beta".into(), (200, 42.0))]);
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters.clone(), now + 300).unwrap();
        let value = serde_json::to_value(&section).unwrap();
        assert_eq!(value["daily"][0]["account"], "alpha");
        assert_eq!(value["daily"][0]["used"], 5.0);
        assert_eq!(value["daily"][1]["account"], "beta");
        assert_eq!(value["daily"][1]["used"], 2.0);
        assert!(section.windows.is_empty());
        provider_observed_daily(&state, &mut section, "codex", "percentage_points", counters, now + 600).unwrap();
        assert_eq!(serde_json::to_value(&section).unwrap()["daily"], value["daily"]);
        let persisted = provider_daily_snapshot_section(&state, "codex", now + 601).unwrap();
        assert_eq!(serde_json::to_value(persisted).unwrap()["daily"], value["daily"]);
    }
    #[tokio::test]
    async fn antigravity_catalog_reads_daily_snapshot_without_querying_cli() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".into(), temp.path()));
        let now = chrono::Utc::now().timestamp();
        let mut section = ProviderUsageSection {
            provider: "antigravity".into(),
            title: "Antigravity 用量".into(),
            ..Default::default()
        };
        for used in [12.0, 12.625] {
            provider_observed_daily(&state, &mut section, "antigravity", "percentage_points",
                HashMap::from([("Gemini".into(), (now + 3600, used))]), now).unwrap();
        }
        let response = provider_usage_handler(State(state), Query(ProviderUsageQuery {
            provider: None, refresh: None,
        })).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let antigravity = json["sections"].as_array().unwrap().iter()
            .find(|section| section["provider"] == "antigravity")
            .expect("catalog must include the Antigravity daily snapshot");
        assert_eq!(antigravity["daily"][0]["used"], 0.625);
    }

    #[test]
    fn antigravity_ignores_third_party_buckets_and_preserves_weekly_precision() {
        let at = 1_800_000_000;
        let reset = chrono::DateTime::from_timestamp(at + 4500, 0).unwrap().to_rfc3339();
        let mut payload = serde_json::json!({
            "status": "SUCCESS", "num_turns": 0,
            "command": {"name": "usage", "data": {"groups": [
                {"name": "Claude and GPT models", "buckets": [
                    {"id": "3p-weekly", "window": "weekly", "remaining_fraction": 0.1, "reset_time": reset}
                ]},
                {"name": "Gemini Models", "buckets": [
                    {"id": "gemini-weekly", "window": "weekly", "remaining_fraction": 0.875125, "reset_time": reset},
                    {"id": "gemini-5h", "window": "5h", "remaining_fraction": 0.2, "reset_time": reset}
                ]}
            ]}}
        });
        let snapshot = antigravity_quota_snapshot(&serde_json::to_vec(&payload).unwrap(), at).unwrap();
        assert_eq!(snapshot.windows.map(|w| (w.window, w.used, w.reset_at)), [
            ("5h额度".into(), Some("80.0%".into()), Some(at + 4500)),
            ("周额度".into(), Some("12.5%".into()), Some(at + 4500)),
        ]);
        assert_eq!(snapshot.weekly_counter.0, at + 4500);
        assert!((snapshot.weekly_counter.1 - 12.4875).abs() < 1e-9);

        payload["command"]["data"]["groups"][1]["buckets"][0]["remaining_fraction"] = serde_json::json!(1.2);
        assert!(antigravity_quota_snapshot(&serde_json::to_vec(&payload).unwrap(), at).is_err());
    }
}
