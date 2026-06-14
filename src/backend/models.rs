async fn models_cached(State(state): State<Arc<AppState>>) -> Response<Body> {
    const MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
    {
        let cache = state.model_cache.read().await;
        if let Some(body) = fresh_model_cache_body(&cache, MODEL_CACHE_TTL) {
            return Json(body).into_response();
        }
    }

    let body = match fetch_api_server_models(&state).await {
        Ok(body) => model_cache_payload_from_source(&body, "api_server"),
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("model list unavailable from Hermes API Server: {err}"),
            );
        }
    };

    let mut cache = state.model_cache.write().await;
    cache.fetched_at = Some(std::time::Instant::now());
    cache.body = Some(body.clone());
    Json(body).into_response()
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
