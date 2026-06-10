use std::env::current_exe;

pub const YAHU_VERSION: &str = env!("YAHU_VERSION");

#[derive(Serialize)]
pub struct VersionInfo {
    pub version: String,
}

pub async fn yahu_version() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: YAHU_VERSION.to_string(),
    })
}

#[derive(Serialize)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub download_url: String,
    pub release_url: String,
}

pub async fn check_update(
    State(state): State<Arc<AppState>>,
) -> Result<Json<UpdateCheck>, (StatusCode, String)> {
    let repo = &state.github_repo;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = state
        .client
        .get(&url)
        .header("User-Agent", format!("yahu/{YAHU_VERSION}"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    if !resp.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("GitHub API returned {}", resp.status()),
        ));
    }

    let release: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let tag = release["tag_name"].as_str().unwrap_or("v0.0.0");
    let html_url = release["html_url"].as_str().unwrap_or("").to_string();
    let current = YAHU_VERSION.strip_prefix('v').unwrap_or(YAHU_VERSION);
    let latest = tag.strip_prefix('v').unwrap_or(tag);

    let asset_name = match asset_target() {
        Some(t) => format!("yahu-{t}.tar.gz"),
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "unsupported platform".into(),
            ))
        }
    };

    let download_url = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some(&asset_name))
                .and_then(|a| a["browser_download_url"].as_str())
                .map(String::from)
        })
        .unwrap_or_default();

    Ok(Json(UpdateCheck {
        current: current.to_string(),
        latest: latest.to_string(),
        available: compare_versions(current, latest) == std::cmp::Ordering::Less,
        download_url,
        release_url: html_url,
    }))
}

#[derive(Serialize)]
pub struct ApplyResult {
    pub success: bool,
    pub message: String,
}

pub async fn apply_update(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApplyResult>, (StatusCode, String)> {
    let repo = &state.github_repo;

    // Get latest release info
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let release: serde_json::Value = state
        .client
        .get(&url)
        .header("User-Agent", format!("yahu/{YAHU_VERSION}"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let tag = release["tag_name"].as_str().unwrap_or("");
    let asset_name = match asset_target() {
        Some(t) => format!("yahu-{t}.tar.gz"),
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "unsupported platform".into(),
            ))
        }
    };

    let download_url = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some(&asset_name))
                .and_then(|a| a["browser_download_url"].as_str())
                .map(String::from)
        })
        .ok_or_else(|| (StatusCode::NOT_FOUND, "asset not found".into()))?;

    info!("downloading update from {download_url}");

    // Download to temp file
    let tmp_dir = std::env::temp_dir();
    let archive_path = tmp_dir.join(format!("yahu-update-{tag}.tar.gz"));

    let resp = state
        .client
        .get(&download_url)
        .header("User-Agent", format!("yahu/{YAHU_VERSION}"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    fs::write(&archive_path, &bytes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("extracting update archive");

    // Extract binary from tar.gz
    let extracted = extract_yahu_binary(&archive_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let bin_path =
        current_exe().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let bin_dir = bin_path.parent().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "cannot determine binary directory".into(),
        )
    })?;

    #[cfg(not(target_os = "windows"))]
    {
        // Linux / macOS: unlink, move, execve
        let new_path = bin_dir.join("yahu-new");

        fs::rename(&extracted, &new_path)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&new_path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        // Unlink current binary
        fs::remove_file(&bin_path)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Move new binary into place
        fs::rename(&new_path, &bin_path)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        info!("update applied, restarting");

        // execve to replace current process
        let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
        let err = Command::new(&bin_path).args(args).status().await;
        // If execve succeeds we never reach here; if it fails, exit
        warn!("execve failed: {err:?}");
        std::process::exit(0);
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: can't replace running binary, use helper batch script
        let helper = bin_dir.join("yahu-update-helper.bat");
        let old_exe = bin_path.with_extension("exe.old");
        let bat = format!(
            concat!(
                "@echo off\r\n",
                "timeout /t 2 /nobreak >nul\r\n",
                "del /f /q \"{old}\" 2>nul\r\n",
                "ren \"{cur}\" \"{old_name}.old\" 2>nul\r\n",
                "move /y \"{new}\" \"{cur}\" >nul 2>&1\r\n",
                "del /f /q \"{old}\" 2>nul\r\n",
                "start \"\" \"{cur}\" {args}\r\n",
                "del /f /q \"%~f0\"\r\n",
            ),
            old = old_exe.display(),
            cur = bin_path.display(),
            old_name = bin_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("yahu"),
            new = extracted.display(),
            args = std::env::args().skip(1).collect::<Vec<_>>().join(" "),
        );

        fs::write(&helper, bat)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        info!("spawning update helper and exiting");

        Command::new("cmd")
            .args(["/C", "start", "", "/MIN", helper.to_str().unwrap_or("")])
            .spawn()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(1));
            std::process::exit(0);
        });
    }

    #[allow(unreachable_code)]
    Ok(Json(ApplyResult {
        success: true,
        message: "update applied".into(),
    }))
}

fn asset_target() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu"),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu"),
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("x86_64", "windows") => Some("x86_64-pc-windows-gnu"),
        _ => None,
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.trim_start_matches('v').parse().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}

async fn extract_yahu_binary(archive: &PathBuf) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir();
    let out_dir = archive.parent().unwrap_or(&tmp);
    let extracted_dir = out_dir.join(format!(
        "yahu-extract-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    fs::create_dir_all(&extracted_dir)
        .await
        .map_err(|e| e.to_string())?;

    // tar xzf (works on Linux/macOS and Windows with tar included)
    let status = Command::new("tar")
        .args([
            "xzf",
            archive.to_str().unwrap_or(""),
            "-C",
            extracted_dir.to_str().unwrap_or(""),
        ])
        .status()
        .await
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err("tar extraction failed".into());
    }

    // Find the yahu binary in extracted files
    find_yahu_binary(&extracted_dir)
        .await
        .ok_or_else(|| "yahu binary not found in archive".into())
}

fn find_yahu_binary_sync(dir: &PathBuf) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_yahu_binary_sync(&path) {
                return Some(found);
            }
        } else {
            let name = path.file_name()?.to_str()?;
            if name == "yahu" || name == "yahu.exe" {
                return Some(path);
            }
        }
    }
    None
}

async fn find_yahu_binary(dir: &PathBuf) -> Option<PathBuf> {
    tokio::task::spawn_blocking({
        let dir = dir.clone();
        move || find_yahu_binary_sync(&dir)
    })
    .await
    .unwrap_or(None)
}
