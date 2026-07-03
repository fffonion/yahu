use std::fs as std_fs;
use std::io;

async fn memory_get(State(state): State<Arc<AppState>>) -> Response<Body> {
    match read_memory_payload_from_files(&state.hermes_home) {
        Ok(payload) => Json(payload).into_response(),
        Err(err) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot read memory files: {err}"),
        ),
    }
}

async fn memory_put(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MemoryPayload>,
) -> Response<Body> {
    match write_memory_payload_to_files(&state.hermes_home, &payload) {
        Ok(()) => Json(payload).into_response(),
        Err(err) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write memory files: {err}"),
        ),
    }
}

fn read_memory_payload_from_files(hermes_home: &std::path::Path) -> io::Result<MemoryPayload> {
    Ok(MemoryPayload {
        memory: read_memory_text(&memory_file_path(hermes_home, "memory"))?,
        user: read_memory_text(&memory_file_path(hermes_home, "user"))?,
    })
}

fn write_memory_payload_to_files(hermes_home: &std::path::Path, payload: &MemoryPayload) -> io::Result<()> {
    write_memory_text(&memory_file_path(hermes_home, "memory"), &payload.memory)?;
    write_memory_text(&memory_file_path(hermes_home, "user"), &payload.user)?;
    Ok(())
}

fn memory_file_path(hermes_home: &std::path::Path, target: &str) -> std::path::PathBuf {
    let filename = if target == "user" { "USER.md" } else { "MEMORY.md" };
    hermes_home.join("memories").join(filename)
}

fn read_memory_text(path: &std::path::Path) -> io::Result<String> {
    let _lock = MemoryFileLock::acquire(path)?;
    match std_fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err),
    }
}

fn write_memory_text(path: &std::path::Path, content: &str) -> io::Result<()> {
    use std::io::Write;
    let _lock = MemoryFileLock::acquire(path)?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "memory file has no parent directory")
    })?;
    std_fs::create_dir_all(parent)?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_nanos();
    let tmp_path = parent.join(format!(".mem_{}_{}.tmp", std::process::id(), nanos));
    let write_result = (|| -> io::Result<()> {
        let mut file = std_fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        std_fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std_fs::remove_file(&tmp_path);
    }
    write_result
}

struct MemoryFileLock {
    file: std_fs::File,
}

impl MemoryFileLock {
    fn acquire(path: &std::path::Path) -> io::Result<Self> {
        let lock_path = path.with_extension(format!(
            "{}lock",
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        if let Some(parent) = lock_path.parent() {
            std_fs::create_dir_all(parent)?;
        }
        let file = std_fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(lock_path)?;
        lock_exclusive(&file)?;
        Ok(Self { file })
    }
}

impl Drop for MemoryFileLock {
    fn drop(&mut self) {
        let _ = unlock_file(&self.file);
    }
}

#[cfg(unix)]
fn lock_exclusive(file: &std_fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock_file(file: &std_fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_UN) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &std_fs::File) -> io::Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn unlock_file(_file: &std_fs::File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
const LOCK_EX: std::os::raw::c_int = 2;
#[cfg(unix)]
const LOCK_UN: std::os::raw::c_int = 8;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}
