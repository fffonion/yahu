import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
function cssRule(styles: string, selector: string) {
  const start = styles.indexOf(`${selector}{`);
  if (start < 0) return '';
  const end = styles.indexOf('}', start);
  return end >= 0 ? styles.slice(start, end + 1) : '';
}

describe('chat reasoning display toggle', () => {
  test('composer has an icon-only quick toggle for reasoning/thinking visibility', () => {
    const source = app();
    expect(source).toContain("showReasoning={showReasoning}");
    expect(source).toContain("setShowReasoning={setShowReasoning}");
    expect(source).toContain("className={`icon-btn composer-view-toggle reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`}");
    expect(source).toContain("aria-pressed={props.showReasoning}");
    expect(source).toContain('<Lightbulb /></button>');
    expect(source).not.toContain('reasoning-view-toggle ${props.showReasoning ? \'active\' : \'\'}`} aria-pressed={props.showReasoning} aria-label={props.showReasoning ? t(\'chat.hideThinking\') : t(\'chat.showThinking\')} title={props.showReasoning ? t(\'chat.hideThinking\') : t(\'chat.showThinking\')} onClick={() => props.setShowReasoning(!props.showReasoning)}><Eye /></button>');
  });

  test('assistant messages render reasoning only when the toggle is enabled', () => {
    const source = app();
    expect(source).toContain("<MessageView message={m} showReasoning={props.showReasoning} assistantName={sessionModel || undefined} />");
    expect(source).toContain("message.reasoning && showReasoning");
    expect(source).toContain("className=\"msg-reasoning\"");
  });

  test('composer has a session-only tool-call visibility toggle that defaults visible', () => {
    const source = app();
    const i18n = readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
    expect(source).toContain('const [showToolCalls, setShowToolCalls] = useState(true);');
    expect(source).not.toContain('SHOW_TOOL_CALLS_KEY');
    expect(source).not.toContain('localStorage.setItem(SHOW_TOOL_CALLS_KEY');
    expect(source).toContain("showToolCalls={showToolCalls}");
    expect(source).toContain("setShowToolCalls={setShowToolCalls}");
    expect(source).toContain("className={`icon-btn composer-view-toggle tool-call-view-toggle ${props.showToolCalls ? 'active' : ''}`}");
    expect(source).toContain("aria-pressed={props.showToolCalls}");
    expect(source).toContain('<Terminal /></button>');
    expect(i18n).toContain("'chat.showToolCalls'");
    expect(i18n).toContain("'chat.hideToolCalls'");
  });

  test('assistant message title uses the responding model name instead of generic Hermes Agent', () => {
    const source = app();
    expect(source).toContain("type ChatMessage = { id: string; role: Role; content: string; reasoning?: string; timestamp?: string | number; pending?: boolean; toolName?: string; toolInput?: unknown; toolCalls?: unknown; tokenCount?: number; model?: string; provider?: string; platformSenderName?: string; platformSenderId?: string };");
    expect(source).toContain("function messageRoleName(message: ChatMessage, assistantName?: string) { return message.role === 'assistant' ? (message.model || assistantName || 'Hermes Agent') : roleName(message.role); }");
    expect(source).toContain("const assistantMsg: ChatMessage = { id: assistantId, role: 'assistant', content: '', pending: true, model: sessionModel, provider: sessionProvider };");
    expect(source).toContain('const senderLabel = messageSenderLabel(message, assistantName);');
    expect(source).toContain('className="msg-sender-name"');
  });

  test('reasoning block and toggle have compact themed styles', () => {
    const styles = css();
    expect(styles).toContain('.composer-view-toggle{box-sizing:border-box;width:38px;height:38px;min-width:38px;max-width:38px;justify-content:center;padding:0;flex:0 0 38px}');
    expect(styles).toContain('.reasoning-view-toggle.active,.tool-call-view-toggle.active');
    expect(styles).toContain('.msg-reasoning');
    expect(styles).toContain('.msg-reasoning pre');
    expect(cssRule(styles, '.msg-reasoning')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning>span')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning pre')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning')).toContain('var(--surface-2)');
  });
});
