import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const memoryBackend = () => readFileSync(new URL('../../src/backend/memory.rs', import.meta.url), 'utf8');
const backend = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');

describe('memory API routing', () => {
  test('yahu memory endpoint proxies Hermes API Server instead of reading HERMES_HOME files', () => {
    const source = memoryBackend();
    expect(source).toContain('format!("{}/api/memory", state.api_url)');
    expect(source).toContain('state.client.request(method, url)');
    expect(source).toContain('response_from_reqwest(state, None, resp).await');
    expect(source).not.toContain('state.hermes_home.join("memories")');
    expect(source).not.toContain('MEMORY.md');
    expect(source).not.toContain('USER.md');
    expect(source).not.toContain('fs::read_to_string');
    expect(source).not.toContain('fs::write');
  });

  test('memory route remains a yahu endpoint backed by API server proxying', () => {
    const source = backend();
    expect(source).toContain('.route("/memory", get(memory_get).put(memory_put))');
  });
});
