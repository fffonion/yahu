async fn static_assets(uri: Uri) -> Response<Body> {
    let req_path = uri.path().trim_start_matches('/');
    let requested_path = if req_path.is_empty() {
        "index.html"
    } else {
        req_path
    };
    let resolved_path = if ASSETS.get_file(requested_path).is_some() {
        requested_path
    } else {
        "index.html"
    };
    let file = ASSETS.get_file(resolved_path);
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, static_asset_cache_control(resolved_path))
                .body(Body::from(file.contents().to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from("asset not found"))
            .unwrap(),
    }
}

fn static_asset_cache_control(path: &str) -> &'static str {
    if path == "index.html" || path == "sw.js" {
        "no-store"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    }
}
