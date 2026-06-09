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
    let req_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let bytes = match to_bytes(body, MAX_PROXY_BODY).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &format!("cannot read body: {err}"));
        }
    };
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
        Ok(resp) => response_from_reqwest(resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API proxy failed: {err}"),
        ),
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

async fn response_from_reqwest(resp: reqwest::Response) -> Response<Body> {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (key, value) in resp.headers() {
        if matches!(
            key.as_str(),
            "content-type" | "cache-control" | "x-hermes-session-id" | "x-hermes-session-key"
        ) {
            builder = builder.header(key, value);
        }
    }
    builder
        .body(Body::from_stream(resp.bytes_stream()))
        .unwrap()
}
