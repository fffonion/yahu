import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
const updateRs = () => readFileSync(new URL('../../src/backend/update.rs', import.meta.url), 'utf8');

describe('self update flow', () => {
  test('linux update returns success before replacing the process image', () => {
    const source = updateRs();
    expect(source).toContain('return Ok(Json(ApplyResult');
    expect(source).toContain('update applied; restarting');
    expect(source).toContain('std::thread::sleep(Duration::from_millis(500))');
    expect(source).toContain('std::process::Command::new(&restart_path).args(args).exec()');
    expect(source).not.toContain('Command::new(&bin_path).args(args).status().await');
  });

  test('release asset download has a timeout and reports non-success statuses', () => {
    const source = updateRs();
    const timeoutCount = source.split('Duration::from_secs(60)').length - 1;
    expect(timeoutCount).toBeGreaterThanOrEqual(2);
    expect(source).toContain('download timed out');
    expect(source).toContain('download returned {}');
  });

  test('settings update buttons show explicit progress instead of ellipsis-only feedback', () => {
    const source = app();
    const translations = i18n();
    expect(source).toContain("'idle' | 'checking' | 'applying' | 'restarting' | 'error'");
    expect(source).toContain("t('settings.checkingUpdate')");
    expect(source).toContain("t('settings.installingUpdate')");
    expect(source).toContain("t('settings.restartingUpdate')");
    expect(source).toContain("setUpdateStatus('restarting')");
    expect(source).toContain("const text = await r.text().catch(() => '')");
    expect(source).toContain("parsed.detail || parsed.message || text");
    expect(translations).toContain("'settings.checkingUpdate'");
    expect(translations).toContain("'settings.installingUpdate'");
    expect(translations).toContain("'settings.restartingUpdate'");
  });
});
