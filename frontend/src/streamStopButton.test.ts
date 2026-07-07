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
    expect(source).toContain('const stopStreaming = () => { chatAbortRef.current?.abort(); };');
    expect(source).toContain('chatAbortRef.current = controller;');
    expect(source).toContain('signal: controller.signal');
    expect(source).toContain('setStreamingSessionId(sessionId);');
  });

  test('renders an icon-only stop button immediately to the left of send while streaming', () => {
    const source = app();
    expect(source).toContain('streaming={currentSessionStreaming}');
    expect(source).toContain('stopStreaming={stopStreaming}');
    expect(source).toContain('{props.streaming && <button type="button" className="stop-stream-btn mobile-icon-only" aria-label="Stop streaming" title="Stop streaming" onClick={props.stopStreaming}><Square /></button>}');
    expect(source).toContain('className="send-btn mobile-icon-only"');
    expect(source.indexOf('stop-stream-btn mobile-icon-only')).toBeLessThan(source.indexOf('className="send-btn mobile-icon-only"'));
    expect(source).not.toContain('<span className="btn-label">Stop</span>');
  });

  test('stop button keeps send button spacing on desktop and mobile compact composer', () => {
    const styles = css();
    expect(styles).toContain('.composer-footer .stop-stream-btn{margin-left:auto;flex:0 0 auto;width:38px;height:38px;justify-content:center;padding:0;color:var(--danger)}');
    expect(styles).toContain('.composer-footer .stop-stream-btn+.send-btn{margin-left:0}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .stop-stream-btn{margin-left:auto}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .stop-stream-btn{pointer-events:auto}');
  });
});
