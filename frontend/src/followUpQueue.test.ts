import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat follow-up queue and steer behaviour', () => {
  test('defaults follow-up behaviour to browser-side queue and persists the setting', () => {
    const source = app();
    expect(source).toContain("const FOLLOW_UP_BEHAVIOUR_KEY = 'followUpBehaviour'");
    expect(source).toContain("const [followUpBehaviour, setFollowUpBehaviour] = useState<FollowUpBehaviour>(() => normalizeFollowUpBehaviour(localStorage.getItem(FOLLOW_UP_BEHAVIOUR_KEY)))");
    expect(source).toContain("localStorage.setItem(FOLLOW_UP_BEHAVIOUR_KEY, followUpBehaviour)");
    expect(source).toContain('followUpBehaviour={followUpBehaviour}');
    expect(source).toContain('setFollowUpBehaviour={setFollowUpBehaviour}');
  });

  test('sending while the current session streams keeps the composer enabled and queues or steers instead of returning early', () => {
    const source = app();
    expect(source).toContain('if (currentSessionStreaming || remoteSessionStreaming) {');
    expect(source).toContain("if (followUpBehaviour === 'steer')");
    expect(source).toContain('await steerFollowUp(text);');
    expect(source).toContain('enqueueFollowUp(text, sessionId);');

  });

  test('sending waits for an attach-time stream probe before choosing queue or a new turn', () => {
    const source = app();
    expect(source).toContain("const streamStatusProbeRef = useRef<{ sessionId: string; promise: Promise<boolean | null> } | null>(null);");
    expect(source).toContain('const pendingStreamProbe = streamStatusProbeRef.current;');
    expect(source).toContain('const remoteSessionStreaming = pendingStreamProbe?.sessionId === sessionId && await pendingStreamProbe.promise === true;');
    expect(source).toContain('if (currentSessionStreaming || remoteSessionStreaming) {');
    expect(source.indexOf('const remoteSessionStreaming = pendingStreamProbe')).toBeLessThan(source.indexOf('await runChatTurn(text, attachments);'));
  });

  test('queue items are stored in localStorage per session and render above the composer with controls', () => {
    const source = app();
    expect(source).toContain("localStorage.setItem(FOLLOW_UP_QUEUES_KEY, JSON.stringify(next))");
    expect(source).toContain('const followUpQueue = followUpQueues[followUpQueueKey(activeSessionId)] || []');
    expect(source).toContain('followUpQueue={followUpQueue}');
    expect(source).toContain('onSteerQueuedItem={steerQueuedItem}');
    expect(source).toContain('onEditQueuedItem={editQueuedItem}');
    expect(source).toContain('onReorderQueuedItem={reorderQueuedItem}');
    expect(source).toContain('className="followup-queue"');
    expect(source).toContain('className="followup-text"');
    expect(source).toContain('className="followup-drag-handle"');
    expect(source).toContain('draggable={true}');
    expect(source).toContain('<GripVertical />');
  });

  test('steer mode posts a slash steer prompt through the session chat endpoint without opening a second stream reader', () => {
    const source = app();
    expect(source).toContain('const steerFollowUp = async (text: string) =>');
    expect(source).toContain('buildChatRequestBody(`/steer ${text}`, sessionModel, effort, sessionProvider)');
    expect(source).toContain('/chat`), { method: \'POST\'');

  });

  test('settings exposes follow-up behaviour choice', () => {
    const source = app();
    expect(source).toContain('followUpBehaviour: FollowUpBehaviour');
    expect(source).toContain('<option value="queue">{t(\'chat.queue\')}</option>');
    expect(source).toContain('<option value="steer">{t(\'chat.steer\')}</option>');
    expect(source).toContain('settings.followUpBehaviour');
  });

  test('queue chips are single-line and truncate long text', () => {
    const styles = css();
    expect(styles).toContain('.followup-queue{display:grid;gap:6px;padding:8px 12px 0;background:var(--surface)}');
    expect(styles).toContain('.followup-text{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text)');
    expect(styles).toContain('.followup-item{display:grid;grid-template-columns:auto minmax(0,1fr) auto auto;align-items:center;gap:6px;min-height:34px');
  });
});
