#[derive(Serialize, Deserialize, Clone)]
struct ProviderDailyCounter {
    epoch: i64,
    high_water: f64,
    #[serde(default)]
    today: f64,
    #[serde(default)]
    observed_since: i64,
}

#[derive(Serialize, Deserialize)]
struct ProviderDailySnapshot {
    identity: String,
    day_start: i64,
    captured_at: i64,
    today: f64,
    counters: HashMap<String, ProviderDailyCounter>,
    section: ProviderUsageSection,
}

fn provider_daily_day_start(now: i64) -> i64 {
    now - (now + 8 * 3600).rem_euclid(86400)
}

fn provider_observed_daily(
    state: &AppState,
    section: &mut ProviderUsageSection,
    identity: &str,
    unit: &str,
    counters: HashMap<String, (i64, f64)>,
    now: i64,
) -> Result<(), String> {
    if identity.is_empty()
        || counters.is_empty()
        || counters.values().any(|(_, v)| !v.is_finite() || *v < 0.0)
    {
        return Err("今日差值缺少有效计数器或账户标识".into());
    }
    let path = state
        .hermes_home
        .join(format!("state/{}-daily-usage.json", section.provider));
    let previous = match std::fs::read(&path) {
        Ok(raw) => Some(
            serde_json::from_slice::<ProviderDailySnapshot>(&raw)
                .map_err(|_| "今日用量快照无法解析")?,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("今日用量快照读取失败".into()),
    };
    let day_start = provider_daily_day_start(now);
    let mut snapshot = match previous {
        Some(previous) if previous.identity == identity && previous.day_start == day_start => {
            previous
        }
        _ => ProviderDailySnapshot {
            identity: identity.into(),
            day_start,
            captured_at: now,
            today: 0.0,
            counters: counters
                .iter()
                .map(|(key, (epoch, value))| {
                    (
                        key.clone(),
                        ProviderDailyCounter {
                            epoch: *epoch,
                            high_water: *value,
                            today: 0.0,
                            observed_since: now,
                        },
                    )
                })
                .collect(),
            section: section.clone(),
        },
    };
    if now < snapshot.captured_at {
        return Err("今日用量采集时间早于快照".into());
    }
    let observed_keys: Vec<_> = counters.keys().cloned().collect();
    for (key, (epoch, value)) in counters {
        if let Some(previous) = snapshot.counters.get_mut(&key) {
            if epoch != previous.epoch {
                snapshot.today += value;
                previous.today += value;
                previous.epoch = epoch;
                previous.high_water = value;
            } else {
                let increment = (value - previous.high_water).max(0.0);
                snapshot.today += increment;
                previous.today += increment;
                previous.high_water = previous.high_water.max(value);
            }
        } else {
            // A newly observed pool/account starts at its own baseline, never at historical usage.
            snapshot.counters.insert(
                key,
                ProviderDailyCounter {
                    epoch,
                    high_water: value,
                    today: 0.0,
                    observed_since: now,
                },
            );
        }
    }
    if !snapshot.today.is_finite() {
        return Err("今日用量差值无效".into());
    }
    if unit == "percentage_points" {
        section.daily = observed_keys.into_iter().filter_map(|key| {
            let counter = snapshot.counters.get(&key)?;
            Some(ProviderDailyUsage { account: key, used: counter.today, observed_since: counter.observed_since })
        }).collect();
        section.daily.sort_by(|a, b| a.account.cmp(&b.account));
    } else {
    let used = if unit == "USD" {
        if snapshot.today > 0.0 && snapshot.today < 0.01 {
            format!("${}", snapshot.today)
        } else {
            fmt_provider_money(snapshot.today, '$')
        }
    } else {
        format!("{} Credit", snapshot.today)
    };
    section.windows.retain(|w| !w.window.contains("今日"));
    section.windows.push(ProviderUsageWindow {
        window: "今日用量（差值）".into(),
        used: Some(used),
        reset: None,
        reset_at: Some(day_start + 86400),
    });
    }
    section.captured_at = now as f64;
    snapshot.captured_at = now;
    snapshot.section = section.clone();
    let parent = path.parent().ok_or("今日用量快照路径无效")?;
    let raw = serde_json::to_vec(&snapshot).map_err(|_| "今日用量快照序列化失败")?;
    std::fs::create_dir_all(parent).map_err(|_| "今日用量快照保存失败")?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, raw)
        .and_then(|_| std::fs::rename(&temporary, &path))
        .map_err(|_| "今日用量快照保存失败")?;
    Ok(())
}

fn provider_capture_quota_daily(
    state: &AppState,
    section: &mut ProviderUsageSection,
    now: i64,
) -> Result<(), String> {
    section.daily.clear();
    let parsed: Vec<_> = section.windows.iter().filter_map(|window| {
        let (account, period) = [("周额度", "周"), ("月额度", "月"), ("周", "周"), ("月", "月")]
            .into_iter().find_map(|(suffix, period)| window.window.strip_suffix(suffix)
                .map(|account| (account.trim_matches(|c: char| c.is_whitespace() || c == '·'), period)))?;
        let used = provider_used_percent(window).filter(|v| (0.0..=100.0).contains(v))?;
        Some((account, period, used, window.reset_at.unwrap_or(0)))
    }).collect();
    let mut failed = Vec::new();
    for error in &section.errors {
        if error.contains("：Reset 查询失败：") {
            continue;
        }
        if let Some((account, _)) = error.split_once("：查询失败：") {
            failed.push(account);
        } else {
            return Err("今日用量采集失败".into());
        }
    }
    let mut counters = HashMap::new();
    for (account, period, used, epoch) in &parsed {
        if failed.iter().any(|failed| account == failed || account.starts_with(&format!("{failed} "))) {
            continue;
        }
        if (section.provider == "opencode" && *period != "月")
            || (section.provider == "codex" && *period != "周")
            || (section.provider == "grok" && *period == "月"
                && parsed.iter().any(|(other, period, _, _)| other == account && *period == "周"))
        {
            continue;
        }
        counters.insert(if account.is_empty() { "用量".into() } else { account.to_string() }, (*epoch, *used));
    }
    let identity = section.provider.clone();
    provider_observed_daily(state, section, &identity, "percentage_points", counters, now)
}

fn provider_daily_cached_section_is_current(section: &ProviderUsageSection, now: i64) -> bool {
    if !matches!(section.provider.as_str(), "zed-pro" | "sensenova" | "codex" | "grok" | "opencode") {
        return true;
    }
    let captured = section.captured_at as i64;
    captured > 0
        && captured <= now
        && now - captured < 300
        && provider_daily_day_start(captured) == provider_daily_day_start(now)
        && (!section.daily.is_empty() || section
            .windows
            .iter()
            .any(|w| w.window == "今日用量（差值）"))
}

fn provider_daily_snapshot_section(
    state: &AppState,
    provider: &str,
    now: i64,
) -> Option<ProviderUsageSection> {
    let path = state
        .hermes_home
        .join(format!("state/{provider}-daily-usage.json"));
    let raw = std::fs::read(path).ok()?;
    let snapshot: ProviderDailySnapshot = serde_json::from_slice(&raw).ok()?;
    provider_daily_cached_section_is_current(&snapshot.section, now).then_some(snapshot.section)
}

async fn run_provider_daily_snapshot_collector(state: Arc<AppState>) {
    loop {
        let catalog = provider_usage_catalog(&state.hermes_home);
        let zed = async {
            if catalog
                .iter()
                .any(|p| p.provider == "zed-pro" && p.query_ready)
            {
                let section = fetch_zed_pro_usage(&state, None, true).await;
                if !section.errors.is_empty() {
                    warn!("Zed daily snapshot capture failed");
                }
            }
        };
        let sensenova = async {
            if catalog
                .iter()
                .any(|p| p.provider == "sensenova" && p.query_ready)
            {
                let section = fetch_sensenova_usage(&state).await;
                if !section.errors.is_empty() {
                    warn!("SenseNova daily snapshot capture failed");
                }
            }
        };
        let quota = async {
            for provider in ["codex", "grok", "opencode", "opendesign"] {
                if catalog.iter().any(|p| p.provider == provider && p.query_ready) {
                    let cached = shared_cached_section(&state, provider).map(|entry| entry.section);
                    let section = fetch_provider_usage_section(&state, provider, cached.as_ref(), true).await;
                    if section.daily.is_empty() {
                        warn!(provider, "Quota daily snapshot capture failed");
                    }
                }
            }
        };
        tokio::join!(zed, sensenova, quota);
        tokio::time::sleep(Duration::from_secs(
            (300 - chrono::Utc::now().timestamp().rem_euclid(300)) as u64,
        ))
        .await;
    }
}
