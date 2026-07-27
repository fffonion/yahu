const WEB_TERMINAL_CONCURRENCY: usize = 4;
const WEB_TERMINAL_INPUT_LIMIT: usize = 64 * 1024;
const WEB_TERMINAL_MIN_COLS: u16 = 2;
const WEB_TERMINAL_MAX_COLS: u16 = 500;
const WEB_TERMINAL_MIN_ROWS: u16 = 2;
const WEB_TERMINAL_MAX_ROWS: u16 = 200;

static WEB_TERMINAL_SLOTS: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(WEB_TERMINAL_CONCURRENCY)));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalDimensions {
    cols: u16,
    rows: u16,
}

#[derive(Debug, PartialEq, Eq)]
enum TerminalClientMessage {
    Input(String),
    Resize(TerminalDimensions),
}

#[derive(Default, Deserialize)]
struct TerminalQuery {
    cwd: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawTerminalClientMessage {
    Input { data: String },
    Resize { cols: u32, rows: u32 },
}

enum TerminalControl {
    Input(String),
    Resize(TerminalDimensions),
    Close,
}

struct TerminalProcess {
    master: Box<dyn portable_pty::MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

fn clamp_terminal_dimensions(cols: u32, rows: u32) -> TerminalDimensions {
    TerminalDimensions {
        cols: cols.clamp(
            u32::from(WEB_TERMINAL_MIN_COLS),
            u32::from(WEB_TERMINAL_MAX_COLS),
        ) as u16,
        rows: rows.clamp(
            u32::from(WEB_TERMINAL_MIN_ROWS),
            u32::from(WEB_TERMINAL_MAX_ROWS),
        ) as u16,
    }
}

fn parse_terminal_client_message(text: &str) -> Option<TerminalClientMessage> {
    match serde_json::from_str::<RawTerminalClientMessage>(text).ok()? {
        RawTerminalClientMessage::Input { data } if data.len() <= WEB_TERMINAL_INPUT_LIMIT => {
            Some(TerminalClientMessage::Input(data))
        }
        RawTerminalClientMessage::Input { .. } => None,
        RawTerminalClientMessage::Resize { cols, rows } => Some(TerminalClientMessage::Resize(
            clamp_terminal_dimensions(cols, rows),
        )),
    }
}

fn terminal_home_dir(home: Option<std::ffi::OsString>) -> PathBuf {
    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn terminal_working_dir(workspace: &Path, cwd: Option<&str>, home: &Path) -> anyhow::Result<PathBuf> {
    let Some(cwd) = cwd.filter(|value| !value.is_empty()) else {
        return Ok(home.to_path_buf());
    };
    let path = resolve_workspace_path(workspace, cwd)?;
    if !path.is_dir() {
        anyhow::bail!("terminal cwd is not a workspace directory");
    }
    Ok(path)
}

fn terminal_pty_size(size: TerminalDimensions) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn spawn_terminal_process(
    shell: &Path,
    cwd: &Path,
    home: &Path,
    size: TerminalDimensions,
) -> anyhow::Result<TerminalProcess> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system.openpty(terminal_pty_size(size))?;
    let mut command = portable_pty::CommandBuilder::new(shell.as_os_str());
    command.cwd(cwd.as_os_str());
    command.env("HOME", home.as_os_str());
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let child = pair.slave.spawn_command(command)?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    Ok(TerminalProcess {
        master: pair.master,
        reader,
        writer,
        child,
    })
}

fn resize_terminal_process(
    master: &dyn portable_pty::MasterPty,
    size: TerminalDimensions,
) -> anyhow::Result<()> {
    master.resize(terminal_pty_size(size))
}

async fn web_terminal_websocket(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TerminalQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response<Body> {
    if !websocket_origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let home = terminal_home_dir(env::var_os("HOME"));
    let cwd = match terminal_working_dir(&state.workspace, query.cwd.as_deref(), &home) {
        Ok(path) => path,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    let Ok(permit) = WEB_TERMINAL_SLOTS.clone().try_acquire_owned() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many active Web Terminal sessions",
        )
            .into_response();
    };
    ws.on_upgrade(move |socket| stream_web_terminal(socket, permit, cwd, home))
        .into_response()
}

async fn stream_web_terminal(
    mut socket: WebSocket,
    permit: tokio::sync::OwnedSemaphorePermit,
    cwd: PathBuf,
    home: PathBuf,
) {
    let shell = env::var_os("SHELL")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/bash"));
    let initial_size = TerminalDimensions { cols: 80, rows: 24 };
    let process = match tokio::task::spawn_blocking(move || {
        spawn_terminal_process(&shell, &cwd, &home, initial_size)
    })
    .await
    {
        Ok(Ok(process)) => process,
        Ok(Err(error)) => {
            let _ = socket
                .send(Message::Binary(
                    format!("\r\nUnable to start terminal: {error}\r\n")
                        .into_bytes()
                        .into(),
                ))
                .await;
            return;
        }
        Err(error) => {
            let _ = socket
                .send(Message::Binary(
                    format!("\r\nUnable to start terminal task: {error}\r\n")
                        .into_bytes()
                        .into(),
                ))
                .await;
            return;
        }
    };

    let (control_sender, control_receiver) = std::sync::mpsc::channel::<TerminalControl>();
    let (output_sender, mut output_receiver) = mpsc::channel::<Vec<u8>>(32);
    let driver = std::thread::Builder::new()
        .name("yahu-web-terminal".to_string())
        .spawn(move || drive_terminal_process(process, control_receiver, output_sender, permit));
    if let Err(error) = driver {
        let _ = socket
            .send(Message::Binary(
                format!("\r\nUnable to start terminal driver: {error}\r\n")
                    .into_bytes()
                    .into(),
            ))
            .await;
        return;
    }

    let (mut sender, mut receiver) = socket.split();
    loop {
        tokio::select! {
            output = output_receiver.recv() => {
                let Some(output) = output else { break; };
                if sender.send(Message::Binary(output.into())).await.is_err() {
                    break;
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let Some(message) = parse_terminal_client_message(&text) else { continue; };
                        let control = match message {
                            TerminalClientMessage::Input(data) => TerminalControl::Input(data),
                            TerminalClientMessage::Resize(size) => TerminalControl::Resize(size),
                        };
                        if control_sender.send(control).is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    let _ = control_sender.send(TerminalControl::Close);
}

fn drive_terminal_process(
    process: TerminalProcess,
    controls: std::sync::mpsc::Receiver<TerminalControl>,
    output: mpsc::Sender<Vec<u8>>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let TerminalProcess {
        master,
        mut reader,
        mut writer,
        mut child,
    } = process;
    let reader_output = output.clone();
    let reader_thread = std::thread::Builder::new()
        .name("yahu-web-terminal-reader".to_string())
        .spawn(move || {
            let mut buffer = vec![0_u8; 16 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if reader_output.blocking_send(buffer[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    drop(output);

    let mut exited = false;
    loop {
        match controls.recv_timeout(Duration::from_millis(50)) {
            Ok(TerminalControl::Input(data)) => {
                if writer.write_all(data.as_bytes()).and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
            Ok(TerminalControl::Resize(size)) => {
                let _ = resize_terminal_process(master.as_ref(), size);
            }
            Ok(TerminalControl::Close) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                exited = true;
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }

    if !exited {
        let _ = child.kill();
    }
    let _ = child.wait();
    drop(writer);
    drop(master);
    if let Ok(reader_thread) = reader_thread {
        let _ = reader_thread.join();
    }
}
