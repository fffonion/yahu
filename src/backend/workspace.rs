async fn workspace_list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let dir = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let mut reader = match fs::read_dir(&dir).await {
        Ok(reader) => reader,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read directory: {err}"),
            );
        }
    };
    let mut entries = Vec::new();
    while let Ok(Some(entry)) = reader.next_entry().await {
        let meta = match entry.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = rel_join(&rel, &name);
        let kind = if meta.is_dir() { "dir" } else { "file" }.to_string();
        let modified = meta.modified().ok().and_then(system_time_string);
        entries.push(WorkspaceEntry {
            name,
            path,
            kind,
            size: if meta.is_file() {
                Some(meta.len())
            } else {
                None
            },
            modified,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind.as_str() != "dir", a.name.to_lowercase())
            .cmp(&(b.kind.as_str() != "dir", b.name.to_lowercase()))
    });
    Json(WorkspaceList {
        root: state.workspace.display().to_string(),
        path: rel,
        entries,
    })
    .into_response()
}

async fn workspace_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let file = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&file).await {
        Ok(meta) => meta,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot stat file: {err}")),
    };
    if !meta.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }
    let bytes = match fs::read(&file).await {
        Ok(bytes) => bytes,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot read file: {err}")),
    };
    let mime = mime_guess::from_path(&file).first_or_octet_stream();
    let filename = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref());
    if query.download.as_deref() == Some("1") {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename.replace('"', "")),
        );
    }
    builder.body(Body::from(bytes)).unwrap()
}

async fn workspace_rename(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(payload): Json<WorkspaceRenamePayload>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let source = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&source).await.is_err() {
        return json_error(StatusCode::NOT_FOUND, "workspace item not found");
    }
    let target = match workspace_destination_path(&state.workspace, &rel, &payload.name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&target).await.is_ok() {
        return json_error(StatusCode::CONFLICT, "target already exists");
    }
    if let Err(err) = fs::rename(&source, &target).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot rename workspace item: {err}"),
        );
    }
    let parent = rel_parent(&rel);
    let path = rel_join(&parent, &payload.name);
    Json(serde_json::json!({"ok": true, "path": path, "name": payload.name})).into_response()
}

async fn workspace_delete(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let path = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&path).await {
        Ok(meta) => meta,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "workspace item not found"),
    };
    let result = if meta.is_dir() {
        fs::remove_dir_all(&path).await
    } else {
        fs::remove_file(&path).await
    };
    if let Err(err) = result {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot delete workspace item: {err}"),
        );
    }
    Json(serde_json::json!({"ok": true, "path": rel})).into_response()
}
async fn workspace_save(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(payload): Json<WorkspaceSavePayload>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let file = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if let Err(err) = fs::write(&file, &payload.content).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write file: {err}"),
        );
    }
    Json(serde_json::json!({ "ok": true, "path": rel })).into_response()
}

fn resolve_workspace_path(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid workspace path")
            }
        }
    }
    let candidate = root.join(clean);
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    if !canonical.starts_with(root) {
        anyhow::bail!("workspace path escapes root");
    }
    Ok(canonical)
}

fn workspace_destination_path(root: &Path, rel: &str, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        anyhow::bail!("new name must be a single file name");
    }
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => anyhow::bail!("new name must be a single file name"),
    }
    let source = resolve_workspace_path(root, rel)?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("workspace item has no parent"))?;
    let target = parent.join(trimmed);
    if !target.starts_with(root) {
        anyhow::bail!("workspace path escapes root");
    }
    Ok(target)
}

fn rel_parent(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|p| *p != ".")
        .unwrap_or("")
        .to_string()
}

fn rel_join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

fn system_time_string(t: SystemTime) -> Option<String> {
    let secs = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(secs.to_string())
}
