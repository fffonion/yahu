async fn skills_list(State(state): State<Arc<AppState>>) -> Response<Body> {
    let url = format!("{}/v1/skills", state.api_url);
    let mut builder = state.client.request(reqwest::Method::GET, url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        builder = builder.bearer_auth(key);
    }
    match builder.send().await {
        Ok(resp) => response_from_reqwest(state, None, resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API skills request failed: {err}"),
        ),
    }
}

async fn skill_files(State(_state): State<Arc<AppState>>) -> Response<Body> {
    skill_api_unavailable("skill file listing")
}

async fn skill_file(State(_state): State<Arc<AppState>>) -> Response<Body> {
    skill_api_unavailable("skill file reading")
}

async fn skill_toggle(
    State(_state): State<Arc<AppState>>,
    AxumPath(_name): AxumPath<String>,
    Json(_payload): Json<serde_json::Value>,
) -> Response<Body> {
    skill_api_unavailable("skill toggling")
}

async fn skill_item_rename(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<WorkspaceRenamePayload>,
) -> Response<Body> {
    skill_api_unavailable("skill item rename")
}

async fn skill_item_delete(State(_state): State<Arc<AppState>>) -> Response<Body> {
    skill_api_unavailable("skill item delete")
}

async fn skill_delete(
    State(_state): State<Arc<AppState>>,
    AxumPath(_name): AxumPath<String>,
) -> Response<Body> {
    skill_api_unavailable("skill deletion")
}

fn skill_api_unavailable(action: &str) -> Response<Body> {
    json_error(
        StatusCode::NOT_IMPLEMENTED,
        &format!(
            "Hermes API Server does not expose {action}; yahu will not read or mutate local skill files"
        ),
    )
}
