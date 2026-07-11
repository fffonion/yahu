# Cron Pause/Resume Button Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use codex-superpowers-executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one state-aware pause/resume action button to the selected Cron job header.

**Architecture:** Reuse the selected API job row as the state source. A parent callback posts to the existing Hermes API Server pause/resume endpoint, reloads jobs, and shows the localized result; `CronMain` only renders the state-aware control.

**Tech Stack:** React, TypeScript, Bun tests, Hermes API Server cron endpoints.

---

### Task 1: Add the failing UI/API contract test

**Files:**
- Modify: `frontend/src/cronLayout.test.ts`

- [ ] Assert that the Cron header contains localized pause/resume labels and a `cron-pause-toggle` button.
- [ ] Assert that `toggleCronPaused` chooses `pause` or `resume`, posts to `/api/jobs/{id}/{action}`, and reloads jobs.
- [ ] Run `bun test frontend/src/cronLayout.test.ts` and confirm the new assertions fail because the control and callback are absent.

### Task 2: Implement the state-aware action

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/i18n.ts`

- [ ] Import distinct `Pause` and `Play` icons.
- [ ] Add `toggleCronPaused`: find the selected job in `cronJobs`, derive paused state with `jobState`, POST to the existing `pause` or `resume` endpoint, handle non-OK responses, reload jobs, and show a localized toast.
- [ ] Pass the callback into `CronMain`.
- [ ] Render one button between Run and Delete. Active jobs show Pause; paused jobs show Play; unsaved jobs keep it disabled.
- [ ] Add localized pause/resume action, accessibility, and completion strings for English, Simplified Chinese, Traditional Chinese, and Japanese.
- [ ] Run `bun test frontend/src/cronLayout.test.ts` and confirm it passes.

### Task 3: Verify and ship

**Files:**
- No additional production files.

- [ ] Run `bun test`.
- [ ] Run `cargo test --bin yahu -- --nocapture`.
- [ ] Run `bun run build`.
- [ ] Commit the focused files.
- [ ] Deploy with `make install && sudo systemctl restart yahu.service`.
- [ ] In a real browser with mocked cron endpoints, verify the rendered control changes from Pause to Resume after one click and sends exactly one pause POST.
- [ ] Push `main` and report commit, test, build, service, and browser evidence.
