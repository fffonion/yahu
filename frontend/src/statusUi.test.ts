import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('ephemeral status UI', () => {
  test('does not render legacy global status text in page headers or composer footer', () => {
    const app = source();
    const css = styles();

    expect(app).not.toContain('className="status-chip"');
    expect(app).not.toContain('<span>{props.status}</span>');
    expect(app).not.toContain('skill?.description || status');
    expect(app).not.toContain('status={status}');
    expect(css).not.toContain('status-chip');
  });

  test('renders a bottom center toast that automatically disappears after successful actions', () => {
    const app = source();
    const css = styles();

    expect(app).toContain('function StatusToast({ message }: { message: string })');
    expect(app).toContain('<StatusToast message={toastMessage} />');
    expect(app).toContain('const showToast = useCallback((message: string) => {');
    expect(app).toContain('toastTimerRef.current = window.setTimeout(() => setToastMessage(\'\'), 2600);');
    expect(app).toContain('showToast(t(\'memory.saved\'))');
    expect(app).toContain("if (document.activeElement instanceof HTMLElement) document.activeElement.blur();");
    expect(app).toContain('showToast(t(\'cron.saved\'))');
    expect(app).toContain('showToast(t(\'cron.ran\'))');
    expect(app).toContain('showToast(t(\'settings.saved\'))');
    expect(css).toContain('.status-toast{position:fixed;left:50%;bottom:24px;transform:translateX(-50%);');
    expect(css).toContain('@media (max-width:760px){.status-toast{bottom:calc(var(--mobile-bottom-nav-height) + 14px + env(safe-area-inset-bottom,0px));');
  });
});
