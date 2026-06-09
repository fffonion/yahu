async fn list_images(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ImageEntry>>, StatusCode> {
    let mut entries = list_image_entries(&state.image_dir).await?;
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(48).clamp(1, 120);
    if offset >= entries.len() {
        entries.clear();
    } else {
        entries = entries.into_iter().skip(offset).take(limit).collect();
    }
    Ok(Json(entries))
}

async fn refresh_images(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<RefreshResult>, StatusCode> {
    let after = query.after.unwrap_or(0);
    let limit = query.limit.unwrap_or(48).clamp(1, 120);
    let check_names = parse_check_names(query.check.as_deref());
    let mut new_items = Vec::new();
    let mut checked_items = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&state.image_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        if !is_source_image(&path) {
            continue;
        }
        let Some(filename) = safe_file_name_from_path(&path) else {
            continue;
        };
        let modified_at = tokio::fs::symlink_metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .map(system_time_to_millis)
            .unwrap_or(0);
        if modified_at > after {
            if let Some(image_entry) = image_entry_for_png(&state.image_dir, &path).await {
                new_items.push(image_entry);
            }
        } else if check_names.contains(&filename)
            && let Some(image_entry) = image_entry_for_png(&state.image_dir, &path).await
        {
            checked_items.push(image_entry);
        }
    }

    new_items.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    if new_items.len() > limit {
        new_items.truncate(limit);
    }
    checked_items.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    Ok(Json(RefreshResult {
        new_items,
        checked_items,
    }))
}

fn parse_check_names(input: Option<&str>) -> HashSet<String> {
    input
        .unwrap_or("")
        .split(',')
        .filter_map(|name| {
            let trimmed = name.trim();
            (!trimmed.is_empty() && is_safe_filename(trimmed)).then(|| trimmed.to_string())
        })
        .take(240)
        .collect()
}

async fn image_stats(State(state): State<Arc<AppState>>) -> Result<Json<ImageStats>, StatusCode> {
    Ok(Json(compute_image_stats(&state.image_dir).await?))
}

async fn image_entry(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageEntry>, StatusCode> {
    let path = resolve_file(
        &state.image_dir,
        &filename,
        &["png", "jpg", "jpeg", "heic", "webp"],
    )?;
    let png_path =
        png_path_for_related_file(&state.image_dir, &path).ok_or(StatusCode::NOT_FOUND)?;
    let entry = image_entry_for_png(&state.image_dir, &png_path)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(entry))
}

async fn image_metadata(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageMetadata>, StatusCode> {
    let path = resolve_file(
        &state.image_dir,
        &filename,
        &["png", "jpg", "jpeg", "heic", "webp"],
    )?;
    let png_path =
        png_path_for_related_file(&state.image_dir, &path).ok_or(StatusCode::NOT_FOUND)?;
    if !is_regular_file(&png_path) {
        return Err(StatusCode::NOT_FOUND);
    }
    let png_name = safe_file_name_from_path(&png_path).ok_or(StatusCode::BAD_REQUEST)?;
    let png = file_metadata(&state.image_dir, &png_name, false).ok_or(StatusCode::NOT_FOUND)?;
    let webp = find_webp_for_png(&state.image_dir, &png_path)
        .await
        .and_then(|name| file_metadata(&state.image_dir, &name, false));
    let heic = find_heic_for_png(&state.image_dir, &png_path)
        .await
        .and_then(|name| file_metadata(&state.image_dir, &name, true));
    let heic_status = heic_status_for_source(&png_path, png.size, heic.is_some()).to_string();
    let bytes = tokio::fs::read(&png_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (dimensions, png_text) = parse_image_info(&png_path, &bytes);
    Ok(Json(ImageMetadata {
        filename: png_name,
        dimensions,
        png,
        webp,
        heic,
        heic_status,
        png_text,
    }))
}

fn file_metadata(dir: &Path, filename: &str, download: bool) -> Option<FileMetadata> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = dir.join(filename);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    let segment = path_segment(filename);
    let url = if download {
        format!("/image-download/{}?v={}", segment, file_version(&metadata))
    } else {
        format!("/image-files/{}?v={}", segment, file_version(&metadata))
    };
    Some(FileMetadata {
        filename: filename.to_string(),
        url,
        size: metadata.len(),
        modified_at,
    })
}

fn source_can_generate_heic(path: &Path, source_size: u64) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some(ext) if ext.eq_ignore_ascii_case("png")
    ) && source_size > HEIC_GENERATION_MIN_BYTES
}

fn heic_status_for_source(path: &Path, source_size: u64, has_heic: bool) -> &'static str {
    if has_heic {
        return "available";
    }
    if source_can_generate_heic(path, source_size) {
        "missing"
    } else {
        "not_applicable"
    }
}

fn source_download_label(filename: &str) -> &'static str {
    match Path::new(filename).extension().and_then(OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") => {
            "Download JPEG"
        }
        Some(ext) if ext.eq_ignore_ascii_case("webp") => "Download WebP",
        _ => "Download PNG",
    }
}
