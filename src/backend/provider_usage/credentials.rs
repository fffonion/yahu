fn provider_env_value(hermes_home: &Path, key: &str) -> String {
    if let Ok(value) = env::var(key)
        && !value.trim().is_empty()
    {
        return strip_env_quotes(value.trim());
    }
    let path = hermes_home.join(".env");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            return strip_env_quotes(v.trim());
        }
    }
    String::new()
}

fn first_provider_env_value(hermes_home: &Path, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| provider_env_value(hermes_home, key))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn strip_env_quotes(value: &str) -> String {
    let trimmed = value.trim();
    // .env values may be wrapped in matching quotes (shell style or JSON style).
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

/// Read a named custom provider's api_key from ~/.hermes/config.yaml.
fn config_provider_api_key(hermes_home: &Path, provider_name: &str) -> String {
    let path = hermes_home.join("config.yaml");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else {
        return String::new();
    };
    let Some(providers) = config
        .get("custom_providers")
        .and_then(|value| value.as_sequence())
    else {
        return String::new();
    };
    let entry = providers.iter().find(|entry| {
        entry
            .get("name")
            .and_then(|value| value.as_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(provider_name))
    });
    let Some(entry) = entry else {
        return String::new();
    };
    entry
        .get("api_key")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[derive(Clone)]
struct AuthCredentialEntry {
    label: String,
    access_token: String,
    refresh_token: Option<String>,
    user_id: Option<String>,
}

fn auth_json_credential_entries(
    hermes_home: &Path,
    provider: &str,
) -> Vec<AuthCredentialEntry> {
    let path = hermes_home.join("auth.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(entries) = value
        .get("credential_pool")
        .and_then(|pool| pool.get(provider))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut credentials: Vec<AuthCredentialEntry> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let access_token = entry
                .get("access_token")
                .or_else(|| entry.get("api_key"))
                .or_else(|| entry.get("token"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|token| !token.is_empty())?;
            let label = entry
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("账号{}", index + 1));
            let refresh_token = entry
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_string);
            let user_id = json_string_or_number(entry.get("user_id"));
            Some(AuthCredentialEntry {
                label,
                access_token: access_token.to_string(),
                refresh_token,
                user_id,
            })
        })
        .collect();
    credentials.sort_by_key(|entry| entry.label.to_lowercase());
    credentials
}

fn auth_json_credential_pool(
    hermes_home: &Path,
    provider: &str,
) -> Vec<(String, String)> {
    auth_json_credential_entries(hermes_home, provider)
        .into_iter()
        .map(|entry| (entry.label, entry.access_token))
        .collect()
}

fn any_provider_env_value(hermes_home: &Path, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| !provider_env_value(hermes_home, key).is_empty())
}

fn any_custom_provider_api_key(hermes_home: &Path, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| !config_provider_api_key(hermes_home, name).is_empty())
}

fn extract_cookie_value(cookie: &str, name: &str) -> String {
    cookie
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key.trim() == name).then(|| value.trim().trim_matches('"').to_string()))
        .unwrap_or_default()
}
