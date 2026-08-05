import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => [readFileSync(new URL('./App.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./chatMessage.ts', import.meta.url), 'utf8')].join('\n');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
function cssRule(styles: string, selector: string) {
  const start = styles.indexOf(`${selector}{`);
  if (start < 0) return '';
  const end = styles.indexOf('}', start);
  return end >= 0 ? styles.slice(start, end + 1) : '';
}

describe('chat reasoning display toggle', () => {
  test('history shows reasoning by default and the toggle remains session-only', () => {
    const source = app();
    expect(source).toContain('const [showReasoning, setShowReasoning] = useState(true);');
    expect(source).not.toContain('SHOW_REASONING_KEY');
    expect(source).not.toContain('localStorage.setItem(SHOW_REASONING_KEY');
  });

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
    expect(source).toContain("<MessageView message={item.message} showReasoning={showReasoning} assistantName={assistantName} />");
    expect(source).toContain("message.reasoning && showReasoning");
    expect(source).toContain("<details className=\"msg-reasoning msg-reasoning-collapsed\"");
    expect(source).not.toContain("<details open className=\"msg-reasoning msg-reasoning-collapsed\"");
  });

  test('assistant reasoning is rendered before the response body', () => {
    const source = app();
    const reasoningIndex = source.indexOf('{message.reasoning && showReasoning && <details className="msg-reasoning msg-reasoning-collapsed"');
    const responseBodyIndex = source.indexOf('<div className="msg-body">', reasoningIndex);
    expect(reasoningIndex).toBeGreaterThan(-1);
    expect(responseBodyIndex).toBeGreaterThan(reasoningIndex);
  });

  test('desktop reasoning disclosure leaves space before the following response body', () => {
    const styles = css();
    expect(styles).toContain('@media(min-width:761px){.msg-reasoning{margin-bottom:8px}}');
    expect(styles).not.toContain('@media(max-width:760px){.msg-reasoning{margin-bottom:8px}}');
  });

  test('pre-tool assistant text stays visible but its thinking block summary says completed thinking with elapsed time', () => {
    const source = app();
    const styles = css();
    const i18n = readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
    expect(source).toContain('function reasoningSummaryLabel(message: ChatMessage): string');
    expect(source).toContain("const reasoningSummary = reasoningSummaryLabel(message);");
    expect(source).toContain("<details className=\"msg-reasoning msg-reasoning-collapsed\" aria-label={reasoningSummary}><summary><span className=\"reasoning-chevron\"><ChevronRight /></span> <span>{reasoningSummary}</span></summary><pre>{message.reasoning}</pre></details>");
    expect(source).not.toContain("<span>{t('chat.details')}</span></summary><pre>{message.reasoning}</pre>");
    expect(source).not.toContain(">Thinking<");
    expect(source).not.toContain("aria-label=\"Reasoning / thinking\"");
    expect(source).not.toContain("<section className=\"msg-reasoning\"");
    expect(i18n).toContain("'chat.reasoned'");
    expect(i18n).toContain("'zh-CN': '已思考'");
    expect(styles).toContain('.msg-reasoning>summary');
    expect(styles).toContain('.reasoning-chevron{display:inline-grid;place-items:center;width:15px;height:15px;color:var(--muted);transition:transform .15s ease;flex:0 0 auto}');
    expect(styles).toContain('.reasoning-chevron svg{width:15px;height:15px}');
    expect(styles).toContain('.msg-reasoning[open] .reasoning-chevron{transform:rotate(90deg)}');
    expect(styles).toContain('.msg-reasoning-collapsed:not([open]) pre{display:none}');
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
    expect(source).toContain('structuredContent?: { value: unknown }');
    expect(source).toContain('model?: string; provider?: string;');
    expect(source).toContain("function messageRoleName(message: ChatMessage, assistantName?: string) { return message.role === 'assistant' ? (message.model || assistantName || 'Hermes Agent') : roleName(message.role); }");
    expect(source).toContain("const assistantMsg: ChatMessage = { id: assistantId, role: 'assistant', content: '', pending: true, timestamp: Date.now() / 1000, model: sessionModel, provider: sessionProvider };");
    expect(source).toContain('const senderLabel = messageSenderLabel(message, assistantName);');
    expect(source).toContain('className="msg-sender-name"');
  });

  test('reasoning frame uses the small interface radius', () => {
    const styles = css();
    const originalRadius = styles.lastIndexOf('.msg-reasoning{margin-top:10px;');
    const reducedRadius = styles.lastIndexOf('.msg-reasoning{border-radius:var(--radius-sm)}');
    expect(originalRadius).toBeGreaterThan(-1);
    expect(reducedRadius).toBeGreaterThan(originalRadius);
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
