async fn proxy_hermes(
    State(state): State<Arc<AppState>>,
    AxumPath(path): AxumPath<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response<Body> {
    let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();
    let url = format!("{}/{}{}", state.api_url, path, query);
    let stream_session_id = proxied_chat_stream_session_id(&path, &method);
    let req_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let bytes = match to_bytes(body, MAX_PROXY_BODY).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &format!("cannot read body: {err}"));
        }
    };
    if let Some(session_id) = &stream_session_id {
        begin_chat_stream_snapshot(&state, session_id).await;
        if let Some(input) = chat_stream_request_input_text(&bytes) {
            publish_chat_stream_message(
                &state,
                session_id,
                serde_json::json!({
                    "id": format!("user_{}", chat_stream_run_id()),
                    "role": "user",
                    "content": input,
                    "timestamp": unix_now_seconds(),
                }),
            )
            .await;
        }
    }
    let mut builder = state.client.request(req_method, url).body(bytes);
    for (key, value) in headers.iter() {
        let name = key.as_str().to_ascii_lowercase();
        if !should_forward_proxy_header(&name) {
            continue;
        }
        builder = builder.header(key.as_str(), value.as_bytes());
    }
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        builder = builder.bearer_auth(key);
    }
    match builder.send().await {
        Ok(resp) => response_from_reqwest(state.clone(), stream_session_id, resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API proxy failed: {err}"),
        ),
    }
}

async fn chat_stream(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    headers: HeaderMap,
    body: Body,
) -> Response<Body> {
    let bytes = match to_bytes(body, MAX_PROXY_BODY).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &format!("cannot read body: {err}"));
        }
    };
    let body_value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &format!("cannot parse chat body: {err}"));
        }
    };

    if let Some(model_request) = chat_stream_model_switch_request_for_body(session_id.clone(), body_value.clone())
        && let Err(err) = send_model_switch_instruction(&state, &model_request).await
    {
        return json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API model switch failed: {err}"),
        );
    }

    begin_chat_stream_snapshot(&state, &session_id).await;
    if let Some(input) = chat_stream_input_text(body_value.get("input").unwrap_or(&serde_json::Value::Null))
        .filter(|text| !text.is_empty())
    {
        publish_chat_stream_message(
            &state,
            &session_id,
            serde_json::json!({
                "id": format!("user_{}", chat_stream_run_id()),
                "role": "user",
                "content": input,
                "timestamp": unix_now_seconds(),
            }),
        )
        .await;
    }

    let run_body = match chat_stream_run_body(&state, &session_id, &body_value).await {
        Ok(value) => value,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
    };
    let run_url = format!("{}/v1/runs", state.api_url);
    let mut run_builder = state.client.post(run_url).json(&run_body);
    for (key, value) in headers.iter() {
        let name = key.as_str().to_ascii_lowercase();
        if !should_forward_proxy_header(&name) {
            continue;
        }
        run_builder = run_builder.header(key.as_str(), value.as_bytes());
    }
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        run_builder = run_builder.bearer_auth(key);
    }
    let run_resp = match run_builder.send().await {
        Ok(resp) => resp,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Hermes API run start failed: {err}"),
            );
        }
    };
    if !run_resp.status().is_success() {
        let status = run_resp.status();
        let text = run_resp.text().await.unwrap_or_default();
        return json_error(StatusCode::BAD_GATEWAY, &format!("Hermes API run start failed: {status}: {text}"));
    }
    let run_json = match run_resp.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(err) => return json_error(StatusCode::BAD_GATEWAY, &format!("cannot parse run start response: {err}")),
    };
    let Some(run_id) = run_json.get("run_id").and_then(|value| value.as_str()).map(str::to_string) else {
        return json_error(StatusCode::BAD_GATEWAY, "Hermes API run start response missing run_id");
    };
    state
        .active_chat_run_ids
        .write()
        .await
        .insert(session_id.clone(), run_id.clone());

    let events_url = format!("{}/v1/runs/{}/events", state.api_url, path_segment(&run_id));
    let mut events_builder = state.client.get(events_url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        events_builder = events_builder.bearer_auth(key);
    }
    match events_builder.send().await {
        Ok(resp) => response_from_reqwest(state.clone(), Some(session_id), resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API run events failed: {err}"),
        ),
    }
}

async fn stop_chat_stream(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    let Some(run_id) = state.active_chat_run_ids.read().await.get(&session_id).cloned() else {
        return Json(serde_json::json!({"ok": true, "status": "not_running"})).into_response();
    };
    let url = format!("{}/v1/runs/{}/stop", state.api_url, path_segment(&run_id));
    let mut req = state.client.post(url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND => {
            state.active_chat_run_ids.write().await.remove(&session_id);
            Json(serde_json::json!({"ok": true, "run_id": run_id, "status": "stopping"})).into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            json_error(StatusCode::BAD_GATEWAY, &format!("Hermes API run stop failed: {status}: {text}"))
        }
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &format!("Hermes API run stop failed: {err}")),
    }
}

async fn chat_stream_run_body(
    state: &Arc<AppState>,
    session_id: &str,
    source_body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut body = chat_stream_actual_body(source_body)?;
    if let Some(map) = body.as_object_mut() {
        if let Some(system_message) = map.remove("system_message")
            && !map.contains_key("instructions")
        {
            map.insert("instructions".to_string(), system_message);
        }
        map.insert("session_id".to_string(), serde_json::Value::String(session_id.to_string()));
        let history = fetch_session_history_messages(state, session_id)
            .await
            .unwrap_or_default();
        let conversation_history = chat_stream_conversation_history(&history);
        if !conversation_history.is_empty() {
            map.insert("conversation_history".to_string(), serde_json::Value::Array(conversation_history));
        }
    }
    serde_json::to_vec(&body).map_err(|err| format!("cannot serialize run body: {err}"))?;
    Ok(body)
}

fn chat_stream_conversation_history(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter_map(|message| {
            let role = message.get("role").and_then(|value| value.as_str())?;
            if !matches!(role, "user" | "assistant") {
                return None;
            }
            let content = normalize_stream_content(message.get("content")?);
            if content.trim().is_empty() {
                return None;
            }
            Some(serde_json::json!({"role": role, "content": content}))
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChatStreamModelSwitchRequest {
    session_id: String,
    command: String,
    body: serde_json::Value,
}

fn chat_stream_model_switch_request_for_body(
    session_id: String,
    body: serde_json::Value,
) -> Option<ChatStreamModelSwitchRequest> {
    let model = body
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "hermes-agent")?;
    let provider = body
        .get("provider")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let command = if let Some(provider) = provider {
        format!("/model {model} --provider {provider} --session")
    } else {
        format!("/model {model} --session")
    };
    let mut switch_body = serde_json::json!({"input": command});
    if let Some(reasoning_effort) = body.get("reasoning_effort") {
        switch_body["reasoning_effort"] = reasoning_effort.clone();
    }
    Some(ChatStreamModelSwitchRequest { session_id, command, body: switch_body })
}

fn chat_stream_actual_body(source_body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut body = source_body.clone();
    if let Some(map) = body.as_object_mut() {
        map.remove("model");
        map.remove("provider");
    }
    serde_json::to_vec(&body).map_err(|err| format!("cannot serialize chat body: {err}"))?;
    Ok(body)
}

async fn send_model_switch_instruction(
    state: &Arc<AppState>,
    request: &ChatStreamModelSwitchRequest,
) -> Result<(), String> {
    let url = format!(
        "{}/api/sessions/{}/chat",
        state.api_url,
        path_segment(&request.session_id)
    );
    let mut builder = state.client.post(url).json(&request.body);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        builder = builder.bearer_auth(key);
    }
    let resp = builder.send().await.map_err(|err| err.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("{status}: {text}"))
    }
}

fn should_forward_proxy_header(name: &str) -> bool {
    !matches!(
        name,
        "host"
            | "cookie"
            | "authorization"
            | "origin"
            | "referer"
            | "connection"
            | "content-length"
            | "transfer-encoding"
    ) && !name.starts_with("sec-fetch-")
        && !name.starts_with("sec-ch-")
}

async fn response_from_reqwest(
    state: Arc<AppState>,
    stream_session_id: Option<String>,
    resp: reqwest::Response,
) -> Response<Body> {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut builder = Response::builder().status(status);
    for (key, value) in resp.headers() {
        if matches!(
            key.as_str(),
            "content-type" | "cache-control" | "x-hermes-session-id" | "x-hermes-session-key"
        ) {
            builder = builder.header(key, value);
        }
    }
    if status.is_success()
        && content_type.starts_with("text/event-stream")
        && let Some(session_id) = stream_session_id
    {
        return builder
            .body(Body::from_stream(chat_streaming_body(
                state,
                session_id,
                resp.bytes_stream(),
            )))
            .unwrap();
    }
    builder
        .body(Body::from_stream(resp.bytes_stream()))
        .unwrap()
}

fn proxied_chat_stream_session_id(path: &str, method: &Method) -> Option<String> {
    if method != Method::POST {
        return None;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() == 5
        && parts[0] == "api"
        && parts[1] == "sessions"
        && parts[3] == "chat"
        && parts[4] == "stream"
    {
        return Some(parts[2].to_string());
    }
    None
}

fn chat_stream_run_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn chat_stream_request_input_text(bytes: &[u8]) -> Option<String> {
    let body = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    chat_stream_input_text(body.get("input")?).filter(|text| !text.is_empty())
}

fn chat_stream_input_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.trim().to_string()),
        serde_json::Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .or_else(|| item.get("text").and_then(|value| value.as_str()).map(str::to_string))
                        .or_else(|| item.get("content").and_then(|value| value.as_str()).map(str::to_string))
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(text.trim().to_string())
        }
        serde_json::Value::Object(map) => map
            .get("text")
            .and_then(|value| value.as_str())
            .or_else(|| map.get("content").and_then(|value| value.as_str()))
            .map(|text| text.trim().to_string()),
        _ => None,
    }
}

fn parse_sse_block(block: &str) -> (String, String) {
    let mut event = String::from("message");
    let mut data = Vec::new();
    for line in block.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            event = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start().to_string());
        }
    }
    (event, data.join("\n"))
}

async fn begin_chat_stream_snapshot(state: &Arc<AppState>, session_id: &str) {
    state
        .active_chat_streams
        .write()
        .await
        .insert(session_id.to_string(), Vec::new());
}

async fn publish_chat_stream_message(
    state: &Arc<AppState>,
    session_id: &str,
    message: serde_json::Value,
) {
    {
        let mut active = state.active_chat_streams.write().await;
        let messages = active.entry(session_id.to_string()).or_default();
        if let Some(id) = message.get("id").and_then(|value| value.as_str()) {
            if let Some(existing) = messages
                .iter_mut()
                .find(|item| item.get("id").and_then(|value| value.as_str()) == Some(id))
            {
                *existing = message.clone();
            } else {
                messages.push(message.clone());
            }
        } else {
            messages.push(message.clone());
        }
    }
    let envelope = serde_json::json!({
        "session_id": session_id,
        "message": message,
    });
    let _ = state.chat_streams.send(envelope.to_string());
}

async fn clear_chat_stream_snapshot_later(state: Arc<AppState>, session_id: String) {
    sleep(Duration::from_secs(8)).await;
    state.active_chat_streams.write().await.remove(&session_id);
}

fn stream_payload_tool_name(payload: &serde_json::Value) -> String {
    ["tool_name", "name", "tool", "recipient_name"]
        .iter()
        .find_map(|key| payload.get(key).and_then(|value| value.as_str()))
        .unwrap_or("tool")
        .to_string()
}

fn chat_streaming_body(
    state: Arc<AppState>,
    session_id: String,
    upstream: impl futures_core::Stream<Item = Result<axum::body::Bytes, reqwest::Error>> + Send + 'static,
) -> impl futures_core::Stream<Item = Result<axum::body::Bytes, reqwest::Error>> {
    async_stream::stream! {
        let stream_id = chat_stream_run_id();
        let assistant_id = format!("assistant_{}", stream_id);
        let mut buffer = String::new();
        let mut final_text = String::new();
        let mut reasoning_text = String::new();
        futures_util::pin_mut!(upstream);
        while let Some(item) = futures_util::StreamExt::next(&mut upstream).await {
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(err) => {
                    yield Err(err);
                    continue;
                }
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = buffer.find("\n\n") {
                let block = buffer[..index].to_string();
                buffer = buffer[index + 2..].to_string();
                let (event, data) = parse_sse_block(&block);
                if data.is_empty() {
                    continue;
                }
                let payload = serde_json::from_str::<serde_json::Value>(&data).unwrap_or(serde_json::Value::Null);
                let effective_event = payload
                    .get("event")
                    .and_then(|value| value.as_str())
                    .unwrap_or(event.as_str())
                    .to_string();
                if let Some(run_id) = payload.get("run_id").and_then(|value| value.as_str()) {
                    state.active_chat_run_ids.write().await.insert(session_id.clone(), run_id.to_string());
                }
                let mut outbound_event = effective_event.clone();
                let mut outbound_payload = payload.clone();
                if effective_event == "message.delta" {
                    let delta = payload.get("delta").and_then(|value| value.as_str()).unwrap_or_default();
                    final_text.push_str(delta);
                    outbound_event = "assistant.delta".to_string();
                    outbound_payload = serde_json::json!({"session_id": session_id, "delta": delta});
                    publish_chat_stream_message(
                        &state,
                        &session_id,
                        serde_json::json!({
                            "id": assistant_id,
                            "role": "assistant",
                            "content": final_text,
                            "reasoning": reasoning_text,
                            "pending": true,
                            "timestamp": unix_now_seconds(),
                        }),
                    ).await;
                } else if effective_event == "assistant.delta" {
                    if let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) {
                        final_text.push_str(delta);
                    }
                    publish_chat_stream_message(
                        &state,
                        &session_id,
                        serde_json::json!({
                            "id": assistant_id,
                            "role": "assistant",
                            "content": final_text,
                            "reasoning": reasoning_text,
                            "pending": true,
                            "timestamp": unix_now_seconds(),
                        }),
                    ).await;
                } else if matches!(effective_event.as_str(), "reasoning.delta" | "assistant.reasoning.delta" | "thinking.delta" | "assistant.thinking.delta") {
                    let delta = payload
                        .get("delta")
                        .or_else(|| payload.get("text"))
                        .or_else(|| payload.get("content"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    reasoning_text.push_str(delta);
                    publish_chat_stream_message(
                        &state,
                        &session_id,
                        serde_json::json!({
                            "id": assistant_id,
                            "role": "assistant",
                            "content": final_text,
                            "reasoning": reasoning_text,
                            "pending": true,
                            "timestamp": unix_now_seconds(),
                        }),
                    ).await;
                } else if matches!(effective_event.as_str(), "tool.started" | "tool.completed" | "tool.progress") {
                    let tool_name = stream_payload_tool_name(&payload);
                    if let Some(map) = outbound_payload.as_object_mut() {
                        map.insert("tool_name".to_string(), serde_json::Value::String(tool_name.clone()));
                    }
                    publish_chat_stream_message(
                        &state,
                        &session_id,
                        serde_json::json!({
                            "id": format!("tool_{}_{}", stream_id, tool_name),
                            "role": "tool",
                            "tool_name": tool_name,
                            "content": serde_json::json!({
                                "status": effective_event,
                                "tool_name": stream_payload_tool_name(&payload),
                                "delta": payload.get("delta").cloned().unwrap_or(serde_json::Value::Null),
                                "payload": payload,
                            }).to_string(),
                            "timestamp": unix_now_seconds(),
                        }),
                    ).await;
                } else if effective_event == "assistant.completed" {
                    if let Some(content) = payload.get("content") {
                        final_text = normalize_stream_content(content);
                    }
                    publish_chat_stream_message(
                        &state,
                        &session_id,
                        serde_json::json!({
                            "id": assistant_id,
                            "role": "assistant",
                            "content": final_text,
                            "reasoning": reasoning_text,
                            "pending": false,
                            "timestamp": unix_now_seconds(),
                        }),
                    ).await;
                } else if matches!(effective_event.as_str(), "run.completed" | "done" | "run.cancelled" | "run.failed") {
                    if effective_event == "run.completed" {
                        if let Some(output) = payload.get("output").and_then(|value| value.as_str()).filter(|value| !value.is_empty()) {
                            final_text = output.to_string();
                        } else if let Some(content) = payload
                            .get("messages")
                            .and_then(|messages| messages.as_array())
                            .and_then(|messages| messages.iter().find(|message| message.get("role").and_then(|role| role.as_str()) == Some("assistant")))
                            .and_then(|message| message.get("content"))
                        {
                            let content_text = normalize_stream_content(content);
                            if !content_text.is_empty() {
                                final_text = content_text;
                            }
                        }
                        outbound_payload = serde_json::json!({
                            "session_id": session_id,
                            "messages": [{"role": "assistant", "content": final_text}],
                            "usage": payload.get("usage").cloned().unwrap_or(serde_json::Value::Null),
                        });
                        publish_chat_stream_message(
                            &state,
                            &session_id,
                            serde_json::json!({
                                "id": assistant_id,
                                "role": "assistant",
                                "content": final_text,
                                "reasoning": reasoning_text,
                                "pending": false,
                                "timestamp": unix_now_seconds(),
                            }),
                        ).await;
                    }
                    if matches!(effective_event.as_str(), "run.completed" | "done" | "run.cancelled" | "run.failed") {
                        state.active_chat_run_ids.write().await.remove(&session_id);
                    }
                    let state_for_clear = state.clone();
                    let session_for_clear = session_id.clone();
                    tokio::spawn(async move { clear_chat_stream_snapshot_later(state_for_clear, session_for_clear).await; });
                }
                let outbound_data = serde_json::to_string(&outbound_payload).unwrap_or_else(|_| "{}".to_string());
                yield Ok(axum::body::Bytes::from(format!("event: {}\ndata: {}\n\n", outbound_event, outbound_data)));
            }
        }
    }
}

fn normalize_stream_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .or_else(|| item.get("text").and_then(|value| value.as_str()).map(str::to_string))
                    .or_else(|| item.get("content").and_then(|value| value.as_str()).map(str::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
