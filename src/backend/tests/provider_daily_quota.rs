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
}
