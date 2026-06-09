async fn static_assets(uri: Uri) -> Response<Body> {
    let req_path = uri.path().trim_start_matches('/');
    let file_path = if req_path.is_empty() {
        "index.html"
    } else {
        req_path
    };
    let file = ASSETS
        .get_file(file_path)
        .or_else(|| ASSETS.get_file("index.html"));
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.contents().to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("asset not found"))
            .unwrap(),
    }
}
