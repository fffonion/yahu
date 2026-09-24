import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

const workflow = readFileSync(new URL('../../.github/workflows/release.yml', import.meta.url), 'utf8');

describe('npm release assembly', () => {
  test('archive names map to the target directories used by the package launcher', () => {
    const extraction = workflow.match(/target="\$\(basename "\$archive" \.tar\.gz\)"\n\s*target="\$\{target#yahu-\}"/);
    expect(extraction).not.toBeNull();
    for (const target of [
      'x86_64-unknown-linux-gnu',
      'aarch64-unknown-linux-gnu',
      'aarch64-apple-darwin',
    ]) {
      const result = spawnSync('bash', ['-c', `archive="artifacts/yahu-${target}.tar.gz"\n${extraction![0]}\nprintf '%s' "$target"`], { encoding: 'utf8' });
      expect(result.status).toBe(0);
      expect(result.stdout).toBe(target);
    }
  });
});
