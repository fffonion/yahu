import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const memoryBackend = () => readFileSync(new URL('../../src/backend/memory.rs', import.meta.url), 'utf8');
const backend = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');

describe('memory file routing', () => {
  test('yahu memory endpoint reads and writes profile-scoped memory files with Hermes locks', () => {
    const source = memoryBackend();
    expect(source).toContain('read_memory_payload_from_files(&state.hermes_home)');
    expect(source).toContain('write_memory_payload_to_files(&state.hermes_home, &payload)');
    expect(source).toContain('hermes_home.join("memories").join(filename)');
    expect(source).toContain('MEMORY.md');
    expect(source).toContain('USER.md');
    expect(source).toContain('MemoryFileLock::acquire(path)');
    expect(source).toContain('unsafe { flock(file.as_raw_fd(), LOCK_EX) }');
    expect(source).toContain('std_fs::rename(&tmp_path, path)');
    expect(source).not.toContain('format!("{}/api/memory", state.api_url)');
    expect(source).not.toContain('state.client.request(method, url)');
  });

  test('memory route remains a yahu endpoint for the Memory page', () => {
    const source = backend();
    expect(source).toContain('.route("/memory", get(memory_get).put(memory_put))');
  });
});
