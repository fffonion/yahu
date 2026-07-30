async fn image_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_core::Stream<Item = Result<SseEvent, Infallible>>> {
    let mut rx_images = state.updates.subscribe();
    let mut rx_deletes = state.deletes.subscribe();
    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                Ok(text) = rx_images.recv() => {
                    yield Ok(SseEvent::default().data(text));
                }
                Ok(text) = rx_deletes.recv() => {
                    yield Ok(SseEvent::default().event("delete").data(text));
                }
                else => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list_image_entries(dir: &Path) -> Result<Vec<ImageEntry>, StatusCode> {
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir)
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
        if let Some(image_entry) = image_entry_for_png(dir, &path).await {
            entries.push(image_entry);
        }
    }

    // Sort by the PNG file's timestamp. `created_at` can be misleading after
    // copies/restores or filesystem moves, while `modified_at` tracks when the
    // preview PNG was actually written/updated.
    entries.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    Ok(entries)
}

async fn image_entry_for_png(dir: &Path, png_path: &Path) -> Option<ImageEntry> {
    if !is_source_image(png_path) {
        return None;
    }
    let metadata = tokio::fs::symlink_metadata(png_path).await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    let filename = safe_file_name_from_path(png_path)?;
    let heic_filename = find_heic_for_png(dir, png_path).await;
    let webp_filename = find_webp_for_png(dir, png_path).await;
    let display_filename = webp_filename.as_ref().unwrap_or(&filename);
    let created_at = system_time_to_millis(
        metadata
            .created()
            .unwrap_or_else(|_| metadata.modified().unwrap_or(UNIX_EPOCH)),
    );
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    let png_version = file_version(&metadata);
    let display_version = if display_filename == &filename {
        png_version.clone()
    } else {
        tokio::fs::symlink_metadata(dir.join(display_filename))
            .await
            .ok()
            .map(|m| file_version(&m))
            .unwrap_or_else(|| png_version.clone())
    };
    let heic_url = if let Some(name) = heic_filename.as_ref() {
        let version = tokio::fs::symlink_metadata(dir.join(name))
            .await
            .ok()
            .map(|m| file_version(&m))
            .unwrap_or_else(|| png_version.clone());
        Some(format!("/image-download/{}?v={}", path_segment(name), version))
    } else {
        None
    };
    let png_url = format!("/image-files/{}?v={}", path_segment(&filename), png_version);
    let heic_status = heic_status_for_source(png_path, heic_url.is_some()).to_string();
    let (download_filename, download_url, download_label) =
        if let (Some(name), Some(url)) = (heic_filename.as_ref(), heic_url.as_ref()) {
            (name.clone(), url.clone(), "Download HEIC".to_string())
        } else if heic_status == "not_applicable" {
            (
                filename.clone(),
                png_url.clone(),
                source_download_label(&filename).to_string(),
            )
        } else {
            (filename.clone(), png_url.clone(), "Generate HEIC".to_string())
        };
    Some(ImageEntry {
        image_url: format!("/image-files/{}?v={}", path_segment(display_filename), display_version),
        png_url,
        heic_url,
        heic_status,
        download_filename,
        download_url,
        download_label,
        filename,
        heic_filename,
        created_at,
        modified_at,
        size: metadata.len(),
    })
}
