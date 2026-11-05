    #[tokio::test]
    async fn skills_inventory_uses_managed_runtime_without_legacy_venv() {
        let temp = tempfile::tempdir().unwrap();
        let agent = temp.path().join("hermes-agent");
        std::fs::create_dir_all(agent.join(".hermes/bin")).unwrap();
        std::fs::create_dir_all(agent.join("hermes_cli")).unwrap();
        std::fs::write(agent.join(".hermes/bin/hermes"), "managed installation marker").unwrap();
        std::fs::write(agent.join("hermes_cli/__init__.py"), "").unwrap();
        std::fs::write(agent.join("hermes_cli/_launchers.py"), r#"
import os, sys

def runtime_command(root, args=(), *, code=None):
    assert 'def inventory(' in code
    assert os.environ['HERMES_HOME'] == str(root.parent)
    script = "import json,sys; print(json.dumps({'data':[{'name':'managed-skill'}], 'optional':[], 'roots':sys.argv[1:]}))"
    return [sys.executable, '-c', script, *args]
"#).unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".into(), temp.path()));
        let response = skills_list(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["data"][0]["name"], "managed-skill");
        assert_eq!(payload["roots"][0], temp.path().join("skills").to_string_lossy().as_ref());
        assert_eq!(payload["roots"][1], agent.join("optional-skills").to_string_lossy().as_ref());
    }

    #[tokio::test]
    async fn skills_inventory_reads_editorial_locally_without_api() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("skills/testing/canonical-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "---\nname: canonical-skill\ndescription: Canonical description\nversion: '1.2.3'\nmetadata:\n  hermes:\n    editorial_name: 'Display title'\n    editorial_description: >-\n      Human-facing\n      description\n---\n# Skill\n").unwrap();
        std::fs::write(temp.path().join("config.yaml"), "skills:\n  disabled: [canonical-skill]\n").unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        let response = skills_list(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["data"].as_array().unwrap().len(), 1);
        let row = &payload["data"][0];
        assert_eq!(row["name"], "canonical-skill");
        assert_eq!(row["description"], "Canonical description");
        assert_eq!(row["editorial_name"], "Display title");
        assert_eq!(row["editorial_description"], "Human-facing description");
        assert_eq!(row["version"], "1.2.3");
        assert_eq!(row["enabled"], false);
        assert_eq!(row["category"], "testing");
        let original = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        let modified = original.replace("Display title", "Updated display title");
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        let response = skill_file_write(
            State(state.clone()),
            Query(SkillQuery { name: Some("canonical-skill".to_string()), path: Some("SKILL.md".to_string()) }),
            Json(WorkspaceSavePayload { content: modified.clone() }),
        ).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(std::fs::read_to_string(dir.join("SKILL.md")).unwrap(), modified);
        let response = skills_list(State(state)).await;
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated["data"][0]["name"], "canonical-skill");
        assert_eq!(updated["data"][0]["editorial_name"], "Updated display title");
    }

    #[tokio::test]
    async fn skills_inventory_separates_optional_and_uses_hermes_name_and_platform_rules() {
        let temp = tempfile::tempdir().unwrap();
        for (relative, text) in [
            ("skills/testing/unnamed", "# Unnamed\nFallback description\n"),
            ("skills/testing/enabled", "---\nname: enabled\ndescription: Enabled\n---\n"),
            ("skills/testing/disabled", "---\nname: disabled\ndescription: Disabled\n---\n"),
            ("skills/testing/mac-only", "---\nname: mac-only\ndescription: Mac\nplatforms: [macos]\n---\n"),
            ("hermes-agent/optional-skills/testing/candidate", "---\nname: candidate\ndescription: Optional\n---\n"),
        ] {
            let dir = temp.path().join(relative);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), text).unwrap();
        }
        std::fs::write(temp.path().join("config.yaml"), "skills:\n  disabled: [disabled]\n").unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".into(), temp.path()));
        let response = skills_list(State(state.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let data = payload["data"].as_array().unwrap();
        let names: HashSet<_> = data.iter().map(|row| row["name"].as_str().unwrap()).collect();
        assert_eq!(names, HashSet::from(["unnamed", "enabled", "disabled"]));
        assert_eq!(data.iter().filter(|row| row["enabled"] == true).count(), 2);
        assert!(data.iter().all(|row| row["installed"] == true));
        let optional = payload["optional"].as_array().unwrap();
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0]["name"], "candidate");
        assert_eq!(optional[0]["installed"], false);
        assert_eq!(optional[0]["enabled"], false);
        assert_eq!(optional[0]["mutable"], false);
        assert!(find_skill_dir(&state, "unnamed").is_ok());
        let response = skill_toggle(State(state), AxumPath("candidate".into()), Json(SkillTogglePayload { enabled: true })).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "optional candidates must not be presented as installed toggles");
    }

    #[tokio::test]
    async fn skills_inventory_fallback_preserves_root_precedence_and_skips_archives() {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("skills/testing/plain");
        let optional = temp.path().join("hermes-agent/optional-skills/testing/plain");
        let archive = temp.path().join("skills/testing/.archive/old");
        for dir in [&user, &optional, &archive] { std::fs::create_dir_all(dir).unwrap(); }
        std::fs::write(user.join("SKILL.md"), "---\nname: plain\ndescription: User copy\nmetadata:\n  hermes:\n    editorial_name: 42\n    editorial_description: '  '\n---\n").unwrap();
        std::fs::write(optional.join("SKILL.md"), "---\nname: plain\ndescription: Optional copy\n---\n").unwrap();
        std::fs::write(archive.join("SKILL.md"), "---\nname: old\ndescription: Archive\n---\n").unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        let response = skills_list(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["data"].as_array().unwrap().len(), 1);
        assert_eq!(payload["data"][0]["editorial_name"], "plain");
        assert_eq!(payload["data"][0]["editorial_description"], "User copy");
        assert_eq!(payload["data"][0]["enabled"], true);
    }
