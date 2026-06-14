import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const backend = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');
const readme = () => readFileSync(new URL('../../README.md', import.meta.url), 'utf8');

describe('runtime arguments', () => {
  test('Hermes home is exposed as a first-class flag and env var', () => {
    const source = backend();
    const docs = readme();
    expect(source).toContain('hermes_home: Option<PathBuf>');
    expect(source).toContain('#[arg(long, env = "HERMES_HOME")]');
    expect(source).toContain('let hermes_home = args.hermes_home.unwrap_or_else(||');
    expect(docs).toContain('`--hermes-home` | `HERMES_HOME`');
  });

  test('GitHub release repo is not exposed as a runtime argument', () => {
    const source = backend();
    expect(source).not.toContain('YAHU_GITHUB_REPO');
    expect(source).not.toContain('github_repo');
  });
});
