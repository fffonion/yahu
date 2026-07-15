import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const card = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('subagent progress UI', () => {
  test('renders the websocket-backed progress card inside the chat stream', () => {
    expect(app()).toContain("import { SubagentProgressCard } from './SubagentProgressCard';");
    expect(app()).toContain('<SubagentProgressCard sessionId={props.activeSessionId} />');
    expect(card()).toContain('new WebSocket(subagentWebSocketUrl(window.location, sessionId))');
    expect(card()).toContain('normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId)');
  });

  test('shows nested agents, counters, todo progress, activity, and completion summary', () => {
    const source = card();
    expect(source).toContain('className="subagent-progress-tree"');
    expect(source).toContain('className="subagent-progress-stats"');
    expect(source).toContain('className="subagent-progress-todos"');
    expect(source).toContain('className="subagent-progress-activity"');
    expect(source).toContain('className="subagent-progress-summary"');
    expect(source).toContain('<SubagentProgressNode key={child.sessionId}');
  });

  test('uses a responsive card that remains readable in compact desktop and mobile chat', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-card{');
    expect(styles).toContain('.subagent-progress-node summary{');
    expect(styles).toContain('@media(max-width:760px){.subagent-progress-card');
    expect(styles).toContain('.desktop-compact-chat .subagent-progress-card');
  });
});
