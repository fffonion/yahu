# Yahu Web Terminal Implementation Plan

**Goal:** Add an authenticated PTY-backed Web Terminal with desktop/mobile navigation, mobile keyboard focus, an embedded monospace Nerd Font, and Yahu-theme-aware terminal colors.

**Architecture:** Rust owns a same-origin authenticated WebSocket and one bounded PTY per connection. React mounts xterm.js on the first Terminal visit and keeps it mounted while other tools are active, preserving the WebSocket and PTY session. The fit addon keeps PTY dimensions aligned with the visible viewport. Pure protocol/theme/font helpers carry focused tests, while deployed browser checks prove shell I/O, session preservation, and mobile focus behavior.

**Tech Stack:** Rust 2024, axum WebSocket, portable-pty, Tokio, React, TypeScript, xterm.js, SauceCodePro Nerd Font Mono, Bun/Vite.

---

### Task 1: Backend protocol and security

**Files:**
- Create: `src/backend/terminal.rs`
- Create: `src/backend/tests/terminal.rs`
- Modify: `src/backend/mod.rs`
- Modify: `src/backend/tests.rs`
- Modify: `Cargo.toml`

**Steps:**
1. Add failing tests for same-origin policy, JSON input/resize parsing, resize clamping, and `HOME` fallback.
2. Run `cargo test --bin yahu terminal_ -- --nocapture` and confirm RED.
3. Add protocol types, shared WebSocket-origin validation, home resolution, and `/terminal/ws` route.
4. Add `portable-pty`, rerun focused tests, and confirm GREEN.

### Task 2: PTY lifecycle

**Files:**
- Modify: `src/backend/terminal.rs`
- Modify: `src/backend/tests/terminal.rs`

**Steps:**
1. Add a PTY smoke test that starts a shell in a temporary HOME, writes a deterministic command, reads its marker, resizes, then terminates the child.
2. Confirm RED before wiring the runtime.
3. Implement a four-slot permit, PTY reader thread, input/resize handling, binary output frames, and child kill/wait cleanup.
4. Run the focused backend tests and confirm GREEN.

### Task 3: Frontend terminal core

**Files:**
- Create: `frontend/src/WebTerminal.tsx`
- Create: `frontend/src/webTerminal.ts`
- Create: `frontend/src/webTerminal.test.ts`
- Modify: `package.json`
- Modify: `bun.lock`
- Modify: `frontend/src/main.tsx`

**Steps:**
1. Add failing pure tests for WebSocket URL construction, font-size clamping, light/dark ANSI theme mapping, and non-purple terminal surfaces.
2. Run `bun test frontend/src/webTerminal.test.ts` and confirm RED.
3. Install `@xterm/xterm` and `@xterm/addon-fit`, and bundle SauceCodePro Nerd Font Mono with its license notices.
4. Implement xterm lifecycle, stale-connection guard, binary decoding, fit/resize, status, clear, reconnect, and font-size persistence.
5. Re-run the focused tests and confirm GREEN.

### Task 4: Navigation and mobile keyboard

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/hashRoute.ts`
- Modify: `frontend/src/hashRoute.test.ts`
- Modify: `frontend/src/i18n.ts`
- Modify: `frontend/src/styles.css`
- Create: `frontend/src/webTerminalUi.test.ts`

**Steps:**
1. Add failing tests for `#/terminal`, desktop Terminal-before-Settings ordering, mobile Terminal-before-Settings ordering, keyboard control, helper-textarea attributes, and responsive CSS.
2. Run the focused frontend tests and confirm RED.
3. Add Terminal mode, rail control, shared mobile header action, terminal page, keyboard-focus callback, bundled monospace font, persistent mounted session, and responsive layout.
4. Re-run focused tests and confirm GREEN.

### Task 5: Full verification and deployment

**Files:**
- Modify: Yahu skill references for layout/API/terminal behavior.

**Steps:**
1. Run `bun test && bun run build`.
2. Run `cargo fmt --all -- --check && cargo test --bin yahu && cargo clippy --all-targets --all-features -- -D warnings`.
3. Run `git diff --check` and inspect changed files while preserving existing unrelated edits.
4. Run `make install`, restart only `yahu.service`, and verify health plus the active binary/assets.
5. In a desktop browser, open Terminal, run `printf 'YAHU_TERMINAL_OK\n'`, resize the viewport, and verify PTY columns/rows change.
6. In a mobile viewport, verify Terminal sits left of Settings, tapping the terminal and keyboard button focuses `.xterm-helper-textarea`, a command executes, no horizontal overflow exists, and screenshots show normal theme-mapped colors.
