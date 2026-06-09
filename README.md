# Yet Another Hermes UI (yahu)

A self-contained web UI for [Hermes Agent](https://github.com/NousResearch/hermes-agent). Inspired by `nesquena/hermes-webui`, rebuilt as a single Rust binary with embedded React frontend.

## Features

- Chat with session history, streaming responses, model picker, and reasoning effort selector
- Cron job management, memory editor, native image browser
- Workspace file browser with syntax-highlighted preview
- Skills management with enable/disable toggles and file explorer
- 10 themes (Hermes Light/Dark, VS Code Light+/Dark+, Monokai, Nord, Solarized Dark, Catppuccin Latte/Mocha, Nous)
- i18n: English, 简体中文, 繁體中文, 日本語
- Single binary, no runtime dependencies beyond a Hermes API Server

## Build

```bash
bun install
bun run build
cargo build --release
```

## Run

```bash
# Insecure (no login):
yahu --insecure --api-url http://127.0.0.1:8642 --host 127.0.0.1 --port 9642

# With auth:
HERMES_WEBUI_AUTH_KEY='your-password' \
HERMES_API_KEY='your-api-key' \
yahu --api-url http://127.0.0.1:8642 --host 0.0.0.0 --port 9642
```

Open `http://127.0.0.1:9642`.

## CLI Flags

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--host` | `HERMES_WEBUI_HOST` | `127.0.0.1` | Listen address |
| `--port` | `HERMES_WEBUI_PORT` | `9642` | Listen port |
| `--api-url` | `HERMES_WEBUI_API_URL` | `http://127.0.0.1:8642` | Hermes API Server upstream |
| `--hermes-home` | `HERMES_HOME` | `$HOME/.hermes` | Hermes data directory |
| `--workspace` | `HERMES_WEBUI_WORKSPACE` | `$HOME/workspace` | File browser root |
| `--image-dir` | `HERMES_WEBUI_IMAGE_DIR` | `$HERMES_HOME/cache/images` | Image gallery directory |
| `--insecure` | | | Skip WebUI login gate |

## Install

```bash
make install          # → ~/.local/bin/yahu
make service-install  # → ~/.config/systemd/user/yahu.service
make service-enable   # enable + start user service
```

## License

MIT
