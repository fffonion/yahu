import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('stream-aware composer primary button', () => {
  test('tracks the running chat session and aborts the active stream', () => {
    const source = app();
    expect(source).toContain("const [streamingSessionId, setStreamingSessionId] = useState('');");
    expect(source).toContain('const chatAbortRef = useRef<AbortController | null>(null);');
    expect(source).toContain('const currentSessionStreaming = !!activeSessionId && streamingSessionId === activeSessionId;');
    expect(source).toContain('const stopStreaming = () => {');
    expect(source).toContain("fetch(`/chat/stream/${encodeURIComponent(activeSessionId)}/stop`, { method: 'POST', headers: headers() }).catch(() => {});");
    expect(source).toContain('chatAbortRef.current?.abort();');
    expect(source).toContain('chatAbortRef.current = controller;');
    expect(source).toContain('signal: controller.signal');
    expect(source).toContain('setStreamingSessionId(sessionId);');
  });

  test('uses one icon-only button that stops an input-free stream and sends a typed follow-up', () => {
    const source = app();
    expect(source).toContain("const primaryActionIsStop = props.streaming && !props.input.trim();");
    expect(source).toContain('onClick={primaryActionIsStop ? props.stopStreaming : props.sendMessage}');
    expect(source).toContain("aria-label={primaryActionIsStop ? t('chat.stopStreaming') : t('chat.send')}");
    expect(source).toContain('{primaryActionIsStop ? <Square /> : <ArrowUp />}');
    expect(source).not.toContain('{props.streaming && <button type="button" className="send-btn stop-stream-btn');
    expect(source).not.toContain('<Send /> <span className="btn-label">');
  });

  test('returns to stop after a streaming follow-up clears the input and to send when streaming ends', () => {
    const source = app();
    expect(source).toContain('setInput(\'\');\n      return;');
    expect(source).toContain("const primaryActionIsStop = props.streaming && !props.input.trim();");
    expect(source).toContain("setStreamingSessionId((current) => current === sessionId ? '' : current);");
  });

  test('probes the active session stream status so a remote run uses the same stop state', () => {
    const source = app();
    expect(source).toContain('/chat/stream/${encodeURIComponent(targetSessionId)}/status');
    expect(source).toContain('setStreamingSessionId(targetSessionId);');
    expect(source).toMatch(/streamingSessionIdRef\.current\s*===\s*targetSessionId/);
    expect(source).toContain("setStreamingSessionId('')");
  });

  test('primary button is a fixed icon control without obsolete two-button spacing', () => {
    const styles = css();
    expect(styles).toContain('.composer-footer .composer-primary-btn{margin-left:auto;flex:0 0 38px;width:38px;height:38px;padding:0;justify-content:center}');
    expect(styles).not.toContain('.composer-footer .stop-stream-btn');
    expect(styles).not.toContain('.stop-stream-btn+.send-btn');
  });
});
