    #[tokio::test]
    async fn models_cache_reuses_fresh_persisted_inventory_without_calling_upstream() {
        let temp = tempfile::tempdir().unwrap();
        let cache_path = temp.path().join("cache/yahu/model-inventory.json");
        persist_model_cache_body(
            &cache_path,
            &serde_json::json!({"object":"list","data":[{"id":"MiniMax-M3","provider":"minimax-cn"}]}),
            std::time::SystemTime::now(),
        )
        .unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let response = models_cached(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body["data"][0]["id"], "MiniMax-M3");
    }
