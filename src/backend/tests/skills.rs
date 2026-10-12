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
