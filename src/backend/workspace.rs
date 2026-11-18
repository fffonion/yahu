const WORKSPACE_REVIEW_MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;
const WORKSPACE_REVIEW_JSON_BODY_LIMIT: usize = WORKSPACE_REVIEW_MAX_CONTENT_BYTES * 6 + 1024;
const WORKSPACE_GIT_TIMEOUT: Duration = Duration::from_secs(8);
const WORKSPACE_DIFF_MAX_BYTES: usize = 2 * 1024 * 1024;
const WORKSPACE_LOG_PAGE_SIZE: usize = 20;
// The diff endpoint clamps commits:N to 500; keep the menu within that range.
const WORKSPACE_LOG_MAX_SKIP: usize = 480;

#[derive(Deserialize)]
struct WorkspaceReviewPayload {
    filename: String,
    content: String,
    #[serde(default)]
    path: Option<String>,
}

async fn workspace_list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let roots = &state.workspace_roots;
    let roots_info = || -> Option<Vec<WorkspaceRootInfo>> {
        (roots.len() > 1).then(|| {
            roots
                .iter()
                .enumerate()
                .map(|(index, root)| WorkspaceRootInfo {
                    name: root.name.clone(),
                    path: root.path.display().to_string(),
                    index,
                })
                .collect()
        })
    };
    if roots.len() > 1 && rel.trim().is_empty() {
        let entries = roots
            .iter()
            .enumerate()
            .map(|(index, root)| WorkspaceEntry {
                name: root.name.clone(),
                path: format!("@{index}"),
                kind: "dir".to_string(),
                size: None,
                modified: None,
            })
            .collect();
        return Json(WorkspaceList {
            root: roots[0].path.display().to_string(),
            path: rel,
            entries,
            roots: roots_info(),
        })
        .into_response();
    }
    let (_, dir) = match resolve_workspace_entry(roots, &rel) {
        Ok(resolved) => resolved,
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
        root: roots[0].path.display().to_string(),
        path: rel,
        entries,
        roots: roots_info(),
    })
    .into_response()
}

async fn workspace_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let (_, file) = match resolve_workspace_entry(&state.workspace_roots, &rel) {
        Ok(resolved) => resolved,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&file).await {
        Ok(meta) => meta,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot stat file: {err}")),
    };
    if !meta.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }
    let mime = mime_guess::from_path(&file).first_or_octet_stream();
    let is_text_path = workspace_path_is_text(&file);
    let mime_name = if is_text_path && mime.as_ref() == "application/octet-stream" {
        "text/plain; charset=utf-8"
    } else {
        mime.as_ref()
    };
    let full_preview = mime_name.starts_with("text/")
        || mime_name.starts_with("image/")
        || is_text_path;
    let limit_binary_preview = query.preview.as_deref() == Some("1")
        && query.download.as_deref() != Some("1")
        && !full_preview;
    let bytes = if limit_binary_preview {
        let handle = match fs::File::open(&file).await {
            Ok(handle) => handle,
            Err(err) => {
                return json_error(StatusCode::NOT_FOUND, &format!("cannot read file: {err}"));
            }
        };
        let mut reader = handle.take(WORKSPACE_BINARY_PREVIEW_LIMIT as u64);
        let mut bytes = Vec::with_capacity(WORKSPACE_BINARY_PREVIEW_LIMIT.min(meta.len() as usize));
        if let Err(err) = reader.read_to_end(&mut bytes).await {
            return json_error(StatusCode::NOT_FOUND, &format!("cannot read file: {err}"));
        }
        bytes
    } else {
        match fs::read(&file).await {
            Ok(bytes) => bytes,
            Err(err) => {
                return json_error(StatusCode::NOT_FOUND, &format!("cannot read file: {err}"));
            }
        }
    };
    let filename = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_name);
    if query.preview.as_deref() == Some("1") {
        builder = builder
            .header("x-yahu-file-size", meta.len().to_string())
            .header(
                "x-yahu-preview-truncated",
                if meta.len() > bytes.len() as u64 { "1" } else { "0" },
            );
    }
    if query.download.as_deref() == Some("1") {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename.replace('"', "")),
        );
    }
    builder.body(Body::from(bytes)).unwrap()
}

struct ResolvedWorkspaceFile {
    file: PathBuf,
    parent: PathBuf,
    name: OsString,
}

async fn resolve_workspace_file(
    state: &AppState,
    rel: &str,
) -> Result<ResolvedWorkspaceFile, (StatusCode, String)> {
    let (root_index, file) = resolve_workspace_entry(&state.workspace_roots, rel)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let root = &state.workspace_roots[root_index].path;
    let meta = fs::metadata(&file)
        .await
        .map_err(|err| (StatusCode::NOT_FOUND, format!("cannot stat file: {err}")))?;
    if !meta.is_file() {
        return Err((StatusCode::BAD_REQUEST, "path is not a file".to_string()));
    }
    let canonical_file = match fs::canonicalize(&file).await {
        Ok(path) if path.starts_with(root) => path,
        Ok(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "workspace path escapes root".to_string(),
            ));
        }
        Err(err) => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("cannot resolve file: {err}"),
            ));
        }
    };
    let parent = canonical_file
        .parent()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "file has no parent directory".to_string()))?
        .to_path_buf();
    let name = canonical_file
        .file_name()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "file has no name".to_string()))?
        .to_os_string();
    Ok(ResolvedWorkspaceFile {
        file: canonical_file,
        parent,
        name,
    })
}

const DIFF_BASE_CANDIDATES: [&str; 5] = [
    "origin/HEAD",
    "origin/main",
    "origin/master",
    "main",
    "master",
];

async fn first_existing_ref(parent: &Path) -> Option<String> {
    for candidate in DIFF_BASE_CANDIDATES {
        if run_workspace_git(git_ref_exists_args(parent, candidate))
            .await
            .is_some_and(|output| output.status.success())
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

fn diff_output(output: &std::process::Output) -> (String, bool) {
    let text = String::from_utf8_lossy(&output.stdout);
    let (text, text_truncated) = truncate_utf8(&text, WORKSPACE_DIFF_MAX_BYTES);
    (
        text.to_owned(),
        output.stdout.len() > WORKSPACE_DIFF_MAX_BYTES || text_truncated,
    )
}

async fn workspace_diff(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let resolved = match resolve_workspace_file(&state, &rel).await {
        Ok(resolved) => resolved,
        Err((status, message)) => return json_error(status, &message),
    };
    let parent = &resolved.parent;
    let name = &resolved.name;
    let canonical_file = &resolved.file;

    let is_git = run_workspace_git(git_is_inside_work_tree_args(parent))
        .await
        .is_some_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        });
    let has_head = is_git
        && run_workspace_git(git_verify_head_args(parent))
            .await
            .is_some_and(|output| output.status.success());

    let requested = query
        .base
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut untracked = false;
    let mut base: Option<String> = None;
    let mut diff = String::new();
    let mut truncated = false;

    if is_git && has_head {
        match requested {
            // Whole branch against the merge base with the default branch.
            Some("branch") => {
                if let Some(default_base) = first_existing_ref(parent).await {
                    if let Some(output) =
                        run_workspace_git(git_diff_base_args(parent, &default_base, name))
                            .await
                            .filter(|output| output.status.success())
                    {
                        (diff, truncated) = diff_output(&output);
                    }
                    base = Some(default_base);
                }
            }
            // Accumulated diff for the last N commits.
            Some(value) if value.starts_with("commits:") => {
                let count = value
                    .strip_prefix("commits:")
                    .and_then(|rest| rest.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 500);
                let reference = format!("HEAD~{count}");
                if let Some(output) = run_workspace_git(git_diff_ref_args(parent, &reference, name))
                    .await
                    .filter(|output| output.status.success())
                {
                    (diff, truncated) = diff_output(&output);
                }
                base = Some(reference);
            }
            _ => {
                if let Some(output) = run_workspace_git(git_diff_args(parent, name))
                    .await
                    .filter(|output| output.status.success())
                {
                    (diff, truncated) = diff_output(&output);
                }
                // A clean work tree is common in agent worktrees whose work
                // sits on a branch above the merge base; new files are
                // untracked entirely. Report both so review can still show
                // what changed instead of an empty diff.
                if diff.is_empty() {
                    if let Some(output) = run_workspace_git(git_status_porcelain_args(parent, name))
                        .await
                        .filter(|output| output.status.success())
                    {
                        untracked = String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .any(|line| line.starts_with("??"));
                    }
                    if !untracked {
                        for candidate in DIFF_BASE_CANDIDATES {
                            let exists = run_workspace_git(git_ref_exists_args(parent, candidate))
                                .await
                                .is_some_and(|output| output.status.success());
                            if !exists {
                                continue;
                            }
                            if let Some(output) =
                                run_workspace_git(git_diff_base_args(parent, candidate, name))
                                    .await
                                    .filter(|output| {
                                        output.status.success() && !output.stdout.is_empty()
                                    })
                            {
                                (diff, truncated) = diff_output(&output);
                                base = Some(candidate.to_owned());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    Json(serde_json::json!({
        "path": rel,
        "abs": canonical_file.to_string_lossy().into_owned(),
        "diff": diff,
        "is_git": is_git,
        "has_head": has_head,
        "truncated": truncated,
        "untracked": untracked,
        "base": base,
    }))
    .into_response()
}

async fn workspace_commits(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let skip = query.skip.unwrap_or(0);
    if skip > WORKSPACE_LOG_MAX_SKIP || skip % WORKSPACE_LOG_PAGE_SIZE != 0 {
        return json_error(StatusCode::BAD_REQUEST, "invalid commit page offset");
    }
    let resolved = match resolve_workspace_file(&state, &rel).await {
        Ok(resolved) => resolved,
        Err((status, message)) => return json_error(status, &message),
    };
    let parent = &resolved.parent;

    let is_git = run_workspace_git(git_is_inside_work_tree_args(parent))
        .await
        .is_some_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        });
    let has_head = is_git
        && run_workspace_git(git_verify_head_args(parent))
            .await
            .is_some_and(|output| output.status.success());

    let default_base = if skip == 0 && is_git && has_head {
        first_existing_ref(parent).await
    } else {
        None
    };
    let ahead = if let Some(default_base) = default_base.as_deref() {
        run_workspace_git(git_rev_list_count_args(parent, default_base))
            .await
            .filter(|output| output.status.success())
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
    } else {
        None
    };

    let mut commits = Vec::new();
    let log_output = if is_git && has_head {
        run_workspace_git(git_log_args(parent, skip))
            .await
            .filter(|output| output.status.success())
    } else {
        None
    };
    if let Some(output) = log_output {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let mut parts = line.split('\u{1f}');
            let (Some(sha), Some(short), Some(date), Some(subject)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if sha.is_empty() {
                continue;
            }
            commits.push(serde_json::json!({
                "sha": sha,
                "short": short,
                "date": date,
                "subject": subject,
            }));
        }
    }
    let has_more = commits.len() > WORKSPACE_LOG_PAGE_SIZE && skip < WORKSPACE_LOG_MAX_SKIP;
    commits.truncate(WORKSPACE_LOG_PAGE_SIZE);

    Json(serde_json::json!({
        "path": rel,
        "abs": resolved.file.to_string_lossy().into_owned(),
        "is_git": is_git,
        "has_head": has_head,
        "base": default_base,
        "ahead": ahead,
        "commits": commits,
        "has_more": has_more,
    }))
    .into_response()
}

fn workspace_path_is_text(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "txt" | "json" | "yaml" | "yml" | "toml" | "csv" | "ts" | "tsx"
                    | "js" | "jsx" | "py" | "rs" | "go" | "sh" | "css" | "html" | "lock"
                    | "capnp"
            )
        })
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| {
                    matches!(
                        name,
                        "Makefile" | "Dockerfile" | ".gitignore" | ".dockerignore" | ".env"
                            | ".npmrc" | ".prettierrc" | ".eslintrc"
                    )
                })
        })
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
    let (_, source) = match resolve_workspace_entry(&state.workspace_roots, &rel) {
        Ok(resolved) => resolved,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&source).await.is_err() {
        return json_error(StatusCode::NOT_FOUND, "workspace item not found");
    }
    let target = match workspace_destination_path(&state.workspace_roots, &rel, &payload.name) {
        Ok((_, path)) => path,
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
    let (_, path) = match resolve_workspace_entry(&state.workspace_roots, &rel) {
        Ok(resolved) => resolved,
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
    let (_, file) = match resolve_workspace_entry(&state.workspace_roots, &rel) {
        Ok(resolved) => resolved,
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

async fn workspace_review_report(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkspaceReviewPayload>,
) -> Response<Body> {
    if let Err(err) = validate_review_filename(&payload.filename) {
        return json_error(StatusCode::BAD_REQUEST, &err.to_string());
    }
    if payload.content.is_empty() || payload.content.len() > WORKSPACE_REVIEW_MAX_CONTENT_BYTES {
        return json_error(
            StatusCode::BAD_REQUEST,
            "review content must be non-empty and at most 2 MB",
        );
    }

    let root_index = payload
        .path
        .as_deref()
        .and_then(|path| resolve_workspace_entry(&state.workspace_roots, path).ok())
        .map_or(0, |(index, _)| index);
    let root = state.workspace_roots[root_index].path.clone();
    let review_dir = root.join(".yahu-reviews");
    let canonical_dir = fs::canonicalize(&review_dir).await.unwrap_or_else(|_| review_dir.clone());
    if !canonical_dir.starts_with(&root) {
        return json_error(StatusCode::BAD_REQUEST, "review directory escapes root");
    }
    if let Err(err) = fs::create_dir_all(&review_dir).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create review directory: {err}"),
        );
    }
    let review_dir = match fs::canonicalize(&review_dir).await {
        Ok(path) if path.starts_with(&root) => path,
        Ok(_) => return json_error(StatusCode::BAD_REQUEST, "review directory escapes root"),
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot resolve review directory: {err}"),
            );
        }
    };
    let target = match create_review_report(&review_dir, &payload.filename, &payload.content).await {
        Ok(target) => target,
        Err(ReviewWriteError::Symlink) => {
            return json_error(StatusCode::BAD_REQUEST, "review target must not be a symlink");
        }
        Err(ReviewWriteError::NotFile) => {
            return json_error(StatusCode::BAD_REQUEST, "review target is not a file");
        }
        Err(ReviewWriteError::Escapes) => {
            return json_error(StatusCode::BAD_REQUEST, "review target escapes directory");
        }
        Err(ReviewWriteError::Io(err)) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot write review report: {err}"),
            );
        }
    };
    let abs = match fs::canonicalize(&target).await {
        Ok(path) if path.starts_with(&root) && path.starts_with(&review_dir) => path,
        Ok(_) => return json_error(StatusCode::BAD_REQUEST, "review target escapes root"),
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot resolve review report: {err}"),
            );
        }
    };

    Json(serde_json::json!({
        "rel": format!(".yahu-reviews/{}", payload.filename),
        "abs": abs.to_string_lossy().into_owned(),
    }))
    .into_response()
}

/// Resolve a request path to (root index, absolute path). With one root the
/// grammar is unchanged; with several roots every path must start with
/// `@<index>/` so requests can never cross roots.
fn resolve_workspace_entry(roots: &[WorkspaceRoot], rel: &str) -> anyhow::Result<(usize, PathBuf)> {
    let (index, inner) = if roots.len() > 1 {
        let trimmed = rel.trim_start_matches('/');
        if trimmed.is_empty() {
            (0usize, String::new())
        } else {
            let (prefix, rest) = match trimmed.split_once('/') {
                Some((prefix, rest)) => (prefix, rest.to_string()),
                None => (trimmed, String::new()),
            };
            let index = prefix
                .strip_prefix('@')
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value < roots.len())
                .ok_or_else(|| anyhow::anyhow!("workspace path must start with @<root>"))?;
            (index, rest)
        }
    } else {
        (0, rel.to_string())
    };
    let root = &roots[index].path;
    let mut clean = PathBuf::new();
    for component in Path::new(&inner).components() {
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
    Ok((index, canonical))
}

fn workspace_destination_path(
    roots: &[WorkspaceRoot],
    rel: &str,
    new_name: &str,
) -> anyhow::Result<(usize, PathBuf)> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        anyhow::bail!("new name must be a single file name");
    }
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => anyhow::bail!("new name must be a single file name"),
    }
    let (index, source) = resolve_workspace_entry(roots, rel)?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("workspace item has no parent"))?;
    let target = parent.join(trimmed);
    if !target.starts_with(&roots[index].path) {
        anyhow::bail!("workspace path escapes root");
    }
    Ok((index, target))
}

enum ReviewWriteError {
    Symlink,
    NotFile,
    Escapes,
    Io(std::io::Error),
}

fn review_alternative_name(name: &str, attempt: usize) -> String {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("review");
    let extension = path.extension().and_then(OsStr::to_str).unwrap_or("md");
    format!("{stem}-{attempt}.{extension}")
}

/// Create the report with `create_new` so a second submit in the same second
/// never overwrites the first report; collisions get a numeric suffix.
async fn create_review_report(
    dir: &Path,
    name: &str,
    content: &str,
) -> Result<PathBuf, ReviewWriteError> {
    for attempt in 0..16 {
        let candidate = if attempt == 0 {
            dir.join(name)
        } else {
            dir.join(review_alternative_name(name, attempt))
        };
        match fs::symlink_metadata(&candidate).await {
            Ok(meta) if meta.file_type().is_symlink() => return Err(ReviewWriteError::Symlink),
            Ok(meta) if !meta.is_file() => return Err(ReviewWriteError::NotFile),
            Ok(_) => match fs::canonicalize(&candidate).await {
                Ok(path) if path.starts_with(dir) => {}
                Ok(_) => return Err(ReviewWriteError::Escapes),
                Err(err) => return Err(ReviewWriteError::Io(err)),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(ReviewWriteError::Io(err)),
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await
        {
            Ok(mut file) => {
                if let Err(err) =
                    tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes()).await
                {
                    return Err(ReviewWriteError::Io(err));
                }
                return Ok(candidate);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(ReviewWriteError::Io(err)),
        }
    }
    Err(ReviewWriteError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "too many review reports with the same name",
    )))
}

fn validate_review_filename(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
    {
        anyhow::bail!("review filename must be a single file name");
    }
    match Path::new(name).components().next() {
        Some(Component::Normal(component)) if component == OsStr::new(name) => Ok(()),
        _ => anyhow::bail!("review filename must be a single file name"),
    }
}

fn git_args(dir: &Path, args: &[&OsStr]) -> Vec<OsString> {
    let mut command_args = Vec::with_capacity(args.len() + 2);
    command_args.push(OsString::from("-C"));
    command_args.push(dir.as_os_str().to_owned());
    command_args.extend(args.iter().map(|arg| arg.to_os_string()));
    command_args
}

fn git_is_inside_work_tree_args(dir: &Path) -> Vec<OsString> {
    git_args(
        dir,
        &[OsStr::new("rev-parse"), OsStr::new("--is-inside-work-tree")],
    )
}

fn git_verify_head_args(dir: &Path) -> Vec<OsString> {
    git_args(
        dir,
        &[
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("HEAD"),
        ],
    )
}

fn git_diff_args(dir: &Path, name: &OsStr) -> Vec<OsString> {
    git_args(
        dir,
        &[
            // `--` does not stop git from reading magic pathspecs such as
            // `:(exclude)*`; literals keep the diff to the one named file.
            OsStr::new("--literal-pathspecs"),
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-color"),
            OsStr::new("HEAD"),
            OsStr::new("--"),
            name,
        ],
    )
}

fn git_status_porcelain_args(dir: &Path, name: &OsStr) -> Vec<OsString> {
    let mut args = git_args(
        dir,
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
        ],
    );
    args.push(OsString::from("--"));
    args.push(name.to_os_string());
    args
}

fn git_ref_exists_args(dir: &Path, reference: &str) -> Vec<OsString> {
    git_args(
        dir,
        &[
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("--quiet"),
            OsStr::new(reference),
        ],
    )
}

fn git_diff_base_args(dir: &Path, base: &str, name: &OsStr) -> Vec<OsString> {
    let mut args = git_args(
        dir,
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-color"),
        ],
    );
    args.push(OsString::from(format!("{base}...HEAD")));
    args.push(OsString::from("--"));
    args.push(name.to_os_string());
    args
}

fn git_diff_ref_args(dir: &Path, reference: &str, name: &OsStr) -> Vec<OsString> {
    let mut args = git_args(
        dir,
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-color"),
        ],
    );
    args.push(OsString::from(reference));
    args.push(OsString::from("--"));
    args.push(name.to_os_string());
    args
}

fn git_log_args(dir: &Path, skip: usize) -> Vec<OsString> {
    let mut args = git_args(
        dir,
        &[
            OsStr::new("log"),
            OsStr::new("--format=%H\u{1f}%h\u{1f}%cs\u{1f}%s"),
        ],
    );
    args.push(OsString::from(format!("--max-count={}", WORKSPACE_LOG_PAGE_SIZE + 1)));
    args.push(OsString::from(format!("--skip={skip}")));
    args.push(OsString::from("HEAD"));
    args
}

fn git_rev_list_count_args(dir: &Path, base: &str) -> Vec<OsString> {
    let mut args = git_args(dir, &[OsStr::new("rev-list"), OsStr::new("--count")]);
    args.push(OsString::from(format!("{base}..HEAD")));
    args
}

async fn run_workspace_git(args: Vec<OsString>) -> Option<std::process::Output> {
    timeout(
        WORKSPACE_GIT_TIMEOUT,
        Command::new("git").kill_on_drop(true).args(args).output(),
    )
    .await
    .ok()?
    .ok()
}

fn truncate_utf8(text: &str, cap: usize) -> (&str, bool) {
    if text.len() <= cap {
        return (text, false);
    }
    let mut end = cap;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
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

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn workspace_diff_args_include_parent_and_basename_in_order() {
        let args = git_diff_args(Path::new("/workspace/project"), OsStr::new("note.md"));

        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("--literal-pathspecs"),
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-color"),
                OsString::from("HEAD"),
                OsString::from("--"),
                OsString::from("note.md"),
            ]
        );
    }

    #[test]
    fn workspace_diff_base_args_wrap_the_merge_base_range() {
        let args = git_diff_base_args(
            Path::new("/workspace/project"),
            "origin/main",
            OsStr::new("note.md"),
        );
        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("--literal-pathspecs"),
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-color"),
                OsString::from("origin/main...HEAD"),
                OsString::from("--"),
                OsString::from("note.md"),
            ]
        );
    }

    #[test]
    fn workspace_diff_ref_args_use_the_commit_reference() {
        let args = git_diff_ref_args(
            Path::new("/workspace/project"),
            "HEAD~3",
            OsStr::new("note.md"),
        );
        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("--literal-pathspecs"),
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-color"),
                OsString::from("HEAD~3"),
                OsString::from("--"),
                OsString::from("note.md"),
            ]
        );
    }

    #[test]
    fn workspace_log_args_request_a_bounded_page_with_an_extra_row() {
        let args = git_log_args(Path::new("/workspace/project"), 20);
        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("log"),
                OsString::from("--format=%H\u{1f}%h\u{1f}%cs\u{1f}%s"),
                OsString::from("--max-count=21"),
                OsString::from("--skip=20"),
                OsString::from("HEAD"),
            ]
        );
    }

    #[test]
    fn workspace_rev_list_count_args_range_from_the_base() {
        let args = git_rev_list_count_args(Path::new("/workspace/project"), "origin/main");
        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("rev-list"),
                OsString::from("--count"),
                OsString::from("origin/main..HEAD"),
            ]
        );
    }

    #[test]
    fn workspace_diff_status_args_limit_the_pathspec_to_one_file() {
        let args = git_status_porcelain_args(Path::new("/workspace/project"), OsStr::new("note.md"));
        assert_eq!(
            args,
            vec![
                OsString::from("-C"),
                OsString::from("/workspace/project"),
                OsString::from("--literal-pathspecs"),
                OsString::from("status"),
                OsString::from("--porcelain=v1"),
                OsString::from("--"),
                OsString::from("note.md"),
            ]
        );
    }

    #[test]
    fn workspace_review_alternative_name_keeps_stem_and_extension() {
        assert_eq!(
            review_alternative_name("review-20261110-120000.md", 2),
            "review-20261110-120000-2.md"
        );
    }

    #[test]
    fn workspace_review_filename_validation_rejects_paths_and_accepts_safe_name() {
        for name in ["", "a/b", "a\\b", ".", "..", "bad\0name"] {
            assert!(validate_review_filename(name).is_err(), "accepted {name:?}");
        }

        assert!(validate_review_filename("review-20261110-120000.md").is_ok());
    }

    #[test]
    fn workspace_diff_truncation_preserves_utf8_boundary() {
        let (text, truncated) = truncate_utf8("aéz", 2);

        assert_eq!(text, "a");
        assert!(truncated);
    }

    #[test]
    fn workspace_entry_resolves_single_root_without_prefix() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("sub")).unwrap();
        let roots = [WorkspaceRoot {
            name: "root".to_string(),
            path: root.path().canonicalize().unwrap(),
        }];

        let (index, path) = resolve_workspace_entry(&roots, "sub").unwrap();
        assert_eq!(index, 0);
        assert!(path.ends_with("sub"));
        assert!(resolve_workspace_entry(&roots, "../escape").is_err());
    }

    #[test]
    fn workspace_entry_requires_root_prefix_in_multi_root_mode() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(second.path().join("note.md"), "x").unwrap();
        let roots = [
            WorkspaceRoot {
                name: "first".to_string(),
                path: first.path().canonicalize().unwrap(),
            },
            WorkspaceRoot {
                name: "second".to_string(),
                path: second.path().canonicalize().unwrap(),
            },
        ];

        let (index, path) = resolve_workspace_entry(&roots, "@1/note.md").unwrap();
        assert_eq!(index, 1);
        assert!(path.ends_with("note.md"));

        let (index, path) = resolve_workspace_entry(&roots, "@0").unwrap();
        assert_eq!(index, 0);
        assert_eq!(path, roots[0].path);

        assert!(resolve_workspace_entry(&roots, "@2/note.md").is_err());
        assert!(resolve_workspace_entry(&roots, "note.md").is_err());
        assert!(resolve_workspace_entry(&roots, "@0/../escape").is_err());

        let (index, target) = workspace_destination_path(&roots, "@1/note.md", "renamed.md").unwrap();
        assert_eq!(index, 1);
        assert!(target.starts_with(&roots[1].path));
        assert!(target.ends_with("renamed.md"));
    }
}
