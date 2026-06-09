// Reference excerpts copied from hermes-image-browser/src/main.rs.
// Used by CI when the sibling standalone checkout is unavailable.


const FS_EVENT_DEBOUNCE: Duration = Duration::from_millis(1500);

const FILE_STABILITY_PROBE: Duration = Duration::from_millis(250);

const FILE_STABILITY_ATTEMPTS: usize = 12;

const FILE_STABILITY_REQUEUE_ATTEMPTS: usize = 8;



fn start_image_watcher(
    dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> anyhow::Result<RecommendedWatcher> {
    let watch_dir = dir.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            if let Err(err) = res {
                warn!("image directory watch error: {}", err);
            }
            return;
        };
        if !is_interesting_event(&event.kind) {
            return;
        }
        for path in event.paths {
            if is_png_heic_or_webp(&path) {
                let _ = tx.send(path);
            }
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    info!("watching {} with inotify", watch_dir.display());
    Ok(watcher)
}

fn is_interesting_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        // CloseWrite is the important inotify signal for files written in-place:
        // do not refresh while ImageMagick/heif-enc is still transcoding.
        EventKind::Access(AccessKind::Close(AccessMode::Write))
            // Atomic-renames into the watched directory are already complete.
            | EventKind::Modify(ModifyKind::Name(_))
            // Metadata-only mtime changes (external organize/reorder tools) must
            // still refresh clients. These are also debounced before broadcast.
            | EventKind::Modify(ModifyKind::Metadata(_))
            // Keep Create and Data as fallbacks for platforms/backends that do
            // not expose close-write. process_fs_events still debounces and
            // probes for stable non-zero files before broadcasting.
            | EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
    )
}

fn process_fs_events(
    image_dir: PathBuf,
    mut rx: mpsc::UnboundedReceiver<PathBuf>,
    updates: broadcast::Sender<String>,
) {
    let mut last_sent: HashMap<String, ImageFingerprint> = HashMap::new();
    let mut pending: HashMap<PathBuf, (Instant, usize)> = HashMap::new();

    loop {
        if pending.is_empty() {
            let Some(changed_path) = rx.recv().await else {
                break;
            };
            if let Some(png_path) = png_path_for_changed_file(&image_dir, &changed_path) {
                pending.insert(png_path, (Instant::now() + FS_EVENT_DEBOUNCE, 0));
            }
            continue;
        }

        let next_deadline = pending
            .values()
            .map(|(deadline, _)| *deadline)
            .min()
            .unwrap();
        tokio::select! {
            maybe_path = rx.recv() => {
                let Some(changed_path) = maybe_path else {
                    break;
                };
                if let Some(png_path) = png_path_for_changed_file(&image_dir, &changed_path) {
                    // Coalesce PNG + derived HEIC/WebP bursts. Every related
                    // event pushes the deadline out and resets the retry count,
                    // so one /image generation should produce one refresh after
                    // all conversions close.
                    pending.insert(png_path, (Instant::now() + FS_EVENT_DEBOUNCE, 0));
                }
            }
            _ = sleep_until(next_deadline) => {
                let now = Instant::now();
                let due: Vec<PathBuf> = pending
                    .iter()
                    .filter_map(|(path, (deadline, _))| (*deadline <= now).then_some(path.clone()))
                    .collect();
                for png_path in due {
                    let retry_count = pending.remove(&png_path).map(|(_, retry_count)| retry_count).unwrap_or(0);
                    if !wait_for_stable_image_files(&image_dir, &png_path).await {
                        if retry_count + 1 >= FILE_STABILITY_REQUEUE_ATTEMPTS {
                            warn!(
                                "dropping unstable image update after {} retries: {}",
                                retry_count + 1,
                                png_path.display()
                            );
                        } else {
                            pending.insert(png_path, (Instant::now() + FILE_STABILITY_PROBE, retry_count + 1));
                        }
                        continue;
                    }
                    let Some(entry) = image_entry_for_png(&image_dir, &png_path).await else {
                        continue;
                    };
                    let fingerprint = (
                        entry.modified_at,
                        entry.size,
                        entry.heic_filename.clone(),
                        entry.image_url.clone(),
                        entry.png_url.clone(),
                        entry.heic_url.clone(),
                    );
                    if last_sent.get(&entry.filename) == Some(&fingerprint) {
                        continue;
                    }
                    last_sent.insert(entry.filename.clone(), fingerprint);
                    let msg = serde_json::json!({"type": "image", "data": entry}).to_string();
                    let _ = updates.send(msg);
                }
            }
        }
    }
}

fn wait_for_stable_image_files(dir: &Path, png_path: &Path) -> bool {
    for _ in 0..FILE_STABILITY_ATTEMPTS {
        let Some(first) = image_file_snapshot(dir, png_path).await else {
            return false;
        };
        if first.iter().any(|(_, size, _)| *size == 0) {
            sleep(FILE_STABILITY_PROBE).await;
            continue;
        }
        sleep(FILE_STABILITY_PROBE).await;
        let Some(second) = image_file_snapshot(dir, png_path).await else {
            continue;
        };
        if first == second {
            return true;
        }
    }
    false
}

fn image_file_snapshot(dir: &Path, png_path: &Path) -> Option<Vec<(String, u64, i64)>> {
    let mut names = Vec::new();
    let png_name = safe_file_name_from_path(png_path)?;
    names.push(png_name);
    names.extend(find_all_heic_for_png(dir, png_path).await);
    names.extend(find_all_webp_for_png(dir, png_path).await);
    names.sort();
    names.dedup();

    let mut snapshot = Vec::with_capacity(names.len());
    for name in names {
        let metadata = tokio::fs::symlink_metadata(dir.join(&name)).await.ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
        snapshot.push((name, metadata.len(), modified_at));
    }
    Some(snapshot)
}
