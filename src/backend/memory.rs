async fn memory_get(State(state): State<Arc<AppState>>) -> Response<Body> {
    let mem_dir = state.hermes_home.join("memories");
    let memory = fs::read_to_string(mem_dir.join("MEMORY.md"))
        .await
        .unwrap_or_default();
    let user = fs::read_to_string(mem_dir.join("USER.md"))
        .await
        .unwrap_or_default();
    Json(MemoryResponse { memory, user }).into_response()
}

async fn memory_put(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MemoryPayload>,
) -> Response<Body> {
    let mem_dir = state.hermes_home.join("memories");
    if let Err(err) = fs::create_dir_all(&mem_dir).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create memory directory: {err}"),
        );
    }
    if let Err(err) = fs::write(mem_dir.join("MEMORY.md"), payload.memory).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write MEMORY.md: {err}"),
        );
    }
    if let Err(err) = fs::write(mem_dir.join("USER.md"), payload.user).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write USER.md: {err}"),
        );
    }
    Json(serde_json::json!({"status":"ok"})).into_response()
}
