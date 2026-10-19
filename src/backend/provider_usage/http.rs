use sha2::Digest;

const PROVIDER_USAGE_TIMEOUT: Duration = Duration::from_secs(15);
async fn provider_http_get_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<Value, String> {
    let mut request = client.get(url).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("HTTP {status} {snippet}"));
    }
    serde_json::from_str::<Value>(&body).map_err(|err| format!("响应不是 JSON：{err}"))
}

async fn provider_http_get_json_retry(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<Value, String> {
    match provider_http_get_json(client, url, headers).await {
        Ok(value) => Ok(value),
        Err(first_error) => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            provider_http_get_json(client, url, headers)
                .await
                .map_err(|second_error| format!("{first_error}; retry: {second_error}"))
        }
    }
}

async fn provider_http_post_json(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    headers: &[(String, String)],
) -> Result<Value, String> {
    let mut request = client.post(url).json(&body).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        let snippet: String = text.chars().take(160).collect();
        return Err(format!("HTTP {status} {snippet}"));
    }
    serde_json::from_str::<Value>(&text).map_err(|err| format!("响应不是 JSON：{err}"))
}

fn provider_grpc_status_error(status: &str) -> Option<String> {
    let Ok(code) = status.parse::<u32>() else {
        return Some("gRPC 响应包含无效状态".to_string());
    };
    if code == 0 {
        return None;
    }
    let detail = match code {
        4 => "上游额度查询超时",
        7 => "上游拒绝额度访问权限",
        8 => "上游额度查询限流",
        13 => "上游额度服务内部错误",
        14 => "上游额度服务暂不可用",
        16 => "上游额度查询认证失败",
        _ => "上游额度查询失败",
    };
    Some(format!("gRPC {code}：{detail}"))
}

async fn provider_http_post_bytes(
    client: &reqwest::Client,
    url: &str,
    body: &[u8],
    headers: &[(String, String)],
) -> Result<Vec<u8>, String> {
    let mut request = client
        .post(url)
        .body(body.to_vec())
        .timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    if let Some(status) = response.headers().get("grpc-status") {
        let status = status
            .to_str()
            .map_err(|_| "gRPC 响应包含无效状态".to_string())?;
        if let Some(error) = provider_grpc_status_error(status) {
            return Err(error);
        }
    }
    let body = response.bytes().await.map_err(|err| err.to_string())?;
    Ok(body.to_vec())
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Random 200-1500ms delay so concurrent account queries don't hit upstream
/// consoles at the same instant (mirrors the script's jitter).
async fn jitter_delay() {
    let millis = 200
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 % 1_300)
            .unwrap_or(0));
    sleep(Duration::from_millis(millis)).await;
}
