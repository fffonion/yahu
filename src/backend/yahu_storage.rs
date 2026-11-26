fn yahu_json_path(hermes_home: &Path, name: &str) -> PathBuf {
    hermes_home.join("yahu").join(name)
}

fn migrate_yahu_json(hermes_home: &Path) -> std::io::Result<()> {
    use std::io::ErrorKind;
    let directory = hermes_home.join("yahu");
    let marker = directory.join(".json-layout-v1");
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "Yahu storage must be a directory",
            ));
        }
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&directory)?;
        }
        Err(err) => return Err(err),
    }
    if marker.is_file() {
        return Ok(());
    }

    // Migrate only Yahu-owned files. Leave legacy copies for rollback and old readers.
    const LEGACY_FILES: &[&str] = &[
        "skill-recent.json",
        "cache/yahu/model-inventory.json",
        "cache/yahu/model-price-overrides.json",
        "state/token_usage_cache.json",
        "state/vyceai-usage-snapshot.json",
        "state/mimo_service_token.json",
        "state/sensenova_service_token.json",
        "state/opendesign-control-key.json",
        "state/antigravity-daily-usage.json",
        "state/codex-daily-usage.json",
        "state/grok-daily-usage.json",
        "state/opencode-daily-usage.json",
        "state/opendesign-daily-usage.json",
        "state/sensenova-daily-usage.json",
        "state/zed-pro-daily-usage.json",
    ];
    for legacy in LEGACY_FILES {
        let source = hermes_home.join(legacy);
        let target = directory.join(source.file_name().expect("migration filename"));
        if target.try_exists()? {
            continue;
        }
        match std::fs::symlink_metadata(&source) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => continue,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }
        let mut source = std::fs::File::open(source)?;
        let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
        std::io::copy(&mut source, &mut temp)?;
        temp.as_file().sync_all()?;
        match temp.persist_noclobber(target) {
            Ok(_) => {}
            Err(err) if err.error.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.error),
        }
    }
    tempfile::NamedTempFile::new_in(directory)?.persist(marker)?;
    Ok(())
}

fn write_yahu_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let parent = path.parent().expect("Yahu JSON parent");
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), value)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
