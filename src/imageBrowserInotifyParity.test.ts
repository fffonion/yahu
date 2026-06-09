import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';

const headless = () => readFileSync(new URL('./main.rs', import.meta.url), 'utf8');
const siblingStandalone = new URL('../../hermes-image-browser/src/main.rs', import.meta.url).pathname;
const STANDALONE_PATH = process.env.HERMES_IMAGE_BROWSER_SRC || (existsSync(siblingStandalone) ? siblingStandalone : '/dev/null');
const standalone = () => {
  const path = STANDALONE_PATH;
  if (path === '/dev/null') {
    throw new Error('Set HERMES_IMAGE_BROWSER_SRC env var to path of hermes-image-browser/src/main.rs');
  }
  return readFileSync(path, 'utf8');
};

function extractFunction(source: string, name: string) {
  const marker = `fn ${name}(`;
  let startIndex = source.indexOf(marker);
  if (startIndex < 0) {
    startIndex = source.indexOf(`async fn ${name}(`);
  }
  expect(startIndex).toBeGreaterThanOrEqual(0);
  const bodyStart = source.indexOf('{', startIndex);
  expect(bodyStart).toBeGreaterThan(startIndex);
  let depth = 0;
  for (let i = bodyStart; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === '{') depth += 1;
    if (ch === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(startIndex, i + 1).trim();
    }
  }
  throw new Error(`unterminated function ${name}`);
}

function extractConst(source: string, name: string) {
  const line = source.split('\n').find((row) => row.startsWith(`const ${name}:`));
  expect(line).toBeTruthy();
  return line;
}

function sliceBetween(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start);
  expect(startIndex).toBeGreaterThanOrEqual(0);
  const endIndex = source.indexOf(end, startIndex);
  expect(endIndex).toBeGreaterThan(startIndex);
  return source.slice(startIndex, endIndex).trim();
}

describe('image browser inotify parity with standalone source', () => {
  test('watcher, event filter, debounce, stability probe, and fingerprint logic match standalone code exactly', () => {
    const headlessSource = headless();
    const standaloneSource = standalone();
    for (const name of ['FS_EVENT_DEBOUNCE', 'FILE_STABILITY_PROBE', 'FILE_STABILITY_ATTEMPTS', 'FILE_STABILITY_REQUEUE_ATTEMPTS']) {
      expect(extractConst(headlessSource, name)).toBe(extractConst(standaloneSource, name));
    }
    for (const name of ['start_image_watcher', 'is_interesting_event', 'process_fs_events', 'wait_for_stable_image_files', 'image_file_snapshot']) {
      expect(extractFunction(headlessSource, name)).toBe(extractFunction(standaloneSource, name));
    }
  });

  test('SSE endpoint keeps standalone delete event and message event behavior under image-api namespace', () => {
    const source = headless();
    const events = sliceBetween(source, 'async fn image_events(', 'async fn serve_png');
    expect(source).toContain('.route("/image-api/events", get(image_events))');
    expect(events).toContain('let mut rx_images = state.updates.subscribe();');
    expect(events).toContain('let mut rx_deletes = state.deletes.subscribe();');
    expect(events).toContain('yield Ok(SseEvent::default().data(text));');
    expect(events).toContain('yield Ok(SseEvent::default().event("delete").data(text));');
    expect(events).toContain('Sse::new(stream).keep_alive(KeepAlive::default())');
  });
});
