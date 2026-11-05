// OpenDesign (Open Design Cloud) Coding Plan usage.
//
// Verified against the official client and dashboard bundles:
// - Cookie mode: GET /api/v1/workspaces/current, then
//   GET /api/v1/workspaces/{workspaceId}/billing/coding-plan-usage
// - Control Key mode: GET /api/v1/workspaces/{workspaceId}/billing/preflight
// - Package balance: GET /api/v1/workspaces/{workspaceId}/wallet/balance
// Inference API keys created on the API Keys page cannot read account quota.

const OPENDESIGN_API_BASE: &str = "https://amr-api.open-design.ai";

fn opendesign_credential(state: &AppState) -> OpenDesignCredential {
    let control_key = first_provider_env_value(
        &state.hermes_home,
        &["OPENDESIGN_CONTROL_KEY", "VELA_CONTROL_KEY"],
    );
    if !control_key.is_empty() {
        return OpenDesignCredential {
            control_key,
            cookie: String::new(),
            from_store: false,
        };
    }
    if let Some(stored) = load_opendesign_stored_key(&state.hermes_home) {
        return OpenDesignCredential {
            control_key: stored.control_key,
            cookie: String::new(),
            from_store: true,
        };
    }
    OpenDesignCredential {
        control_key: String::new(),
        cookie: provider_env_value(&state.hermes_home, "OPENDESIGN_COOKIE"),
        from_store: false,
    }
}

struct OpenDesignCredential {
    control_key: String,
    cookie: String,
    from_store: bool,
}

fn opendesign_workspace_id(state: &AppState) -> String {
    provider_env_value(&state.hermes_home, "OPENDESIGN_WORKSPACE_ID")
}

fn opendesign_cookie_header(raw: &str) -> String {
    raw.trim()
        .strip_prefix("Cookie:")
        .map(str::trim)
        .unwrap_or(raw.trim())
        .to_string()
}

/// Keep only the HTTP status from an upstream failure so no response body or
/// credential echo reaches the card or logs.
fn opendesign_safe_error(err: &str) -> String {
    let head: Vec<&str> = err.split_whitespace().take(2).collect();
    if head.first().is_some_and(|first| *first == "HTTP") {
        return head.join(" ");
    }
    "请求失败".into()
}

fn opendesign_percent(used: f64, limit: f64) -> Option<String> {
    (limit > 0.0 && used.is_finite() && limit.is_finite())
        .then(|| format!("{:.1}%", (used / limit * 100.0).min(100.0)))
}

fn opendesign_window_label(duration_seconds: f64) -> Option<&'static str> {
    match duration_seconds as i64 {
        18_000 => Some("5h额度"),
        604_800 => Some("周额度"),
        _ => None,
    }
}

fn opendesign_iso_seconds(value: Option<&Value>) -> Option<i64> {
    let raw = value?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|stamp| stamp.timestamp())
}

fn opendesign_quota_windows(
    payload: &Value,
) -> Result<Vec<ProviderUsageWindow>, String> {
    let plan = payload.get("codingPlan").unwrap_or(payload);
    if plan
        .get("eligible")
        .and_then(Value::as_bool)
        .is_some_and(|eligible| !eligible)
    {
        return Err("当前账户没有生效的 Design Plan".into());
    }
    let windows = plan
        .get("windows")
        .and_then(Value::as_array)
        .ok_or_else(|| "返回内容缺少额度窗口".to_string())?;
    let mut out = Vec::new();
    for window in windows {
        let duration = window.get("durationSeconds").and_then(provider_number);
        let Some(label) = duration.and_then(opendesign_window_label) else {
            continue;
        };
        let used = window.get("usedCredits").and_then(provider_number);
        let limit = window.get("limitCredits").and_then(provider_number);
        let (Some(used), Some(limit)) = (used, limit) else {
            return Err(format!("{label} 缺少有效用量"));
        };
        let percent = opendesign_percent(used, limit)
            .ok_or_else(|| format!("{label} 额度无效"))?;
        let reset_at = opendesign_iso_seconds(window.get("resetsAt"));
        out.push(ProviderUsageWindow {
            window: label.into(),
            used: Some(percent),
            reset: reset_at.map(provider_reset_text_local),
            reset_at,
        });
    }
    if out.is_empty() {
        return Err("当前账户没有生效的 Design Plan".into());
    }
    out.sort_by_key(|window| match window.window.as_str() {
        "5h额度" => 0,
        _ => 1,
    });
    Ok(out)
}

fn opendesign_balance_text(payload: &Value) -> Option<String> {
    let raw = payload.get("balanceUsd")?.as_str()?.trim().to_string();
    let balance = raw.parse::<f64>().ok().filter(|value| value.is_finite())?;
    Some(format!("余额 **{}**", fmt_provider_money(balance, '$')))
}

async fn opendesign_cookie_request(
    state: &AppState,
    url: &str,
    cookie: &str,
    workspace_id: &str,
) -> Result<Value, String> {
    let mut headers = vec![("Accept".to_string(), "application/json".to_string())];
    if !cookie.is_empty() {
        headers.push(("Cookie".into(), cookie.to_string()));
    }
    if !workspace_id.is_empty() {
        headers.push(("x-vela-workspace-id".into(), workspace_id.to_string()));
    }
    provider_http_get_json(&state.client, url, &headers).await
}

async fn opendesign_control_request(
    state: &AppState,
    url: &str,
    control_key: &str,
    workspace_id: &str,
) -> Result<Value, String> {
    let headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Authorization".into(), format!("Bearer {control_key}")),
        ("x-vela-workspace-id".into(), workspace_id.to_string()),
    ];
    provider_http_get_json(&state.client, url, &headers).await
}

async fn opendesign_resolve_workspace(
    state: &AppState,
    base: &str,
    credential: &OpenDesignCredential,
    cookie: &str,
    configured: &str,
) -> Result<String, String> {
    if !configured.is_empty() {
        return Ok(configured.to_string());
    }
    let url = format!("{base}/api/v1/workspaces/current");
    let payload = if credential.control_key.is_empty() {
        opendesign_cookie_request(state, &url, cookie, "").await?
    } else {
        opendesign_control_request(state, &url, &credential.control_key, "").await?
    };
    let active = payload
        .get("activeWorkspaceId")
        .and_then(Value::as_str)
        .or_else(|| payload.get("workspaceId").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    active
        .map(str::to_string)
        .ok_or_else(|| "未找到当前工作区".to_string())
}

async fn fetch_opendesign_usage_from_base(state: &AppState, base: &str) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "opendesign".into(),
        title: "OpenDesign 用量".into(),
        ..Default::default()
    };
    let credential = opendesign_credential(state);
    if credential.control_key.is_empty() && credential.cookie.is_empty() {
        section.errors.push(
            "缺少 OPENDESIGN_COOKIE 或 OPENDESIGN_CONTROL_KEY(推理 API key 无法查询额度)".into(),
        );
        return section;
    }
    let cookie = opendesign_cookie_header(&credential.cookie);
    let control_mode = !credential.control_key.is_empty();
    let workspace = match opendesign_resolve_workspace(
        state,
        base,
        &credential,
        &cookie,
        &opendesign_workspace_id(state),
    )
    .await
    {
        Ok(workspace) => workspace,
        Err(err) => {
            section
                .errors
                .push(format!("工作区查询失败：{}", opendesign_safe_error(&err)));
            return section;
        }
    };
    let workspace_encoded = urlencoding_lite(&workspace);
    let (quota_url, wallet_url) = if control_mode {
        (
            format!("{base}/api/v1/workspaces/{workspace_encoded}/billing/preflight"),
            format!("{base}/api/v1/workspaces/{workspace_encoded}/wallet/balance"),
        )
    } else {
        (
            format!("{base}/api/v1/workspaces/{workspace_encoded}/billing/coding-plan-usage"),
            format!("{base}/api/v1/workspaces/{workspace_encoded}/wallet/balance"),
        )
    };
    let quota = if control_mode {
        opendesign_control_request(state, &quota_url, &credential.control_key, &workspace).await
    } else {
        opendesign_cookie_request(state, &quota_url, &cookie, &workspace).await
    };
    match quota {
        Ok(payload) => match opendesign_quota_windows(&payload) {
            Ok(windows) => section.windows = windows,
            Err(err) => section.errors.push(err),
        },
        Err(err) => {
            let safe = opendesign_safe_error(&err);
            if credential.from_store && matches!(safe.as_str(), "HTTP 401" | "HTTP 403") {
                section
                    .errors
                    .push("OpenDesign 授权已失效，请重新连接".into());
            } else {
                section.errors.push(format!("额度查询失败：{safe}"));
            }
        }
    }
    let wallet = if control_mode {
        opendesign_control_request(state, &wallet_url, &credential.control_key, &workspace).await
    } else {
        opendesign_cookie_request(state, &wallet_url, &cookie, &workspace).await
    };
    match wallet {
        Ok(payload) => {
            if payload
                .get("workspaceId")
                .and_then(Value::as_str)
                .is_some_and(|id| id != workspace)
            {
                section.errors.push("钱包响应对应的工作区不一致".into());
            } else if let Some(text) = opendesign_balance_text(&payload) {
                section.description = text;
            }
        }
        Err(err) => section.errors.push(format!("钱包查询失败：{}", opendesign_safe_error(&err))),
    }
    if !section.windows.is_empty() {
        let _ = provider_capture_quota_daily(state, &mut section, chrono::Utc::now().timestamp());
    }
    section
}

async fn fetch_opendesign_usage(state: &AppState) -> ProviderUsageSection {
    let base = opendesign_api_base(&state.hermes_home);
    fetch_opendesign_usage_from_base(state, &base).await
}
