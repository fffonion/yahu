async fn serve_png(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> axum::response::Response {
    match resolve_file(&state.image_dir, &filename, &["png", "jpg", "jpeg", "webp"]) {
        Ok(path) => serve_local_file(path, false).await,
        Err(status) => status.into_response(),
    }
}

async fn download_heic(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> axum::response::Response {
    match resolve_file(&state.image_dir, &filename, &["heic"]) {
        Ok(path) => serve_local_file(path, true).await,
        Err(status) => status.into_response(),
    }
}

async fn generate_heic(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageEntry>, (StatusCode, String)> {
    let png_path = resolve_file(&state.image_dir, &filename, &["png"]).map_err(|status| {
        if resolve_file(&state.image_dir, &filename, &["jpg", "jpeg"]).is_ok() {
            (
                StatusCode::BAD_REQUEST,
                "HEIC generation is not supported for JPEG sources".to_string(),
            )
        } else {
            (status, "source image file not found".to_string())
        }
    })?;

    let source_metadata = tokio::fs::symlink_metadata(&png_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "source image file not found".to_string(),
        )
    })?;
    if !source_metadata.is_file() {
        return Err((
            StatusCode::NOT_FOUND,
            "source image file not found".to_string(),
        ));
    }
    if !source_can_generate_heic(&png_path) {
        return Err((
            StatusCode::BAD_REQUEST,
            "HEIC generation is only supported for PNG sources".to_string(),
        ));
    }

    if find_heic_for_png(&state.image_dir, &png_path)
        .await
        .is_none()
    {
        let script = heic_conversion_script();
        if !is_regular_file(&script) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HEIC conversion script not found: {}", script.display()),
            ));
        }
        let stem = png_path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "invalid PNG filename".to_string()))?;
        let output = png_path.with_file_name(format!("{stem}_q82.heic"));
        let convert = Command::new("bash")
            .arg(&script)
            .args(["-q", "82"])
            .arg(&png_path)
            .arg(&output)
            .output();
        let completed = timeout(Duration::from_secs(300), convert)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "HEIC conversion timed out".to_string(),
                )
            })?
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to run HEIC conversion: {err}"),
                )
            })?;
        if !completed.status.success() {
            let stderr = String::from_utf8_lossy(&completed.stderr)
                .trim()
                .to_string();
            let stdout = String::from_utf8_lossy(&completed.stdout)
                .trim()
                .to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit code {}", completed.status)
            };
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HEIC conversion failed: {detail}"),
            ));
        }
        if !is_regular_file(&output) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "HEIC conversion completed but output is missing: {}",
                    output.display()
                ),
            ));
        }
    }

    let entry = image_entry_for_png(&state.image_dir, &png_path)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "source image file not found".to_string(),
            )
        })?;
    let msg = serde_json::json!({"type": "image", "data": entry.clone()}).to_string();
    let _ = state.updates.send(msg);
    Ok(Json(entry))
}

fn heic_conversion_script() -> PathBuf {
    if let Ok(path) = env::var("HERMES_WEBUI_HEIC_SCRIPT") {
        let candidate = PathBuf::from(path);
        if is_regular_file(&candidate) {
            return candidate;
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(bin_dir) = exe.parent()
    {
        let share_script = bin_dir
            .parent()
            .unwrap_or(bin_dir)
            .join("share/yet-another-hermes-ui/scripts/png-to-ios-heic.sh");
        if is_regular_file(&share_script) {
            return share_script;
        }
    }
    if let Ok(cwd) = env::current_dir() {
        let project_script = cwd.join("scripts/png-to-ios-heic.sh");
        if is_regular_file(&project_script) {
            return project_script;
        }
    }
    if let Ok(home) = env::var("HERMES_HOME") {
        let hermes_script = PathBuf::from(home).join("scripts/png-to-ios-heic.sh");
        if is_regular_file(&hermes_script) {
            return hermes_script;
        }
    }
    if let Ok(home) = env::var("HOME") {
        let hermes_script = PathBuf::from(home).join(".hermes/scripts/png-to-ios-heic.sh");
        if is_regular_file(&hermes_script) {
            return hermes_script;
        }
    }
    // Fallback: look relative to the binary or next to the installed share dir
    let cwd = std::env::current_dir().unwrap_or_default();
    let share_path = cwd.join("scripts/png-to-ios-heic.sh");
    if is_regular_file(&share_path) {
        return share_path;
    }
    PathBuf::from("/usr/local/share/yet-another-hermes-ui/scripts/png-to-ios-heic.sh")
}
