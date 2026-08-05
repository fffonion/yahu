async fn models_cached(State(state): State<Arc<AppState>>) -> Response<Body> {
    const MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
    {
        let cache = state.model_cache.read().await;
        if let Some(body) = fresh_model_cache_body(&cache, MODEL_CACHE_TTL) {
            return Json(body).into_response();
        }
    }
    let disk_cache_path = state.hermes_home.join("cache/yahu/model-inventory.json");
    if let Some(body) = fresh_persisted_model_cache_body(&disk_cache_path, MODEL_CACHE_TTL, std::time::SystemTime::now()) {
        return Json(body).into_response();
    }

    let mut errors = Vec::new();
    let body = match fetch_non_empty_model_payload(&state, fetch_api_server_models(&state), "api_server").await {
        Ok(body) => body,
        Err(err) => {
            errors.push(format!("api_server: {err}"));
            match fetch_non_empty_model_payload(&state, load_hermes_model_inventory(&state), "hermes_inventory").await {
                Ok(body) => body,
                Err(err) => {
                    errors.push(format!("inventory: {err}"));
                    return json_error(
                        StatusCode::BAD_GATEWAY,
                        &format!("model list unavailable: {}", errors.join("; ")),
                    );
                }
            }
        }
    };

    let mut cache = state.model_cache.write().await;
    cache.fetched_at = Some(std::time::Instant::now());
    cache.body = Some(body.clone());
    if let Err(err) = persist_model_cache_body(&disk_cache_path, &body, std::time::SystemTime::now()) {
        warn!("failed to persist model inventory cache: {err}");
    }
    Json(body).into_response()
}

async fn fetch_non_empty_model_payload<F>(
    _state: &AppState,
    fetch: F,
    source: &str,
) -> anyhow::Result<serde_json::Value>
where
    F: std::future::Future<Output = anyhow::Result<serde_json::Value>>,
{
    let raw = fetch.await?;
    let payload = model_cache_payload_from_source(&raw, source);
    if !yet_another_hermes_ui::model_cache_payload_has_models(&payload) {
        anyhow::bail!("no non-placeholder models returned");
    }
    Ok(payload)
}

async fn fetch_api_server_models(state: &AppState) -> anyhow::Result<serde_json::Value> {
    let mut req = state.client.get(format!("{}/v1/models", state.api_url));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("model list request failed: {}", resp.status());
    }
    Ok(resp.json::<serde_json::Value>().await?)
}

async fn load_hermes_model_inventory(state: &AppState) -> anyhow::Result<serde_json::Value> {
    let agent_dir = hermes_agent_dir(state);
    if !agent_dir.join("hermes_cli/inventory.py").exists() {
        anyhow::bail!("Hermes agent source not found at {}", agent_dir.display());
    }
    let python = hermes_python_command(&agent_dir);
    let script = r#"
import json, os, sys
agent_dir = os.environ.get('HERMES_AGENT_DIR')
sys.path.insert(0, agent_dir)
from hermes_cli.inventory import build_models_payload, load_picker_context
from agent.model_metadata import get_model_context_length
payload = build_models_payload(load_picker_context(), max_models=80, pricing=True, capabilities=True)
for provider in payload.get('providers', []):
    provider_id = provider.get('slug') or provider.get('provider') or provider.get('id') or provider.get('name') or ''
    caps = provider.setdefault('capabilities', {})
    for model_id in provider.get('models', []):
        if not isinstance(model_id, str):
            continue
        model_caps = caps.setdefault(model_id, {})
        try:
            context_length = get_model_context_length(model_id, provider=provider_id or None)
        except Exception:
            context_length = None
        if context_length:
            model_caps['context_length'] = int(context_length)
print(json.dumps(payload))
"#;
    let output = timeout(
        Duration::from_secs(45),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(
            "inventory command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn hermes_agent_dir(state: &AppState) -> PathBuf {
    env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"))
}

fn hermes_python_command(agent_dir: &Path) -> PathBuf {
    if let Ok(python) = env::var("HERMES_WEBUI_PYTHON") {
        let trimmed = python.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let venv_python = agent_dir.join("venv/bin/python3");
    if venv_python.is_file() {
        return venv_python;
    }
    PathBuf::from("python3")
}
