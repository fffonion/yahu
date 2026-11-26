    #[tokio::test]
    async fn ui_preferences_persist_order_and_reject_stale_writes() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(String::new(), temp.path()));
        let request = |body| Json(serde_json::from_value::<UiPreferencesPatch>(body).unwrap());
        let initial = patch_ui_preferences(State(state.clone()), request(serde_json::json!({
            "revision": 0, "pinned_sessions": ["second", "first"], "enabled_providers": ["codex"]
        }))).await;
        assert_eq!(initial.status(), StatusCode::OK);
        let stale = patch_ui_preferences(State(state.clone()), request(serde_json::json!({
            "revision": 0, "provider_order": ["grok", "codex"]
        }))).await;
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        let applied = patch_ui_preferences(State(state.clone()), request(serde_json::json!({
            "revision": 1, "provider_order": ["grok", "codex"], "pinned_sessions": []
        }))).await;
        assert_eq!(applied.status(), StatusCode::OK);
        let reopened = Arc::new(test_app_state(String::new(), temp.path()));
        let response = get_ui_preferences(State(reopened)).await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({"revision":2,"pinned_sessions":[],"enabled_providers":["codex"],"provider_order":["grok","codex"]}));
        assert!(temp.path().join("yahu/preferences.json").is_file());
    }

    #[test]
    fn yahu_json_migration_preserves_active_files_and_unrelated_hermes_state() {
        let root = tempfile::tempdir().unwrap();
        for (name, content) in [
            ("skill-recent.json", "[\"old\"]"),
            ("state/codex-daily-usage.json", "{\"today\":7}"),
            ("state/opendesign-control-key.json", "{\"fixture\":true}"),
            ("cache/yahu/model-inventory.json", "[]"),
            ("state/unrelated-daily-usage.json", "null"),
            ("auth.json", "{}"),
        ] {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        std::fs::create_dir(root.path().join("yahu")).unwrap();
        std::fs::write(root.path().join("yahu/skill-recent.json"), "[\"new\"]").unwrap();
        migrate_yahu_json(root.path()).unwrap();
        for (name, expected) in [("skill-recent.json", "[\"new\"]"), ("codex-daily-usage.json", "{\"today\":7}"), ("model-inventory.json", "[]")] {
            assert_eq!(std::fs::read_to_string(root.path().join("yahu").join(name)).unwrap(), expected);
        }
        assert!(root.path().join("state/codex-daily-usage.json").is_file());
        assert!(!root.path().join("yahu/auth.json").exists());
        assert!(!root.path().join("yahu/unrelated-daily-usage.json").exists());
        let private = root.path().join("yahu/opendesign-control-key.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&private).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_file(&private).unwrap();
        migrate_yahu_json(root.path()).unwrap();
        assert!(!private.exists());
    }
