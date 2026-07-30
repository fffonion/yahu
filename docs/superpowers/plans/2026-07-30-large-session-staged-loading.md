# Large Session Staged Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use codex-superpowers-executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the latest window of a large conversation before loading full-history-derived totals, minimap data, or context usage.

**Architecture:** Add a bounded local `state.db` tail reader for the initial skeleton window, retaining the existing full reconstruction as fallback and for older/around history. In React, mark a session latest-ready only after its newest messages commit, then start total/minimap and context requests in parallel while fixed DOM shells prevent layout movement.

**Tech Stack:** Rust/axum/rusqlite backend, React/TypeScript frontend, Bun tests, Cargo tests.

---

### Task 1: Bounded local latest-history reader

**Files:**
- Create: `src/backend/history_tail.rs`
- Modify: `src/backend/mod.rs`
- Test: `src/backend/tests/message_windows.rs`

- [x] **Step 1: Write failing Rust tests**

Add tests that create lineage segments in a temporary `state.db`, call `fetch_local_latest_history_window`, and assert chronological IDs, cross-lineage collection, unfinished-turn detail retention, and `has_older=true` without loading unrelated old payload rows.

- [x] **Step 2: Verify RED**

Run: `cargo test --bin yahu fetch_local_latest_history_window -- --nocapture`
Expected: compile failure because the helper is absent.

- [x] **Step 3: Implement the bounded reader**

Add `include!("history_tail.rs");` after `sessions.rs`. Implement:

```rust
struct LocalLatestHistoryWindow {
    messages: Vec<serde_json::Value>,
    has_older: bool,
}

fn fetch_local_latest_history_window(
    state: &AppState,
    session_id: &str,
    skeleton_limit: usize,
) -> anyhow::Result<Option<LocalLatestHistoryWindow>>
```

Open `state.db` read-only, resolve `local_session_history_entries`, query message rows by segment with `ORDER BY id DESC LIMIT ?`, and expand backward in bounded batches until `history_skeleton_messages` can provide `skeleton_limit` rows and the retained prefix reaches a user/system boundary. Reuse `row_to_session_message`, `local_message_history_filter`, compression-prefix trimming, and stable-ID deduplication. Return `None` when local schema/data cannot represent the session.

- [x] **Step 4: Verify GREEN**

Run the focused Cargo test and confirm all tail-reader tests pass.

### Task 2: Latest protocol and fallback

**Files:**
- Modify: `src/backend/sessions.rs`
- Test: `src/backend/tests/message_windows.rs`

- [x] **Step 1: Write failing handler tests**

Assert `view=latest` uses the local bounded reader, returns `metadata_pending=true`, omits an authoritative total, keeps `has_newer=false`, and falls back to the existing stitched API path when local data is unavailable.

- [x] **Step 2: Verify RED**

Run: `cargo test --bin yahu chat_messages_latest_phase -- --nocapture`
Expected: assertions fail because `phase` is ignored.

- [x] **Step 3: Implement handler branch**

Use the existing `ChatMessagesQuery.view` field. Before `fetch_session_history_messages`, handle the no-cursor `view=latest` request through `spawn_blocking(fetch_local_latest_history_window)`, inject turn durations, build the skeleton page, and return:

```json
{
  "object": "list",
  "data": [],
  "has_older": true,
  "has_newer": false,
  "metadata_pending": true
}
```

Populate `data` and flags from the local result. Existing directions and fallback retain the current response shape.

- [x] **Step 4: Verify GREEN**

Run the focused handler tests and existing message-window Rust tests.

### Task 3: React staged orchestration and fixed shells

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/styles.css`
- Modify: `frontend/src/i18n.ts`
- Create: `frontend/src/chatStagedLoadingUi.test.ts`
- Update: `frontend/src/sessionSearchUi.test.ts`

- [x] **Step 1: Write failing frontend tests**

Assert latest requests include `view=latest`; the session-selection effect no longer starts user-nav/context immediately; a second effect keyed by `latestReadySessionId` starts both after latest messages are committed; header total and minimap shells remain mounted in pending, empty, and error states.

- [x] **Step 2: Verify RED**

Run: `bun test frontend/src/chatStagedLoadingUi.test.ts frontend/src/sessionSearchUi.test.ts --timeout 20000`
Expected: tests fail on the old parallel-load effect and missing shells.

- [x] **Step 3: Implement staged state**

Add session-scoped state:

```ts
const [latestReadySessionId, setLatestReadySessionId] = useState('');
const [historyMetadataLoading, setHistoryMetadataLoading] = useState(false);
const [historyTotal, setHistoryTotal] = useState<number | null>(null);
const [userNavLoading, setUserNavLoading] = useState(false);
```

The latest request sends `view=latest`. After applying the latest message merge, schedule `setLatestReadySessionId(sessionId)` so metadata begins in a later effect after the render commit. The metadata effect uses an `AbortController`/request token, starts user-nav and context in parallel, applies `body.total` only for the still-active session, and ignores stale completions.

- [x] **Step 4: Implement fixed DOM shells**

Always render the header total span and `ChatUserNavigator`. Pending header content uses an equal-width skeleton. Pending minimap uses the same outer rail and inert placeholder bars. Empty/error results keep the outer rail dimensions.

- [x] **Step 5: Verify GREEN**

Run focused frontend tests, then all chat pagination, detail, streaming, minimap, and mobile tests.

### Task 4: Full verification and deployment

**Files:**
- No new production files

- [x] **Step 1: Run frontend gates**

Run `bun test --timeout 20000` and `bun run build`.

- [x] **Step 2: Run Rust gates**

Run `cargo fmt --all -- --check`, `cargo test --bin yahu`, and `cargo clippy --all-targets --all-features -- -D warnings`.

- [x] **Step 3: Check the diff**

Run `git diff --check` and inspect only task-related files while preserving unrelated worktree changes.

- [x] **Step 4: Deploy**

Build the frontend and release binary, install the release binary to `/home/wow/.local/bin/yahu`, restart only `yahu.service`, confirm active state and HTTP 200 at `/`.

- [x] **Step 5: Browser verification**

Open the largest session. Verify network ordering: latest request completes and message DOM renders before user-nav/context requests complete. Confirm the total and minimap shells exist throughout, streaming detail remains visible, console has no relevant errors, and the desktop screenshot shows no layout movement or overflow.
