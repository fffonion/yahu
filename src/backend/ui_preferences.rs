#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
struct UiPreferences {
    revision: u64,
    pinned_sessions: Option<Vec<String>>,
    enabled_providers: Option<Vec<String>>,
    provider_order: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiPreferencesPatch {
    revision: u64,
    pinned_sessions: Option<Vec<String>>,
    enabled_providers: Option<Vec<String>>,
    provider_order: Option<Vec<String>>,
}

fn read_ui_preferences(path: &Path) -> std::io::Result<UiPreferences> {
    match std::fs::File::open(path) {
        Ok(file) => Ok(serde_json::from_reader(std::io::BufReader::new(file))?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(UiPreferences::default()),
        Err(err) => Err(err),
    }
}

fn ui_preferences_response(status: StatusCode, preferences: UiPreferences) -> Response<Body> {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(preferences),
    )
        .into_response()
}

async fn get_ui_preferences(State(state): State<Arc<AppState>>) -> Response<Body> {
    let path = yahu_json_path(&state.hermes_home, "preferences.json");
    match tokio::task::spawn_blocking(move || read_ui_preferences(&path)).await {
        Ok(Ok(preferences)) => ui_preferences_response(StatusCode::OK, preferences),
        _ => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read Yahu preferences",
        ),
    }
}

async fn patch_ui_preferences(
    State(state): State<Arc<AppState>>,
    Json(mut patch): Json<UiPreferencesPatch>,
) -> Response<Body> {
    for (ids, limit) in [
        (&mut patch.pinned_sessions, MAX_PINNED_SESSION_IDS),
        (&mut patch.enabled_providers, 256),
        (&mut patch.provider_order, 256),
    ] {
        if let Some(ids) = ids {
            if ids.len() > limit
                || ids.iter().any(|id| {
                    id.is_empty()
                        || id.len() > 256
                        || id.trim() != id
                        || id.contains(',')
                        || id.chars().any(char::is_control)
                })
            {
                return json_error(StatusCode::BAD_REQUEST, "invalid preference IDs");
            }
            let mut seen = std::collections::HashSet::new();
            ids.retain(|id| seen.insert(id.clone()));
        }
    }
    let path = yahu_json_path(&state.hermes_home, "preferences.json");
    let lock = state.ui_preferences_write.clone().lock_owned().await;
    let updated = tokio::task::spawn_blocking(move || -> std::io::Result<_> {
        let _lock = lock;
        let mut current = read_ui_preferences(&path)?;
        if current.revision != patch.revision {
            return Ok((StatusCode::CONFLICT, current));
        }
        if let Some(ids) = patch.pinned_sessions {
            current.pinned_sessions = Some(ids);
        }
        if let Some(ids) = patch.enabled_providers {
            current.enabled_providers = Some(ids);
        }
        if let Some(ids) = patch.provider_order {
            current.provider_order = Some(ids);
        }
        current.revision = current
            .revision
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("preference revision overflow"))?;
        write_yahu_json(&path, &current)?;
        Ok((StatusCode::OK, current))
    })
    .await;
    match updated {
        Ok(Ok((status, preferences))) => ui_preferences_response(status, preferences),
        _ => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not save Yahu preferences",
        ),
    }
}
