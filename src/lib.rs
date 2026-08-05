use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, ErrorKind},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct ModelCache {
    pub fetched_at: Option<Instant>,
    pub body: Option<Value>,
}

#[derive(Deserialize, Serialize)]
struct PersistedModelCache {
    saved_at_unix_seconds: u64,
    body: Value,
}

pub fn fresh_model_cache_body(cache: &ModelCache, ttl: Duration) -> Option<Value> {
    match (cache.fetched_at, cache.body.as_ref()) {
        (Some(fetched_at), Some(body)) if fetched_at.elapsed() < ttl => Some(body.clone()),
        _ => None,
    }
}

pub fn fresh_persisted_model_cache_body(
    path: &Path,
    ttl: Duration,
    now: SystemTime,
) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    let cache: PersistedModelCache = serde_json::from_slice(&bytes).ok()?;
    let saved_at = UNIX_EPOCH.checked_add(Duration::from_secs(cache.saved_at_unix_seconds))?;
    let age = now.duration_since(saved_at).ok()?;
    (age < ttl && model_cache_payload_has_models(&cache.body)).then_some(cache.body)
}

pub fn persist_model_cache_body(path: &Path, body: &Value, now: SystemTime) -> io::Result<()> {
    if !model_cache_payload_has_models(body) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "model cache has no usable models",
        ));
    }
    let saved_at_unix_seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidInput,
                "model cache timestamp predates Unix epoch",
            )
        })?
        .as_secs();
    let payload = serde_json::to_vec(&PersistedModelCache {
        saved_at_unix_seconds,
        body: body.clone(),
    })
    .map_err(io::Error::other)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "model cache path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut temporary_file = tempfile::NamedTempFile::new_in(parent)?;
    io::Write::write_all(&mut temporary_file, &payload)?;
    temporary_file
        .persist(path)
        .map(|_| ())
        .map_err(|err| err.error)
}

pub fn model_cache_payload_from_source(body: &Value, source: &str) -> Value {
    json!({
        "object": "list",
        "source": source,
        "data": flatten_model_options(body),
    })
}

pub fn model_cache_payload_has_models(body: &Value) -> bool {
    body.get("data")
        .and_then(Value::as_array)
        .map(|rows| !rows.is_empty())
        .unwrap_or(false)
}

pub fn flatten_model_options(body: &Value) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    if let Some(providers) = body.get("providers").and_then(Value::as_array) {
        for provider in providers {
            let provider_id = provider
                .get("slug")
                .or_else(|| provider.get("provider"))
                .or_else(|| provider.get("id"))
                .or_else(|| provider.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let provider_label = provider
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(provider_id);
            if let Some(models) = provider.get("models").and_then(Value::as_array) {
                for model in models {
                    if let Some(model_id) = model.as_str() {
                        let context_length = provider
                            .get("capabilities")
                            .and_then(|caps| caps.get(model_id))
                            .and_then(read_context_length);
                        push_model(
                            &mut out,
                            &mut seen,
                            model_id,
                            provider_id,
                            provider_label,
                            context_length,
                        );
                    }
                }
            }
        }
    }

    if let Some(data) = body.get("data").and_then(Value::as_array) {
        for row in data {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| row.as_str())
                .unwrap_or("");
            let provider = row.get("provider").and_then(Value::as_str).unwrap_or("");
            let label = row.get("label").and_then(Value::as_str).unwrap_or(id);
            push_model_with_label(
                &mut out,
                &mut seen,
                id,
                provider,
                label,
                read_context_length(row),
            );
        }
    }

    out
}

fn push_model(
    out: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<String>,
    model_id: &str,
    provider_id: &str,
    provider_label: &str,
    context_length: Option<i64>,
) {
    let label = if provider_label.is_empty() {
        model_id.to_string()
    } else {
        format!("{provider_label} · {model_id}")
    };
    push_model_with_label(out, seen, model_id, provider_id, &label, context_length);
}

fn read_context_length(row: &Value) -> Option<i64> {
    row.get("context_length")
        .or_else(|| row.get("context_window"))
        .or_else(|| row.get("contextWindow"))
        .or_else(|| row.get("limit").and_then(|limit| limit.get("context")))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

fn push_model_with_label(
    out: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<String>,
    model_id: &str,
    provider_id: &str,
    label: &str,
    context_length: Option<i64>,
) {
    let id = model_id.trim();
    if id.is_empty() || id == "hermes-agent" || !seen.insert(id.to_string()) {
        return;
    }
    let mut row = json!({
        "id": id,
        "object": "model",
        "owned_by": provider_id,
        "provider": provider_id,
        "label": label,
    });
    if let Some(context_length) = context_length {
        row["context_length"] = json!(context_length);
    }
    out.push(row);
}
