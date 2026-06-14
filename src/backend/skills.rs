async fn skills_list(State(state): State<Arc<AppState>>) -> Response<Body> {
    let disabled = load_disabled_skills(&state).await.unwrap_or_default();
    let mut found = HashMap::<String, (SkillInfo, PathBuf)>::new();
    for root in skill_roots(&state) {
        collect_skill_dirs(&root, &root, &disabled, &mut found);
    }
    let mut data: Vec<SkillInfo> = found.into_values().map(|(skill, _)| skill).collect();
    data.sort_by(|a, b| {
        (a.category.to_lowercase(), a.name.to_lowercase())
            .cmp(&(b.category.to_lowercase(), b.name.to_lowercase()))
    });
    Json(serde_json::json!({"object": "list", "data": data})).into_response()
}

async fn skill_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_default();
    let root = match find_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &err.to_string()),
    };
    let dir = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let mut reader = match fs::read_dir(&dir).await {
        Ok(reader) => reader,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read skill directory: {err}"),
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
        entries.push(WorkspaceEntry {
            name,
            path,
            kind: if meta.is_dir() { "dir" } else { "file" }.to_string(),
            size: meta.is_file().then_some(meta.len()),
            modified: meta.modified().ok().and_then(system_time_string),
        });
    }
    entries.sort_by(|a, b| {
        (a.kind.as_str() != "dir", a.name.to_lowercase())
            .cmp(&(b.kind.as_str() != "dir", b.name.to_lowercase()))
    });
    Json(WorkspaceList {
        root: root.display().to_string(),
        path: rel,
        entries,
    })
    .into_response()
}

async fn skill_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_else(|| "SKILL.md".to_string());
    let root = match find_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &err.to_string()),
    };
    let file = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&file).await {
        Ok(meta) => meta,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot stat skill file: {err}"),
            );
        }
    };
    if !meta.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }
    let bytes = match fs::read(&file).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read skill file: {err}"),
            );
        }
    };
    let mime = mime_guess::from_path(&file).first_or_text_plain();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(bytes))
        .unwrap()
}


async fn skill_file_write(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
    Json(payload): Json<WorkspaceSavePayload>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_else(|| "SKILL.md".to_string());
    let root = match user_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return skill_mutation_error(err),
    };
    let file = match resolve_skill_write_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if let Some(parent) = file.parent()
        && let Err(err) = fs::create_dir_all(parent).await
    {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create skill directory: {err}"),
        );
    }
    if let Err(err) = fs::write(&file, payload.content).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write skill file: {err}"),
        );
    }
    Json(serde_json::json!({"ok": true, "path": rel})).into_response()
}

async fn skill_toggle(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<SkillTogglePayload>,
) -> Response<Body> {
    if find_skill_dir(&state, &name).is_err() {
        return json_error(StatusCode::NOT_FOUND, "skill not found");
    }
    match set_skill_enabled(&state, &name, payload.enabled).await {
        Ok(()) => Json(serde_json::json!({"ok": true, "name": name, "enabled": payload.enabled}))
            .into_response(),
        Err(err) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot toggle skill: {err}"),
        ),
    }
}

async fn skill_item_rename(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
    Json(payload): Json<WorkspaceRenamePayload>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let root = match user_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return skill_mutation_error(err),
    };
    let source = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&source).await.is_err() {
        return json_error(StatusCode::NOT_FOUND, "skill item not found");
    }
    let target = match skill_item_destination_path(&root, &rel, &payload.name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&target).await.is_ok() {
        return json_error(StatusCode::CONFLICT, "target already exists");
    }
    if let Err(err) = fs::rename(&source, &target).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot rename skill item: {err}"),
        );
    }
    let parent = rel_parent(&rel);
    let path = rel_join(&parent, &payload.name);
    Json(serde_json::json!({"ok": true, "path": path, "name": payload.name})).into_response()
}

async fn skill_item_delete(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let root = match user_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return skill_mutation_error(err),
    };
    let path = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&path).await {
        Ok(meta) => meta,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "skill item not found"),
    };
    let result = if meta.is_dir() {
        fs::remove_dir_all(&path).await
    } else {
        fs::remove_file(&path).await
    };
    if let Err(err) = result {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot delete skill item: {err}"),
        );
    }
    Json(serde_json::json!({"ok": true, "path": rel})).into_response()
}

fn skill_mutation_error(err: anyhow::Error) -> Response<Body> {
    let message = err.to_string();
    let status = if message.contains("skill not found") {
        StatusCode::NOT_FOUND
    } else if message.contains("skill directory is not user-deletable") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    json_error(status, &message)
}

async fn skill_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> Response<Body> {
    match delete_skill_dir(&state, &name).await {
        Ok(path) => Json(serde_json::json!({"ok": true, "name": name, "deleted_path": path.display().to_string()})).into_response(),
        Err(err) => {
            let message = err.to_string();
            let status = if message.contains("skill not found") {
                StatusCode::NOT_FOUND
            } else if message.contains("skill directory is not user-deletable") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            json_error(status, &message)
        }
    }
}

async fn delete_skill_dir(state: &AppState, name: &str) -> anyhow::Result<PathBuf> {
    let canonical = user_skill_dir(state, name)?;
    fs::remove_dir_all(&canonical).await?;
    let _ = set_skill_enabled(state, name, true).await;
    Ok(canonical)
}

fn user_skill_dir(state: &AppState, name: &str) -> anyhow::Result<PathBuf> {
    let dir = find_skill_dir(state, name)?;
    let user_root_raw = state.hermes_home.join("skills");
    let user_root = user_root_raw.canonicalize().unwrap_or(user_root_raw);
    let canonical = dir.canonicalize()?;
    if !canonical.starts_with(&user_root) {
        anyhow::bail!("skill directory is not user-deletable");
    }
    if !canonical.join("SKILL.md").is_file() {
        anyhow::bail!("skill directory is invalid");
    }
    Ok(canonical)
}

fn skill_item_destination_path(root: &Path, rel: &str, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        anyhow::bail!("new name must be a single file name");
    }
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => anyhow::bail!("new name must be a single file name"),
    }
    let source = resolve_skill_file_path(root, rel)?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("skill item has no parent"))?;
    let target = parent.join(trimmed);
    if !target.starts_with(root) {
        anyhow::bail!("skill file path escapes root");
    }
    Ok(target)
}
fn skill_roots(state: &AppState) -> Vec<PathBuf> {
    let mut roots = vec![state.hermes_home.join("skills")];
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    roots.push(agent_dir.join("optional-skills"));
    roots
}

fn collect_skill_dirs(
    root: &Path,
    dir: &Path,
    disabled: &HashSet<String>,
    found: &mut HashMap<String, (SkillInfo, PathBuf)>,
) {
    let skill_md = dir.join("SKILL.md");
    if skill_md.is_file() {
        if let Ok(text) = std::fs::read_to_string(&skill_md) {
            if let Some(name) = frontmatter_value(&text, "name") {
                let description = frontmatter_value(&text, "description").unwrap_or_default();
                let category = dir
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .and_then(|p| p.to_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("uncategorized")
                    .to_string();
                found.entry(name.clone()).or_insert_with(|| {
                    (
                        SkillInfo {
                            name: name.clone(),
                            description,
                            category,
                            enabled: !disabled.contains(&name),
                        },
                        dir.to_path_buf(),
                    )
                });
            }
        }
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            collect_skill_dirs(root, &entry.path(), disabled, found);
        }
    }
}

fn frontmatter_value(text: &str, key: &str) -> Option<String> {
    let body = text.strip_prefix("---")?.splitn(2, "---").next()?;
    for line in body.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn find_skill_dir(state: &AppState, name: &str) -> anyhow::Result<PathBuf> {
    let disabled = HashSet::new();
    let mut found = HashMap::<String, (SkillInfo, PathBuf)>::new();
    for root in skill_roots(state) {
        collect_skill_dirs(&root, &root, &disabled, &mut found);
    }
    found
        .remove(name)
        .map(|(_, dir)| dir)
        .ok_or_else(|| anyhow::anyhow!("skill not found"))
}

fn resolve_skill_file_path(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid skill file path")
            }
        }
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = root.join(clean);
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    if !canonical.starts_with(&root) {
        anyhow::bail!("skill file path escapes root");
    }

    Ok(canonical)
}

fn resolve_skill_write_path(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid skill file path")
            }
        }
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = root.join(clean);
    if !candidate.starts_with(&root) {
        anyhow::bail!("skill file path escapes root");
    }
    Ok(candidate)
}

async fn load_disabled_skills(state: &AppState) -> anyhow::Result<HashSet<String>> {
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    let python = env::var("HERMES_WEBUI_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script = "import json, os, sys\nsys.path.insert(0, os.environ['HERMES_AGENT_DIR'])\nfrom hermes_cli.config import load_config\nfrom hermes_cli.skills_config import get_disabled_skills\nprint(json.dumps(sorted(get_disabled_skills(load_config()))))";
    let output = timeout(
        Duration::from_secs(20),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let list: Vec<String> = serde_json::from_slice(&output.stdout)?;
    Ok(list.into_iter().collect())
}

async fn set_skill_enabled(state: &AppState, name: &str, enabled: bool) -> anyhow::Result<()> {
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    let python = env::var("HERMES_WEBUI_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script = "import os, sys\nsys.path.insert(0, os.environ['HERMES_AGENT_DIR'])\nfrom hermes_cli.config import load_config\nfrom hermes_cli.skills_config import get_disabled_skills, save_disabled_skills\nname=os.environ['SKILL_NAME']; enabled=os.environ.get('SKILL_ENABLED')=='1'\nconfig=load_config(); disabled=get_disabled_skills(config)\n(disabled.discard(name) if enabled else disabled.add(name))\nsave_disabled_skills(config, disabled)";
    let output = timeout(
        Duration::from_secs(20),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .env("SKILL_NAME", name)
            .env("SKILL_ENABLED", if enabled { "1" } else { "0" })
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}
