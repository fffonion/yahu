// OpenDesign device-authorization sign-in, storing the issued control key in a
// Yahu-owned private file.
//
// Contract verified against the official client (`vela login`) and the public
// CLI-login web flow:
// - POST /api/v1/cli/device-authorizations {cliVersion, host, profile}
//   -> {deviceId, deviceSecret, userCode, activationUrl, pollIntervalSeconds,
//       expiresAt}
// - POST /api/v1/cli/device-authorizations/{deviceId}/token {deviceSecret}
//   -> pending, or {status, controlKey, runtimeKey, user:{email,id}}
// The issued keys are long-lived: the control plane has no OAuth2 refresh
// token, so Yahu re-runs this device authorization when a stored key is
// rejected instead of pretending to refresh it.

const OPENDESIGN_CLI_VERSION: &str = "0.1.4";
const OPENDESIGN_DEVICE_FALLBACK_TTL: i64 = 10 * 60;

#[derive(Serialize, Deserialize, Clone)]
struct OpenDesignStoredKey {
    control_key: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    created_at: i64,
}

#[derive(Clone)]
struct OpenDesignPendingDevice {
    device_id: String,
    device_secret: String,
    poll_interval_seconds: u64,
    expires_at: i64,
    last_poll: i64,
}

static OPENDESIGN_PENDING: OnceLock<tokio::sync::Mutex<Option<OpenDesignPendingDevice>>> =
    OnceLock::new();

fn opendesign_pending() -> &'static tokio::sync::Mutex<Option<OpenDesignPendingDevice>> {
    OPENDESIGN_PENDING.get_or_init(|| tokio::sync::Mutex::new(None))
}

fn opendesign_key_path(hermes_home: &Path) -> std::path::PathBuf {
    hermes_home.join("state/opendesign-control-key.json")
}

fn load_opendesign_stored_key(hermes_home: &Path) -> Option<OpenDesignStoredKey> {
    let raw = std::fs::read_to_string(opendesign_key_path(hermes_home)).ok()?;
    let stored: OpenDesignStoredKey = serde_json::from_str(&raw).ok()?;
    (!stored.control_key.trim().is_empty()).then_some(stored)
}

fn save_opendesign_stored_key(
    hermes_home: &Path,
    control_key: &str,
    email: &str,
) -> Result<OpenDesignStoredKey, String> {
    let stored = OpenDesignStoredKey {
        control_key: control_key.trim().to_string(),
        email: email.trim().to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };
    save_private_provider_session(&opendesign_key_path(hermes_home), &stored)?;
    Ok(stored)
}

fn opendesign_api_base(hermes_home: &Path) -> String {
    let base = provider_env_value(hermes_home, "OPENDESIGN_API_BASE_URL");
    if base.is_empty() {
        OPENDESIGN_API_BASE.to_string()
    } else {
        base.trim_end_matches('/').to_string()
    }
}

fn opendesign_device_host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "yahu".to_string())
}

async fn opendesign_device_start_from_base(
    state: &AppState,
    base: &str,
) -> Result<Value, String> {
    let body = serde_json::json!({
        "cliVersion": OPENDESIGN_CLI_VERSION,
        "host": opendesign_device_host(),
        "profile": "prod",
    });
    let payload = provider_http_post_json(
        &state.client,
        &format!("{base}/api/v1/cli/device-authorizations"),
        body,
        &[("Accept".to_string(), "application/json".to_string())],
    )
    .await
    .map_err(|err| format!("发起设备授权失败：{}", opendesign_safe_error(&err)))?;
    let field = |key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let (Some(device_id), Some(device_secret), Some(user_code), Some(activation_url)) = (
        field("deviceId"),
        field("deviceSecret"),
        field("userCode"),
        field("activationUrl"),
    ) else {
        return Err("设备授权响应缺少必要字段".into());
    };
    let interval = payload
        .get("pollIntervalSeconds")
        .and_then(provider_number)
        .filter(|value| *value >= 1.0)
        .map(|value| value as u64)
        .unwrap_or(2);
    let expires_at = payload
        .get("expiresAt")
        .and_then(codex_timestamp)
        .unwrap_or_else(|| chrono::Utc::now().timestamp() + OPENDESIGN_DEVICE_FALLBACK_TTL);
    *opendesign_pending().lock().await = Some(OpenDesignPendingDevice {
        device_id,
        device_secret,
        poll_interval_seconds: interval,
        expires_at,
        last_poll: 0,
    });
    Ok(serde_json::json!({
        "status": "pending",
        "user_code": user_code,
        "activation_url": activation_url,
        "expires_at": expires_at,
        "poll_interval_seconds": interval,
    }))
}

async fn opendesign_device_poll_from_base(
    state: &AppState,
    base: &str,
) -> Result<Value, String> {
    let Some(pending) = opendesign_pending().lock().await.clone() else {
        return Err("没有进行中的设备授权".into());
    };
    let now = chrono::Utc::now().timestamp();
    if now > pending.expires_at {
        *opendesign_pending().lock().await = None;
        return Err("设备授权已过期，请重新连接".into());
    }
    // Respect the upstream poll interval while the browser approval is open.
    if now - pending.last_poll < pending.poll_interval_seconds as i64 {
        return Ok(serde_json::json!({"status": "pending"}));
    }
    if let Some(current) = opendesign_pending().lock().await.as_mut() {
        current.last_poll = now;
    }
    let payload = provider_http_post_json(
        &state.client,
        &format!(
            "{base}/api/v1/cli/device-authorizations/{}/token",
            urlencoding_lite(&pending.device_id)
        ),
        serde_json::json!({"deviceSecret": pending.device_secret}),
        &[("Accept".to_string(), "application/json".to_string())],
    )
    .await;
    let payload = match payload {
        Ok(payload) => payload,
        Err(err) => {
            // A network hiccup on the way to the approval is not a failure.
            return Ok(serde_json::json!({
                "status": "pending",
                "warning": opendesign_safe_error(&err),
            }));
        }
    };
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let control_key = payload
        .get("controlKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(control_key) = control_key {
        let email = payload
            .get("user")
            .and_then(|user| user.get("email"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let stored = save_opendesign_stored_key(&state.hermes_home, control_key, email)?;
        *opendesign_pending().lock().await = None;
        return Ok(serde_json::json!({
            "status": "connected",
            "account": stored.email,
        }));
    }
    if matches!(
        status.as_str(),
        "expired" | "canceled" | "cancelled" | "denied" | "rejected"
    ) {
        *opendesign_pending().lock().await = None;
        return Err("设备授权未获批准，请重新连接".into());
    }
    Ok(serde_json::json!({"status": "pending"}))
}

async fn opendesign_auth_start(State(state): State<Arc<AppState>>) -> Response<Body> {
    let base = opendesign_api_base(&state.hermes_home);
    match opendesign_device_start_from_base(&state, &base).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err),
    }
}

async fn opendesign_auth_status(State(state): State<Arc<AppState>>) -> Response<Body> {
    let base = opendesign_api_base(&state.hermes_home);
    match opendesign_device_poll_from_base(&state, &base).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err),
    }
}

async fn opendesign_auth_cancel() -> Response<Body> {
    *opendesign_pending().lock().await = None;
    (StatusCode::OK, Json(serde_json::json!({"status": "cancelled"}))).into_response()
}
