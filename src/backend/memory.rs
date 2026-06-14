async fn memory_get(State(state): State<Arc<AppState>>) -> Response<Body> {
    proxy_memory_request(state, reqwest::Method::GET, None).await
}

async fn memory_put(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MemoryPayload>,
) -> Response<Body> {
    let body = match serde_json::to_vec(&payload) {
        Ok(body) => body,
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot encode memory payload: {err}"),
            );
        }
    };
    proxy_memory_request(state, reqwest::Method::PUT, Some(body)).await
}

async fn proxy_memory_request(
    state: Arc<AppState>,
    method: reqwest::Method,
    body: Option<Vec<u8>>,
) -> Response<Body> {
    let url = format!("{}/api/memory", state.api_url);
    let mut builder = state.client.request(method, url);
    if let Some(body) = body {
        builder = builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
    }
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        builder = builder.bearer_auth(key);
    }
    match builder.send().await {
        Ok(resp) => response_from_reqwest(state, None, resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API memory request failed: {err}"),
        ),
    }
}
