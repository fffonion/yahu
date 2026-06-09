async fn serve_local_file(path: PathBuf, attachment: bool) -> axum::response::Response {
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let display_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("image")
        .replace('"', "");
    let mut headers = HeaderMap::new();
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap(),
    );
    if attachment {
        if let Ok(value) = HeaderValue::from_str(&content_disposition_attachment(&display_name)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    } else {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400, immutable"),
        );
    }
    (headers, bytes).into_response()
}


fn content_disposition_attachment(display_name: &str) -> String {
    let fallback: String = display_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' '))
        .collect();
    let fallback = if fallback.trim().is_empty() { "image" } else { fallback.trim() };
    let encoded = utf8_percent_encode(display_name, NON_ALPHANUMERIC).to_string();
    format!("attachment; filename=\"{}\"; filename*=UTF-8''{}", fallback.replace('"', ""), encoded)
}

fn file_version(metadata: &std::fs::Metadata) -> String {
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    format!("{}-{}", modified_at, metadata.len())
}

fn is_source_image(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(is_source_image_ext)
        .unwrap_or(false)
}

fn is_source_image_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("png")
        || ext.eq_ignore_ascii_case("jpg")
        || ext.eq_ignore_ascii_case("jpeg")
}

fn is_png_heic_or_webp(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            is_source_image_ext(ext)
                || ext.eq_ignore_ascii_case("heic")
                || ext.eq_ignore_ascii_case("webp")
        })
        .unwrap_or(false)
}

fn png_path_for_changed_file(dir: &Path, changed_path: &Path) -> Option<PathBuf> {
    png_path_for_related_file(dir, changed_path)
}

fn safe_file_name_from_path(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(OsStr::to_str)
        .filter(|name| is_safe_filename(name))
        .map(str::to_owned)
}

fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && !name.contains('\0')
        && !name.chars().any(|ch| {
            ch.is_control() || matches!(ch, '?' | '#' | '%' | '"' | ';' | ':' | '*' | '<' | '>' | '|')
        })
}

fn decode_filename(input: &str) -> Result<String, StatusCode> {
    let decoded = percent_decode_str(input)
        .decode_utf8()
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .to_string();
    if is_safe_filename(&decoded) {
        Ok(decoded)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

fn resolve_file(dir: &Path, filename: &str, allowed_exts: &[&str]) -> Result<PathBuf, StatusCode> {
    let decoded = decode_filename(filename)?;
    let path = dir.join(&decoded);
    let ext_ok = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            allowed_exts
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false);
    if !ext_ok {
        return Err(StatusCode::BAD_REQUEST);
    }
    if is_regular_file(&path) {
        Ok(path)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn find_heic_for_png(dir: &Path, png_path: &Path) -> Option<String> {
    find_preferred_related_for_png_by_ext(dir, png_path, "heic", &["", "_q82"])
}

async fn find_all_heic_for_png(dir: &Path, png_path: &Path) -> Vec<String> {
    find_related_for_png_by_ext(dir, png_path, "heic", &["", "_q82"]).await
}

fn find_preferred_related_for_png_by_ext(
    dir: &Path,
    png_path: &Path,
    ext: &str,
    preferred_suffixes: &[&str],
) -> Option<String> {
    let stem = png_path.file_stem().and_then(OsStr::to_str)?;
    for suffix in preferred_suffixes {
        let path = dir.join(format!("{stem}{suffix}.{ext}"));
        if is_regular_file(&path)
            && let Some(name) = safe_file_name_from_path(&path)
        {
            return Some(name);
        }
    }
    None
}

async fn find_related_for_png_by_ext(
    dir: &Path,
    png_path: &Path,
    ext: &str,
    preferred_suffixes: &[&str],
) -> Vec<String> {
    let Some(stem) = png_path.file_stem().and_then(OsStr::to_str) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();

    for suffix in preferred_suffixes {
        let path = dir.join(format!("{stem}{suffix}.{ext}"));
        if is_regular_file(&path)
            && let Some(name) = safe_file_name_from_path(&path)
            && seen.insert(name.clone())
        {
            names.push(name);
        }
    }

    let prefix = format!("{stem}_");
    let mut matches = Vec::new();
    let Ok(mut read_dir) = tokio::fs::read_dir(dir).await else {
        return names;
    };
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let Some(name) = safe_file_name_from_path(&path) else {
            continue;
        };
        if name.starts_with(&prefix)
            && path
                .extension()
                .and_then(OsStr::to_str)
                .map(|path_ext| path_ext.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
        {
            let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata.modified().map(system_time_to_millis).unwrap_or(0);
            matches.push((modified, name));
        }
    }
    matches.sort_by_key(|(modified, _)| Reverse(*modified));
    for (_, name) in matches {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

async fn find_webp_for_png(dir: &Path, png_path: &Path) -> Option<String> {
    find_preferred_related_for_png_by_ext(dir, png_path, "webp", &["", "_preview"])
}

async fn find_all_webp_for_png(dir: &Path, png_path: &Path) -> Vec<String> {
    find_related_for_png_by_ext(dir, png_path, "webp", &["", "_preview"]).await
}

fn png_path_for_related_file(dir: &Path, path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?;
    if is_source_image_ext(ext) {
        return Some(path.to_path_buf());
    }
    let stem = path.file_stem()?.to_str()?;
    let png_stem = stem
        .strip_suffix("_preview")
        .or_else(|| {
            let (base, quality) = stem.rsplit_once("_q")?;
            quality
                .chars()
                .all(|ch| ch.is_ascii_digit())
                .then_some(base)
        })
        .unwrap_or(stem);
    for source_ext in ["png", "jpg", "jpeg"] {
        let source_path = dir.join(format!("{png_stem}.{source_ext}"));
        if is_regular_file(&source_path) {
            return Some(source_path);
        }
    }
    None
}

async fn related_files_for_image(dir: &Path, path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if is_regular_file(path) {
        files.push(path.to_path_buf());
    }
    let png_path = png_path_for_related_file(dir, path);
    if let Some(png) = png_path.as_ref() {
        if is_regular_file(png) {
            files.push(png.to_path_buf());
        }
        for heic_name in find_all_heic_for_png(dir, png).await {
            let heic_path = dir.join(heic_name);
            if is_regular_file(&heic_path) {
                files.push(heic_path);
            }
        }
        for webp_name in find_all_webp_for_png(dir, png).await {
            let webp_path = dir.join(webp_name);
            if is_regular_file(&webp_path) {
                files.push(webp_path);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    files
        .into_iter()
        .filter(|p| {
            let Some(name) = safe_file_name_from_path(p) else {
                return false;
            };
            seen.insert(name)
        })
        .collect()
}

fn delete_event_filename(dir: &Path, requested: &str, path: &Path) -> String {
    if let Some(png) = png_path_for_related_file(dir, path)
        && let Some(name) = safe_file_name_from_path(&png)
    {
        return name;
    }
    requested.to_string()
}

fn system_time_to_millis(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}
