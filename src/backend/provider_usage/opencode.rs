const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const OPENCODE_USAGE_PERMISSION_ERROR: &str = "OpenCode Go 用量查询返回 403：当前 OPENCODE_GO_API_KEY 无 Go Plan 用量权限，需要使用关联 Go 订阅、权限更高的 key。";

#[derive(Deserialize)]
struct OpenCodeGoUsageResponse {
    usage: OpenCodeGoUsageWindows,
}

#[derive(Deserialize)]
struct OpenCodeGoUsageWindows {
    rolling: OpenCodeGoUsageWindow,
    weekly: OpenCodeGoUsageWindow,
    monthly: OpenCodeGoUsageWindow,
}

#[derive(Deserialize)]
struct OpenCodeGoUsageWindow {
    #[serde(rename = "status")]
    _status: String,
    percent: f64,
    #[serde(rename = "resetsAt")]
    resets_at: String,
}

async fn opencode_go_usage_from_url(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<OpenCodeGoUsageWindows, String> {
    let response = client
        .get(url)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .header("Accept", "application/json")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("OpenCode Go 用量请求失败：{err}"))?;
    let status = response.status();
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(OPENCODE_USAGE_PERMISSION_ERROR.into());
    }
    if !status.is_success() {
        return Err(format!("OpenCode Go 用量 HTTP {status}"));
    }
    response
        .json::<OpenCodeGoUsageResponse>()
        .await
        .map(|payload| payload.usage)
        .map_err(|err| format!("OpenCode Go 用量响应无效：{err}"))
}

fn opencode_percent_text(percent: f64) -> String {
    let mut value = format!("{:.2}", percent.clamp(0.0, 100.0));
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    format!("{value}%")
}

fn opencode_go_window(
    label: &str,
    usage: OpenCodeGoUsageWindow,
) -> Result<ProviderUsageWindow, String> {
    let reset_at = chrono::DateTime::parse_from_rfc3339(&usage.resets_at)
        .map_err(|_| format!("OpenCode Go {label}重置时间无效"))?
        .timestamp();
    Ok(ProviderUsageWindow {
        window: label.into(),
        used: Some(opencode_percent_text(usage.percent)),
        reset: Some(provider_reset_duration(
            (reset_at - chrono::Utc::now().timestamp()).max(0) as f64,
        )),
        reset_at: Some(reset_at),
    })
}

async fn fetch_opencode_usage_from_url(state: &AppState, url: &str) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "opencode".into(),
        title: "OpenCode Go 用量".into(),
        ..Default::default()
    };
    let api_key = provider_env_value(&state.hermes_home, "OPENCODE_GO_API_KEY");
    if api_key.is_empty() {
        section.errors.push("缺少 OPENCODE_GO_API_KEY".into());
        return section;
    }

    match opencode_go_usage_from_url(&state.client, url, &api_key).await {
        Ok(usage) => {
            for result in [
                opencode_go_window("5h额度", usage.rolling),
                opencode_go_window("周额度", usage.weekly),
                opencode_go_window("月额度", usage.monthly),
            ] {
                match result {
                    Ok(window) => section.windows.push(window),
                    Err(error) => section.errors.push(error),
                }
            }
            section.description = "Go Plan 额度".into();
        }
        Err(error) => section.errors.push(error),
    }
    section
}

async fn fetch_opencode_usage(state: &AppState) -> ProviderUsageSection {
    fetch_opencode_usage_from_url(state, OPENCODE_GO_USAGE_URL).await
}
