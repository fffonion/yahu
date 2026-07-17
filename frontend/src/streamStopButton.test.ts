import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('stream stop button', () => {
  test('tracks the running chat session and aborts the active stream', () => {
    const source = app();
    expect(source).toContain('const [streamingSessionId, setStreamingSessionId] = useState(\'\');');
    expect(source).toContain('const chatAbortRef = useRef<AbortController | null>(null);');
    expect(source).toContain('const currentSessionStreaming = !!activeSessionId && streamingSessionId === activeSessionId;');
    expect(source).toContain('const stopStreaming = () => {');
    expect(source).toContain('fetch(`/chat/stream/${encodeURIComponent(activeSessionId)}/stop`, { method: \'POST\', headers: headers() }).catch(() => {});');
    expect(source).toContain('chatAbortRef.current?.abort();');
    expect(source).toContain('chatAbortRef.current = controller;');
    expect(source).toContain('signal: controller.signal');
    expect(source).toContain('setStreamingSessionId(sessionId);');
  });

  test('renders an icon-only stop button immediately to the left of send while streaming', () => {
    const source = app();
    expect(source).toContain('streaming={currentSessionStreaming}');
    expect(source).toContain('stopStreaming={stopStreaming}');
    expect(source).toContain('{props.streaming && <button type="button" className="send-btn stop-stream-btn mobile-icon-only" aria-label={t(\'chat.stopStreaming\')} title={t(\'chat.stopStreaming\')} onClick={props.stopStreaming}><Square /></button>}');
    expect(source).toContain('className="send-btn mobile-icon-only"');
    expect(source.indexOf('send-btn stop-stream-btn mobile-icon-only')).toBeLessThan(source.indexOf('className="send-btn mobile-icon-only"'));
    expect(source).not.toContain('<span className="btn-label">Stop</span>');
  });

  test('send button shows the queue label while streaming, including when another platform is driving the run', () => {
    const source = app();
    expect(source).toContain("props.streaming ? t('chat.queue') : t('chat.send')");
    expect(source).not.toContain("props.busy ? t('chat.queue') : t('chat.send')");
    expect(source).toContain("props.streaming ? t('chat.queueFollowUp') : t('chat.send')");
  });

  test('probes the active session stream status so attaching to a remote run shows the stop button', () => {
    const source = app();
    expect(source).toContain("/chat/stream/${encodeURIComponent(targetSessionId)}/status");
    expect(source).toContain('setStreamingSessionId(targetSessionId);');
    expect(source).toMatch(/streamingSessionIdRef\.current\s*===\s*targetSessionId/);
    expect(source).toContain("setStreamingSessionId('')");
  });

  test('stop button shares send styling and stays adjacent to send on desktop and mobile compact composer', () => {
    const styles = css();
    expect(styles).toContain('.composer-footer .stop-stream-btn{margin-left:auto;flex:0 0 auto;justify-content:center}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .stop-stream-btn+.send-btn{margin-left:0}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .stop-stream-btn+.send-btn{margin-left:0}');
    expect(styles.lastIndexOf('.composer-wrap:not(.composer-compact) .composer-footer .stop-stream-btn+.send-btn{margin-left:0}')).toBeGreaterThan(styles.lastIndexOf('.composer-wrap:not(.composer-compact) .composer-footer .send-btn{margin-left:auto}'));
    expect(styles).not.toContain('stop-stream-btn{margin-left:auto;flex:0 0 auto;width:38px;height:38px;justify-content:center;padding:0;color:var(--danger)}');
  });
});
