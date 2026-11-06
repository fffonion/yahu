// WorkBuddy (Tencent CodeBuddy) credit quota.
//
// Verified against the desktop client's billing endpoint:
// - POST {base}/v2/billing/meter/get-user-resource with an empty JSON body
//   returns data.Response.Data.Accounts[], one row per purchased credit pack
//   (PackageName, CapacitySizePrecise/CapacityUsedPrecise in credits, and a
//   CycleEndTime in China Standard Time).
// - GET on the same path is a 404; the enterprise route
//   (/v2/billing/meter/get-enterprise-resource) rejects personal tokens.
// - Accounts live in ~/.hermes/auth.json credential_pool.workbuddy with a ~30
//   day OAuth access token. Yahu only reads that pool: it never refreshes or
//   rewrites Hermes auth state, so an expired token is reported, not renewed.

const WORKBUDDY_BILLING_PATH: &str = "/v2/billing/meter/get-user-resource";
const WORKBUDDY_API_BASE: &str = "https://copilot.tencent.com";
const WORKBUDDY_DEFAULT_DOMAIN: &str = "www.codebuddy.cn";
const WORKBUDDY_ORIGIN: &str = "https://www.codebuddy.cn";

struct WorkbuddyAccount {
    label: String,
    access_token: String,
    uid: String,
    domain: String,
    expires_at_ms: Option<i64>,
}

fn workbuddy_accounts(hermes_home: &Path) -> Vec<WorkbuddyAccount> {
    let path = hermes_home.join("auth.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(entries) = value
        .get("credential_pool")
        .and_then(|pool| pool.get("workbuddy"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut accounts: Vec<WorkbuddyAccount> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let access_token = entry
            .get("access_token")
            .or_else(|| entry.get("api_key"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if access_token.is_empty() {
            continue;
        }
        let label = ["nickname", "label"]
            .iter()
            .find_map(|key| {
                entry
                    .get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("账号{}", index + 1));
        let mut unique = label.clone();
        let mut suffix = 2;
        while accounts.iter().any(|account| account.label == unique) {
            unique = format!("{label} ({suffix})");
            suffix += 1;
        }
        let domain = entry
            .get("domain")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or(WORKBUDDY_DEFAULT_DOMAIN)
            .to_string();
        accounts.push(WorkbuddyAccount {
            label: unique,
            access_token,
            uid: json_string_or_number(entry.get("uid")).unwrap_or_default(),
            domain,
            expires_at_ms: entry.get("expires_at_ms").and_then(provider_number).map(|value| value as i64),
        });
    }
    accounts.sort_by_key(|account| account.label.to_lowercase());
    accounts
}

fn workbuddy_safe_error(err: &str) -> String {
    let head: Vec<&str> = err.split_whitespace().take(2).collect();
    if head.first().is_some_and(|first| *first == "HTTP") {
        return head.join(" ");
    }
    "请求失败".into()
}

fn workbuddy_credit_text(value: f64) -> String {
    let mut text = format!("{:.2}", value.max(0.0));
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn workbuddy_package_label(name: &str) -> String {
    let trimmed = name.trim();
    let shortened = trimmed.strip_prefix("CodeBuddy").unwrap_or(trimmed).trim();
    if shortened.is_empty() {
        trimmed.to_string()
    } else {
        shortened.to_string()
    }
}

/// `CycleEndTime` is a bare "YYYY-MM-DD HH:MM:SS" China Standard Time stamp.
fn workbuddy_cycle_end(text: &str) -> Option<i64> {
    let naive = chrono::NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
    let offset = chrono::FixedOffset::east_opt(8 * 3600)?;
    naive.and_local_timezone(offset).single().map(|stamp| stamp.timestamp())
}

fn workbuddy_windows(
    payload: &Value,
    account_prefix: Option<&str>,
) -> Result<Vec<ProviderUsageWindow>, String> {
    let accounts = payload
        .get("data")
        .and_then(|data| data.get("Response"))
        .and_then(|response| response.get("Data"))
        .and_then(|data| data.get("Accounts"))
        .and_then(Value::as_array)
        .ok_or_else(|| "返回内容缺少额度包".to_string())?;
    let mut windows = Vec::new();
    for pack in accounts {
        let package = pack
            .get("PackageName")
            .and_then(Value::as_str)
            .map(workbuddy_package_label)
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| "额度包".to_string());
        let size = pack
            .get("CapacitySizePrecise")
            .and_then(provider_number)
            .or_else(|| pack.get("CycleCapacitySizePrecise").and_then(provider_number))
            .or_else(|| pack.get("CapacitySize").and_then(provider_number));
        let used = pack
            .get("ActualCycleCapacityUsed")
            .and_then(provider_number)
            .or_else(|| pack.get("CapacityUsedPrecise").and_then(provider_number))
            .or_else(|| pack.get("CapacityUsed").and_then(provider_number));
        let (Some(size), Some(used)) = (size, used) else {
            return Err(format!("{package} 缺少有效额度"));
        };
        // Empty packs carry no signal on the card.
        if size <= 0.0 {
            continue;
        }
        let reset_at = pack
            .get("CycleEndTime")
            .and_then(Value::as_str)
            .and_then(workbuddy_cycle_end);
        windows.push(ProviderUsageWindow {
            window: match account_prefix {
                Some(prefix) => format!("{prefix} · {package}"),
                None => package,
            },
            used: Some(format!("{} / {}", workbuddy_credit_text(used), workbuddy_credit_text(size))),
            reset: reset_at.map(provider_reset_text_local),
            reset_at,
        });
    }
    if windows.is_empty() {
        return Err("当前账号没有可用额度包".to_string());
    }
    windows.sort_by_key(|window| window.reset_at.unwrap_or(i64::MAX));
    Ok(windows)
}

async fn workbuddy_quota_request(
    state: &AppState,
    base: &str,
    account: &WorkbuddyAccount,
) -> Result<Value, String> {
    let mut headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Authorization".to_string(), format!("Bearer {}", account.access_token)),
        ("X-No-Enterprise-Id".to_string(), "1".to_string()),
        ("X-Domain".to_string(), account.domain.clone()),
        ("X-CodeBuddy-Request".to_string(), "1".to_string()),
        ("X-Requested-With".to_string(), "XMLHttpRequest".to_string()),
        ("Origin".to_string(), WORKBUDDY_ORIGIN.to_string()),
        ("Referer".to_string(), format!("{WORKBUDDY_ORIGIN}/")),
        ("Accept-Language".to_string(), "zh-CN".to_string()),
    ];
    if !account.uid.is_empty() {
        headers.push(("X-User-Id".to_string(), account.uid.clone()));
    }
    let url = format!("{}{WORKBUDDY_BILLING_PATH}", base.trim_end_matches('/'));
    let payload =
        provider_http_post_json(&state.client, &url, serde_json::json!({}), &headers).await?;
    match payload.get("code").and_then(provider_number) {
        Some(code) if code != 0.0 => {
            let message = payload.get("msg").and_then(Value::as_str).unwrap_or("上游返回错误").trim();
            Err(format!("HTTP 200 {message}"))
        }
        _ => Ok(payload),
    }
}

async fn fetch_workbuddy_usage_from_base(state: &AppState, base: &str) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "workbuddy".into(),
        title: "WorkBuddy 用量".into(),
        ..Default::default()
    };
    let accounts = workbuddy_accounts(&state.hermes_home);
    if accounts.is_empty() {
        section
            .errors
            .push("缺少 auth.json credential_pool.workbuddy；用 Hermes 的 WorkBuddy 登录流程生成".into());
        return section;
    }
    let multi_account = accounts.len() > 1;
    let now_ms = chrono::Utc::now().timestamp_millis();
    for account in accounts {
        let scoped = |message: String| {
            if multi_account {
                format!("{}：{message}", account.label)
            } else {
                message
            }
        };
        if account.expires_at_ms.is_some_and(|expires| expires > 0 && expires < now_ms) {
            section
                .errors
                .push(scoped("凭据已过期，请在 Hermes 侧重新登录 workbuddy".into()));
            continue;
        }
        match workbuddy_quota_request(state, base, &account).await {
            Ok(payload) => {
                match workbuddy_windows(&payload, multi_account.then_some(account.label.as_str())) {
                    Ok(windows) => section.windows.extend(windows),
                    Err(err) => section.errors.push(scoped(err)),
                }
            }
            Err(err) => {
                let safe = workbuddy_safe_error(&err);
                let message = if safe == "HTTP 401" || safe == "HTTP 403" {
                    "授权已失效，请在 Hermes 侧重新登录 workbuddy".to_string()
                } else {
                    format!("额度查询失败：{safe}")
                };
                section.errors.push(scoped(message));
            }
        }
    }
    section
}

async fn fetch_workbuddy_usage(state: &AppState) -> ProviderUsageSection {
    fetch_workbuddy_usage_from_base(state, WORKBUDDY_API_BASE).await
}
