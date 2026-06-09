async fn delete_image(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<BatchResult>, StatusCode> {
    let dir = &state.image_dir;
    let mut errors = Vec::new();
    let target = resolve_file_or_not(dir, &filename, &["png", "jpg", "jpeg", "heic", "webp"]);
    let mut success = false;
    let mut event_filename = filename.clone();
    if let Ok(path) = target {
        event_filename = delete_event_filename(dir, &filename, &path);
        let related = related_files_for_image(dir, &path).await;
        if related.is_empty() {
            errors.push(format!("file not found: {}", filename));
        }
        for file in related {
            match tokio::fs::remove_file(&file).await {
                Ok(_) => success = true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => errors.push(format!("delete {}: {}", file.display(), e)),
            }
        }
    } else {
        errors.push(format!("file not found: {}", filename));
    }
    if success {
        let msg = serde_json::json!({"filename": event_filename}).to_string();
        let _ = state.deletes.send(msg);
    }
    Ok(Json(BatchResult {
        success_count: if success { 1 } else { 0 },
        fail_count: errors.len(),
        errors,
        download_pngs: None,
    }))
}

fn resolve_file_or_not(dir: &Path, filename: &str, allowed_exts: &[&str]) -> Result<PathBuf, ()> {
    let decoded = percent_decode_str(filename)
        .decode_utf8()
        .map_err(|_| ())?
        .to_string();
    if !is_safe_filename(&decoded) {
        return Err(());
    }
    let path = dir.join(&decoded);
    let ext_ok = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| allowed_exts.iter().any(|a| ext.eq_ignore_ascii_case(a)))
        .unwrap_or(false);
    if !ext_ok || !is_regular_file(&path) {
        return Err(());
    }
    Ok(path)
}

async fn batch_delete(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> Json<BatchResult> {
    let dir = &state.image_dir;
    let mut success_count = 0usize;
    let mut fail_count = 0usize;
    let mut errors = Vec::new();
    let mut deleted_names = Vec::new();

    for filename in &req.filenames {
        let target = resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic", "webp"]);
        match target {
            Ok(path) => {
                let event_filename = delete_event_filename(dir, filename, &path);
                let related = related_files_for_image(dir, &path).await;
                let mut deleted_any = false;
                for file in related {
                    match tokio::fs::remove_file(&file).await {
                        Ok(_) => deleted_any = true,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => errors.push(format!("delete {}: {}", file.display(), e)),
                    }
                }
                if deleted_any {
                    success_count += 1;
                    deleted_names.push(event_filename);
                } else {
                    fail_count += 1;
                    errors.push(format!("file not found: {}", filename));
                }
            }
            Err(_) => {
                errors.push(format!("file not found: {}", filename));
                fail_count += 1;
            }
        }
    }

    if !deleted_names.is_empty() {
        let msg = serde_json::json!({"filenames": deleted_names}).to_string();
        let _ = state.deletes.send(msg);
    }

    Json(BatchResult {
        success_count,
        fail_count,
        errors,
        download_pngs: None,
    })
}

async fn batch_mtime(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> Json<BatchResult> {
    let dir = &state.image_dir;
    let mut ordered = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut errors = Vec::new();
    let mut fail_count = 0usize;

    for filename in &req.filenames {
        let target = resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic", "webp"]);
        match target {
            Ok(path) => {
                let png_path = match png_path_for_related_file(dir, &path) {
                    Some(path) if is_source_image(&path) && is_regular_file(&path) => path,
                    _ => {
                        fail_count += 1;
                        errors.push(format!("png not found for: {}", filename));
                        continue;
                    }
                };
                let Some(png_name) = safe_file_name_from_path(&png_path) else {
                    fail_count += 1;
                    errors.push(format!("unsafe filename: {}", filename));
                    continue;
                };
                if !seen.insert(png_name.clone()) {
                    continue;
                }
                match tokio::fs::metadata(&png_path).await {
                    Ok(metadata) if metadata.is_file() => {
                        let modified =
                            system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
                        ordered.push((png_name, png_path, modified));
                    }
                    _ => {
                        fail_count += 1;
                        errors.push(format!("file not found: {}", filename));
                    }
                }
            }
            Err(_) => {
                fail_count += 1;
                errors.push(format!("file not found: {}", filename));
            }
        }
    }

    if ordered.is_empty() {
        return Json(BatchResult {
            success_count: 0,
            fail_count,
            errors,
            download_pngs: None,
        });
    }

    // The request order is the current visual order (newest → oldest). Use the
    // oldest selected PNG timestamp as the base, then assign +0.01s going backwards
    // through that visual order so the selected images keep the same sort order
    // without spreading a large selected batch across many seconds.
    let base_millis = ordered.iter().map(|(_, _, m)| *m).min().unwrap_or(0);
    let count = ordered.len();
    let mut success_count = 0usize;

    for (idx, (png_name, png_path, _)) in ordered.iter().enumerate() {
        let target_millis =
            base_millis.saturating_add(((count - 1 - idx) as i64).saturating_mul(10));
        let related = related_files_for_image(dir, png_path).await;
        let mut updated_any = false;
        let mut failed_any = false;
        for file in related {
            match set_file_mtime_millis(&file, target_millis) {
                Ok(_) => updated_any = true,
                Err(e) => {
                    failed_any = true;
                    errors.push(format!("set mtime {}: {}", file.display(), e));
                }
            }
        }
        if updated_any && !failed_any {
            success_count += 1;
        } else {
            fail_count += 1;
            if !updated_any {
                errors.push(format!("file not found: {}", png_name));
            }
        }
    }

    if success_count > 0 {
        let msg = serde_json::json!({"type": "resync"}).to_string();
        let _ = state.updates.send(msg);
    }

    Json(BatchResult {
        success_count,
        fail_count,
        errors,
        download_pngs: None,
    })
}

fn set_file_mtime_millis(path: &Path, millis: i64) -> std::io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    let atime = metadata
        .accessed()
        .map(FileTime::from_system_time)
        .unwrap_or_else(|_| FileTime::from_unix_time(0, 0));
    let mtime = filetime_from_millis(millis);
    filetime::set_file_times(path, atime, mtime)
}

fn filetime_from_millis(millis: i64) -> FileTime {
    let secs = millis.div_euclid(1000);
    let nanos = (millis.rem_euclid(1000) as u32) * 1_000_000;
    FileTime::from_unix_time(secs, nanos)
}

async fn batch_download(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> axum::response::Response {
    let dir = &state.image_dir;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for filename in &req.filenames {
        if resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic"]).is_ok() {
            let (_stem, ext) = split_stem_ext(filename);
            let chosen = if ext.eq_ignore_ascii_case("heic") {
                filename.clone()
            } else {
                find_heic_for_png(dir, &dir.join(filename))
                    .await
                    .unwrap_or_else(|| filename.clone())
            };
            let Ok(path) = resolve_file_or_not(dir, &chosen, &["png", "jpg", "jpeg", "heic"])
            else {
                continue;
            };
            let Ok(bytes) = tokio::fs::read(&path).await else {
                continue;
            };
            files.push((chosen, bytes));
        }
    }
    if files.is_empty() {
        return (StatusCode::BAD_REQUEST, "no valid files selected").into_response();
    }

    match build_zip_store(&files) {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str("attachment; filename=\"hermes_batch.zip\"").unwrap(),
            );
            (headers, bytes).into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

fn build_zip_store(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        if !is_safe_filename(name) {
            return Err(format!("unsafe filename: {name}"));
        }
        let name_bytes = name.as_bytes();
        let size =
            u32::try_from(data.len()).map_err(|_| format!("file too large for zip: {name}"))?;
        let offset = u32::try_from(out.len()).map_err(|_| "zip too large".to_string())?;
        let crc = crc32fast::hash(data);
        write_u32(&mut out, 0x04034b50);
        write_u16(&mut out, 20);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u32(&mut out, crc);
        write_u32(&mut out, size);
        write_u32(&mut out, size);
        write_u16(
            &mut out,
            u16::try_from(name_bytes.len()).map_err(|_| format!("filename too long: {name}"))?,
        );
        write_u16(&mut out, 0);
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        write_u32(&mut central, 0x02014b50);
        write_u16(&mut central, 20);
        write_u16(&mut central, 20);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, crc);
        write_u32(&mut central, size);
        write_u32(&mut central, size);
        write_u16(
            &mut central,
            u16::try_from(name_bytes.len()).map_err(|_| format!("filename too long: {name}"))?,
        );
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, 0);
        write_u32(&mut central, offset);
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32::try_from(out.len()).map_err(|_| "zip too large".to_string())?;
    let central_size = u32::try_from(central.len()).map_err(|_| "zip too large".to_string())?;
    out.extend_from_slice(&central);
    write_u32(&mut out, 0x06054b50);
    write_u16(&mut out, 0);
    write_u16(&mut out, 0);
    write_u16(
        &mut out,
        u16::try_from(files.len()).map_err(|_| "too many files for zip".to_string())?,
    );
    write_u16(
        &mut out,
        u16::try_from(files.len()).map_err(|_| "too many files for zip".to_string())?,
    );
    write_u32(&mut out, central_size);
    write_u32(&mut out, central_offset);
    write_u16(&mut out, 0);
    Ok(out)
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn split_stem_ext(filename: &str) -> (&str, &str) {
    if let Some(dot) = filename.rfind('.') {
        let (stem, ext) = filename.split_at(dot);
        (stem, &ext[1..])
    } else {
        (filename, "")
    }
}
