    #[tokio::test]
    async fn sensenova_rotated_credentials_use_private_session_cache_not_env() {
        let temp = tempfile::tempdir().unwrap();
        let original = "OTHER=unchanged\nSENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\nSENSENOVA_ACCESS_TOKEN=[REDACTED]\nSENSENOVA_REFRESH_TOKEN=[REDACTED]\n";
        std::fs::write(temp.path().join(".env"), original).unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let prior = sensenova_current_auth(&state).await;
        sensenova_persist_auth(temp.path(), &prior, "[REDACTED]", "[REDACTED]", None).unwrap();
        assert!(std::fs::read_to_string(temp.path().join(".env")).unwrap() == original,
            "renewal must not rewrite static configuration");
        let path = temp.path().join("state/sensenova_service_token.json");
        assert!(path.is_file(), "renewal must create a provider-owned session cache");
        let cache: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(cache["access_token"].as_str() == Some("[REDACTED]"), "access cache mismatch");
        assert!(cache["refresh_token"].as_str() == Some("[REDACTED]"), "refresh cache mismatch");
        assert!(cache.get("password").is_none(), "password must not be copied into the session cache");
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    fn write_sensenova_test_session(home: &Path, access: &str, refresh: &str, expires_at: Option<i64>) {
        save_private_provider_session(&home.join("state/sensenova_service_token.json"), &serde_json::json!({
            "username":"[REDACTED]", "access_token":access, "refresh_token":refresh, "expires_at":expires_at,
        })).unwrap();
    }

    async fn sensenova_auth_fixture_server() -> (String, Arc<Mutex<Vec<String>>>) {
        use axum::response::IntoResponse as _;
        use axum::extract::Form;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let oauth_state = Arc::new(Mutex::new(String::new()));
        let authorization_state = oauth_state.clone();
        let complete_state = oauth_state.clone();
        let authorize_base = base.clone();
        let login_base = base.clone();
        let callback_base = base.clone();
        let login_events = events.clone();
        let token_events = events.clone();
        let usage_events = events.clone();
        let app = axum::Router::new()
            .route("/oauth2/auth", get(move |Query(query): Query<HashMap<String, String>>| {
                let state = authorization_state.clone(); let base = authorize_base.clone();
                async move {
                    assert_eq!(query.get("client_id").map(String::as_str), Some("nova"));
                    assert_eq!(query.get("code_challenge_method").map(String::as_str), Some("S256"));
                    *state.lock().await = query.get("state").cloned().unwrap();
                    (StatusCode::FOUND, [("location", format!("{base}/login?login_challenge=%5BREDACTED%5D")), ("set-cookie", "flow=[REDACTED]; Path=/".to_string())])
                }
            }))
            .route("/.well-known/jwks.json", get(|| async { Json(serde_json::json!({"keys":[]})) }))
            .route("/iam/authn/v1/auth/nova/login", post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let events = login_events.clone(); let base = login_base.clone();
                async move {
                    assert!(body["username"].as_str() == Some("[REDACTED]") && body["password"].as_str() == Some("[REDACTED]"), "login credential mismatch");
                    assert!(body["challenge"].as_str() == Some("[REDACTED]"), "challenge mismatch");
                    assert!(!headers.contains_key("authorization"), "do not send an old bearer token to the login service");
                    events.lock().await.push("login".into());
                    Json(serde_json::json!({"data":{"redirect_uri":format!("{base}/complete-login")}}))
                }
            }))
            .route("/complete-login", get(move || {
                let state = complete_state.clone(); let base = callback_base.clone();
                async move { (StatusCode::FOUND, [("location", format!("{base}/?code=%5BREDACTED%5D&state={}", state.lock().await))]) }
            }))
            .route("/oauth2/token", post(move |Form(form): Form<HashMap<String, String>>| {
                let events = token_events.clone();
                async move {
                    if form.get("grant_type").map(String::as_str) == Some("refresh_token") {
                        events.lock().await.push("refresh".into());
                        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid_grant","error_description":"[REDACTED]"}))).into_response()
                    } else {
                        assert_eq!(form.get("grant_type").map(String::as_str), Some("authorization_code"));
                        assert!(form.get("code_verifier").is_some_and(|value| value.len() >= 43), "PKCE verifier missing");
                        events.lock().await.push("exchange".into());
                        Json(serde_json::json!({"access_token":"[REDACTED]","refresh_token":"[REDACTED]","expires_in":10800})).into_response()
                    }
                }
            }))
            .route("/pool-usage", get(move || {
                let events = usage_events.clone();
                async move {
                    events.lock().await.push("usage".into());
                    Json(serde_json::from_str::<Value>(include_str!("fixtures/sensenova-pool-usage.json")).unwrap())
                }
            }));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (base, events)
    }

    #[tokio::test]
    async fn sensenova_password_login_bootstraps_a_missing_session() {
        let (base, events) = sensenova_auth_fixture_server().await;
        let temp = tempfile::tempdir().unwrap();
        let original = "OTHER=unchanged\nSENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\n";
        std::fs::write(temp.path().join(".env"), original).unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_sensenova_usage_from_url(&state, &format!("{base}/pool-usage")).await;
        assert!(section.errors.is_empty(), "username/password should bootstrap a new service session");
        assert_eq!(*events.lock().await, vec!["login", "exchange", "usage"]);
        assert!(std::fs::read_to_string(temp.path().join(".env")).unwrap() == original, "login must not alter static credentials");
        assert!(temp.path().join("state/sensenova_service_token.json").is_file());
        assert!(!serde_json::to_string(&section).unwrap().contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn sensenova_expired_refresh_token_skips_refresh_and_relogs() {
        let (base, events) = sensenova_auth_fixture_server().await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "SENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\n").unwrap();
        let expired_claims = URL_SAFE_NO_PAD.encode(serde_json::json!({"exp":1}).to_string());
        save_private_provider_session(&temp.path().join("state/sensenova_service_token.json"), &serde_json::json!({
            "username":"[REDACTED]", "access_token":"[REDACTED]",
            "refresh_token":format!("[REDACTED].{expired_claims}.[REDACTED]"), "expires_at":1,
        })).unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_sensenova_usage_from_url(&state, &format!("{base}/pool-usage")).await;
        assert!(section.errors.is_empty(), "expired refresh token must recover with password login");
        assert_eq!(*events.lock().await, vec!["login", "exchange", "usage"]);
    }

    #[tokio::test]
    async fn sensenova_invalid_grant_refresh_relogs_once() {
        let (base, events) = sensenova_auth_fixture_server().await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "SENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\n").unwrap();
        save_private_provider_session(&temp.path().join("state/sensenova_service_token.json"), &serde_json::json!({
            "username":"[REDACTED]", "access_token":"[REDACTED]", "refresh_token":"[REDACTED]", "expires_at":1,
        })).unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_sensenova_usage_from_url(&state, &format!("{base}/pool-usage")).await;
        assert!(section.errors.is_empty(), "an explicitly rejected refresh token must recover through password login");
        assert_eq!(*events.lock().await, vec!["refresh", "login", "exchange", "usage"]);
    }

    #[tokio::test]
    async fn sensenova_concurrent_password_login_is_single_flight() {
        let (base, events) = sensenova_auth_fixture_server().await;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "SENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let url = format!("{base}/pool-usage");
        let (first, second) = tokio::join!(fetch_sensenova_usage_from_url(&state, &url), fetch_sensenova_usage_from_url(&state, &url));
        assert!(first.errors.is_empty() && second.errors.is_empty());
        assert_eq!(*events.lock().await, vec!["login", "exchange", "usage", "usage"]);
    }

    #[test]
    fn sensenova_catalog_is_ready_with_only_username_and_password() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "SENSENOVA_USERNAME=[REDACTED]\nSENSENOVA_PASSWORD=[REDACTED]\n").unwrap();
        let provider = provider_usage_catalog(temp.path()).into_iter().find(|item| item.provider == "sensenova").unwrap();
        assert!(provider.configured && provider.query_ready, "static account credentials should enable automatic login");
    }

    #[tokio::test]
    #[ignore = "opt-in real SenseNova password login writes only the private session cache"]
    async fn sensenova_live_password_login_smoke() {
        let home = PathBuf::from(env::var("SENSENOVA_LIVE_HOME").expect("explicit live home required"));
        let state = test_app_state("http://127.0.0.1:1".into(), &home);
        let before = std::fs::read(home.join(".env")).unwrap();
        let prior = sensenova_current_auth(&state).await;
        let result = sensenova_password_auth(&state, SENSENOVA_POOL_USAGE_URL, &prior).await;
        assert!(result.is_ok(), "real password login failed");
        let auth = result.unwrap();
        let restored = load_sensenova_service_auth(&home).expect("saved service session required");
        assert!(restored.access == auth.access && restored.refresh == auth.refresh, "private credential readback mismatch");
        assert!(std::fs::read(home.join(".env")).unwrap() == before, "static configuration was changed");
        let usage = fetch_sensenova_usage(&state).await;
        assert!(usage.errors.is_empty() && !usage.pools.is_empty(), "authenticated usage query failed");
        println!("Real password login, private session readback and usage query passed");
    }
