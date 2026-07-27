# Yahu Web Terminal Design

## Scope

Add an authenticated interactive Web Terminal to Yahu. Desktop navigation places Terminal immediately before Settings. Mobile headers place Terminal immediately left of Settings. Each opened terminal owns one PTY-backed shell and releases it when its WebSocket closes through reconnect or page teardown.

## Backend

- Add authenticated same-origin `GET /terminal/ws` WebSocket route.
- Start the shell named by `SHELL`, falling back to `/bin/bash`.
- Set the initial working directory from the Yahu process user's `HOME`; fall back to `/` when unavailable.
- Set `TERM=xterm-256color` and `COLORTERM=truecolor`.
- Send PTY output as binary WebSocket frames.
- Accept JSON input and resize messages. Clamp resize dimensions to a bounded valid range.
- Stop and reap the child shell when the client closes or the transport fails.
- Limit concurrent Web Terminal PTYs to four and reject additional sessions with a visible terminal message.
- Reuse the existing same-origin WebSocket policy and authentication middleware.

## Frontend

- Add canonical `#/terminal` routing and a Terminal navigation icon before Settings.
- Mount the terminal on its first visit and keep it mounted while other tool routes are active, so switching tools preserves the WebSocket, xterm buffer, cwd, environment, and PTY process.
- Use xterm.js with the fit addon. A `ResizeObserver` refits the viewport and reports rows/columns to the PTY.
- Use bundled SauceCodePro Nerd Font Mono with monospace system fallbacks and retain its license/source notices.
- Provide font-size controls from 11 to 24 pixels, Clear, Reconnect, and status text. Persist the selected font size locally.
- Derive terminal background, foreground, cursor, selection, and standard ANSI colors from the active Yahu theme family. Use conventional blue, green, amber, red, cyan, and muted rose ANSI mappings without a purple-dominant surface.

## Mobile keyboard

- Place the Terminal header action immediately left of Settings.
- Attempt terminal focus when the route opens.
- Focus xterm when the user taps the terminal viewport.
- Provide an explicit mobile keyboard button that synchronously focuses xterm's helper textarea.
- Configure the helper textarea for text input with autocorrect, autocomplete, autocapitalization, and spellcheck disabled.
- Browser verification requires the helper textarea to become `document.activeElement` after tapping the viewport and after pressing the keyboard button.

## Error handling

- Show connecting, connected, disconnected, and error states in the terminal toolbar.
- Write backend connection errors into the terminal viewport and allow Reconnect.
- Ignore malformed client frames without terminating an otherwise valid session.
- Prevent stale WebSocket events from changing the state of a newer connection.

## Verification

- Rust tests cover origin policy, client message parsing, dimension clamping, home-directory resolution, and PTY shell I/O/resize cleanup.
- Frontend tests cover hash routing, desktop/mobile navigation placement, persistent mounted sessions, the bundled monospace font, font-size bounds, theme palettes, keyboard focus hooks, and WebSocket URL construction.
- Run full Bun and Rust gates, install the embedded frontend binary, restart only `yahu.service`, verify the active assets, then exercise a real shell command and resize in desktop and mobile viewports.
