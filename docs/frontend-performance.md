# Frontend resource checks

The performance harness runs the real production `ChatTranscript` component in an isolated browser page with synthetic data. It does not connect to a running Yahu/Hermes service, read credentials, or create conversations.

## Run

Install Playwright in your QA environment and provide its module and Chromium executable when they are outside the project's module resolution path:

```bash
PLAYWRIGHT_MODULE="$PLAYWRIGHT_MODULE" \
CHROMIUM_EXECUTABLE="$CHROMIUM_EXECUTABLE" \
bun scripts/measure-ui-performance.ts --check
```

Both variables are optional when the `playwright` package and its matching Chromium installation are already available. Temporary bundles are created under `TMPDIR` and removed on completion. For a baseline from a Git revision, set `YAHU_PERF_BASELINE_REF` to that revision and omit `--check` (old versions can exceed the new budgets). This overrides only the transcript module inside the isolated bundle; the worktree remains unchanged.

## Workloads and interpretation

- Chromium CPU throttling: 6×. This is a constrained-browser approximation, not a physical low-end-device measurement.
- History: 30 completed turns with six tool results per turn. Unopened details must have zero mounted tool-card children; expansion and nested disclosure state must still work.
- Streaming: eleven synchronous renders from 11,000 to 12,000 source characters. Only the recent tail uses individual animation spans; all preceding text and Markdown remain present.
- Retained JS heap is measured after forced garbage collection relative to the cleared fixture. It excludes the browser process, decoded images, GPU allocations, and the rest of the application. Do not present it as total app/device memory.
- Additional assertions compare the complete Markdown DOM after removing animation wrappers, preserve Unicode grapheme clusters and existing tail-node identity, respect reduced motion, and verify completed-message rendering.
- Timing is intentionally not an absolute pass/fail budget because host contention varies. Compare at least three interleaved baseline/candidate runs and use medians. Node budgets and functional assertions are deterministic.

## Follow verification

`bun scripts/verify-chat-follow.mjs` checks the deployed shell with browser-only data/event fixtures. It uses the same optional `PLAYWRIGHT_MODULE` / `CHROMIUM_EXECUTABLE` overrides as the resource harness. `YAHU_BASE_URL` defaults to loopback port 9642; `YAHU_ENV_FILE` defaults to the standard system-service environment file. The existing login key is used only in process memory and is never printed or saved. Every data route is mocked, and unexpected non-GET data requests fail the run; no conversation is created or modified.

The script checks active asset parity and desktop/mobile latest, history-held, near-latest-but-held, restored-history, and return-to-latest scenarios through streamed updates and completion. Optional `YAHU_QA_OUT` saves synthetic screenshots and JSON receipts. Active stream status is intentionally enabled so default-open trailing details exercise their own scroll callbacks.

## Rendering policy

Completed turn details mount on first expansion, then retain their nested UI state when closed again. Unopened groups do not parse/render their tool cards or register scroll callbacks. Streaming/explicitly opened groups continue to render immediately.

Visibility-aware polling stops timers while the document is hidden, resumes with an immediate refresh, prevents overlapping requests, and tears down on route changes. Chat stream status polling runs on the chat page only. Live event streams and active chat generation are independent of these periodic refreshes.
