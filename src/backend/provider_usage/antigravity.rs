const ANTIGRAVITY_PROVIDER: &str = "antigravity";
const ANTIGRAVITY_TITLE: &str = "Antigravity 用量";
const ANTIGRAVITY_USAGE_TIMEOUT: Duration = Duration::from_secs(25);
const ANTIGRAVITY_MAX_OUTPUT: u64 = 256 * 1024;

fn antigravity_cli() -> Option<PathBuf> {
    let home_bin = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local/bin"));
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(home_bin)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("agy"))
        .find(|file| {
            let Ok(meta) = file.metadata() else {
                return false;
            };
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = true;
            meta.is_file() && executable
        })
}

async fn antigravity_cli_usage(executable: &Path) -> Result<Vec<u8>, &'static str> {
    let mut command = tokio::process::Command::new(executable);
    command
        .args(["-p", "/usage", "--output-format", "json"])
        .current_dir("/")
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    // Retain the service user's existing CLI/keyring context, not Yahu's provider secrets.
    for key in [
        "HOME",
        "PATH",
        "LANG",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
        "XDG_STATE_HOME",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    tokio::time::timeout(ANTIGRAVITY_USAGE_TIMEOUT, async {
        let mut child = command.spawn().map_err(|_| "无法启动 agy")?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or("无法读取 agy 输出")?
            .take(ANTIGRAVITY_MAX_OUTPUT + 1);
        let mut data = Vec::new();
        stdout
            .read_to_end(&mut data)
            .await
            .map_err(|_| "无法读取 agy 输出")?;
        if data.len() as u64 > ANTIGRAVITY_MAX_OUTPUT {
            return Err("agy 输出超过大小限制");
        }
        let status = child.wait().await.map_err(|_| "无法读取 agy 退出状态")?;
        if !status.success() {
            return Err("agy /usage 查询失败，请检查 CLI 登录状态");
        }
        Ok(data)
    })
    .await
    .map_err(|_| "agy /usage 查询超时")?
}

struct AntigravityQuotaSnapshot {
    windows: [ProviderUsageWindow; 2],
    weekly_counter: (i64, f64),
}

fn antigravity_quota_snapshot(
    raw: &[u8],
    now: i64,
) -> Result<AntigravityQuotaSnapshot, &'static str> {
    let payload: Value = serde_json::from_slice(raw).map_err(|_| "agy /usage 未返回有效 JSON")?;
    if payload["status"] != "SUCCESS"
        || payload.pointer("/command/name").and_then(Value::as_str) != Some("usage")
    {
        return Err("agy /usage 查询失败，请检查 CLI 登录状态和版本");
    }
    let groups = payload
        .pointer("/command/data/groups")
        .and_then(Value::as_array)
        .ok_or("agy /usage 缺少额度分组")?;
    let buckets = groups
        .iter()
        .filter_map(|g| g["buckets"].as_array())
        .flatten()
        .collect::<Vec<_>>();
    let window = |id, period, label: &str| {
        let mut matches = buckets
            .iter()
            .filter(|bucket| bucket["id"].as_str() == Some(id));
        let bucket = matches.next().ok_or("agy /usage 缺少 Gemini 额度")?;
        if matches.next().is_some() || bucket["window"].as_str() != Some(period) {
            return Err("agy /usage 的 Gemini 额度窗口无效");
        }
        let fraction = bucket["remaining_fraction"]
            .as_f64()
            .filter(|f| (0.0..=1.0).contains(f))
            .ok_or("agy /usage 的 Gemini 剩余额度无效")?;
        let reset_at = bucket["reset_time"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|date| date.timestamp())
            .ok_or("agy /usage 的 Gemini 重置时间无效")?;
        let used = (1.0 - fraction) * 100.0;
        Ok((
            ProviderUsageWindow {
                window: label.into(),
                used: Some(format!("{used:.1}%")),
                reset: Some(provider_reset_duration(
                    reset_at.saturating_sub(now).max(0) as f64
                )),
                reset_at: Some(reset_at),
            },
            (reset_at, used),
        ))
    };
    let (short, _) = window("gemini-5h", "5h", "5h额度")?;
    let (weekly, counter) = window("gemini-weekly", "weekly", "周额度")?;
    Ok(AntigravityQuotaSnapshot {
        windows: [short, weekly],
        weekly_counter: counter,
    })
}

async fn fetch_antigravity_usage(state: &AppState, force: bool) -> ProviderUsageSection {
    let _guard = state
        .provider_usage_cache
        .antigravity_daily_refresh
        .lock()
        .await;
    let now = chrono::Utc::now().timestamp();
    if !force
        && let Some(mut section) = provider_daily_snapshot_section(state, ANTIGRAVITY_PROVIDER, now)
    {
        refresh_provider_cached_reset_times(&mut section, now);
        return section;
    }
    let mut section = ProviderUsageSection {
        provider: ANTIGRAVITY_PROVIDER.into(),
        title: ANTIGRAVITY_TITLE.into(),
        ..Default::default()
    };
    let result = match antigravity_cli() {
        Some(cli) => antigravity_cli_usage(&cli).await,
        None => Err("未找到 agy CLI"),
    };
    let now = chrono::Utc::now().timestamp();
    match result.and_then(|data| antigravity_quota_snapshot(&data, now)) {
        Ok(snapshot) => {
            section.windows.extend(snapshot.windows);
            if let Err(error) = provider_observed_daily(
                state,
                &mut section,
                ANTIGRAVITY_PROVIDER,
                "percentage_points",
                HashMap::from([("Gemini".into(), snapshot.weekly_counter)]),
                now,
            ) {
                section.errors.push(error);
            }
        }
        Err(error) => section.errors.push(error.into()),
    }
    section
}
