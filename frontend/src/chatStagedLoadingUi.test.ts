import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('large-session staged loading UI', () => {
  test('requests the bounded latest view before deferred metadata', () => {
    const source = app();
    expect(source).toContain("if (direction === 'latest') params.set('view', 'latest');");
    expect(source).toContain("const [latestReadySessionId, setLatestReadySessionId] = useState('');");
    expect(source).toContain("setLatestReadySessionId(sessionId);");
    expect(source).toContain("if (!latestReadySessionId || latestReadySessionId !== activeSessionId) return;");
    expect(source).toContain('Promise.allSettled([');
    expect(source).toContain('loadUserMessageNav(latestReadySessionId)');
    expect(source).toContain('loadContextWindowSnapshot(latestReadySessionId)');
  });

  test('session selection does not start minimap or context before latest messages commit', () => {
    const source = app();
    const selectionStart = source.indexOf("if (!activeSessionId) return;\n    loadSessionDetail(activeSessionId);");
    const selectionEnd = source.indexOf('useEffect(() => {\n    if (watchSourceRef.current)', selectionStart);
    const selectionEffect = source.slice(selectionStart, selectionEnd);
    expect(selectionEffect).toContain("loadMessageWindow(activeSessionId, 'latest');");
    expect(selectionEffect).not.toContain('loadUserMessageNav(activeSessionId);');
    expect(selectionEffect).not.toContain('loadContextWindowSnapshot(activeSessionId);');
  });

  test('minimap ignores stale responses across A to B to A session switches', () => {
    const source = app();
    expect(source).toContain('const userNavRequestRef = useRef(0);');
    expect(source).toContain('userNavRequestRef.current += 1;');
    expect(source).toContain('contextWindowRequestRef.current += 1;');
    expect(source).toContain('const req = ++userNavRequestRef.current;');
    expect(source).toContain('req !== userNavRequestRef.current || activeSessionIdRef.current !== sessionId');
  });

  test('header total and minimap preserve their DOM shells while metadata is pending', () => {
    const source = app();
    const css = styles();
    expect(source).toContain('historyTotal: number | null;');
    expect(source).toContain('userNavLoading: boolean;');
    expect(source).toContain('className={`chat-total-count${props.historyTotal === null ? \' loading\' : \'\'}`}');
    expect(source).toContain('loading={props.userNavLoading}');
    expect(source).toContain('className={`chat-user-minimap${loading ? \' loading\' : \'\'}`}');
    expect(source).toContain('user-minimap-placeholder');
    expect(css).toContain('.chat-total-count{');
    expect(css).toContain('.chat-user-minimap.loading');
    expect(css).toContain('.user-minimap-placeholder');
  });
});
