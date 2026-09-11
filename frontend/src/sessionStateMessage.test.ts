import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { markdownText } from './markdown';
import { isSessionStateMessage, parseSessionStateMessage } from './sessionStateMessage';

describe('session state message formatting', () => {
  test('parses the preserved task list notice and checkbox states', () => {
    expect(parseSessionStateMessage(`[Your active task list was preserved across context compression]
- [>] verify. Build and deploy (in_progress)
- [ ] ship. Commit and push (pending)
- [x] inspect. Inspect the source (completed)
- [-] obsolete. Remove old work (cancelled)`)).toEqual({
      notice: 'Your active task list was preserved across context compression',
      collapsible: true,
      tasks: [
        { id: 'verify', description: 'Build and deploy', status: 'in_progress' },
        { id: 'ship', description: 'Commit and push', status: 'pending' },
        { id: 'inspect', description: 'Inspect the source', status: 'completed' },
        { id: 'obsolete', description: 'Remove old work', status: 'cancelled' },
      ],
    });
  });

  test('accepts a standalone bracketed information notice', () => {
    expect(parseSessionStateMessage('[Session state restored]')).toEqual({
      notice: 'Session state restored',
      tasks: [],
      collapsible: true,
    });
  });

  test('parses async delegation completion as an information notice without dropping its details', () => {
    expect(parseSessionStateMessage(`[ASYNC DELEGATION BATCH COMPLETE — deleg_41e1ea8f]
A background fan-out has finished.

--- ✗ TASK 1/1: review timed out ---`)).toEqual({
      notice: 'ASYNC DELEGATION BATCH COMPLETE — deleg_41e1ea8f',
      tasks: [],
      collapsible: true,
      details: 'A background fan-out has finished.\n\n--- ✗ TASK 1/1: review timed out ---',
    });
  });

  test('parses background process completion as a collapsed system notice', () => {
    expect(parseSessionStateMessage(`[IMPORTANT: Background process proc_6a53982948ff completed normally (exit code 0).\nCommand: set -e`)).toEqual({
      notice: 'Background process proc_6a53982948ff completed normally',
      tasks: [],
      collapsible: true,
      details: 'Command: set -e',
    });
  });

  test('parses background process completion with inline details', () => {
    expect(parseSessionStateMessage('[IMPORTANT: Background process proc_6a53982948ff completed normally (exit code 0).] Command: set -e')).toEqual({
      notice: 'Background process proc_6a53982948ff completed normally',
      tasks: [],
      collapsible: true,
      details: 'Command: set -e',
    });
  });

  test('parses context compaction as a collapsed system notice', () => {
    expect(parseSessionStateMessage('[CONTEXT COMPACTION -- REFERENCE ONLY]\nsummary from the gateway')).toEqual({
      notice: 'CONTEXT COMPACTION -- REFERENCE ONLY',
      tasks: [],
      collapsible: true,
      details: 'summary from the gateway',
    });
  });

  test('parses async delegation completion with inline details as a collapsed notice', () => {
    expect(parseSessionStateMessage('[ASYNC DELEGATION BATCH COMPLETE — deleg_09c06147] A background fan-out unit was completed.')).toEqual({
      notice: 'ASYNC DELEGATION BATCH COMPLETE — deleg_09c06147',
      tasks: [],
      collapsible: true,
      details: 'A background fan-out unit was completed.',
    });
  });

  test('collapses generic Hermes user notices with task or detail content', () => {
    expect(parseSessionStateMessage('[Session state restored]')).toMatchObject({ collapsible: true });
    expect(parseSessionStateMessage('[Your active task list was preserved across context compression]\n- [ ] check. Verify')).toMatchObject({ collapsible: true });
    expect(parseSessionStateMessage('[Continuing toward your standing goal]\n- first item')).toMatchObject({ collapsible: true });
  });

  test('renders every session-state notice through the collapsed details block', () => {
    const transcriptSource = readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
    const stylesSource = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(transcriptSource).toContain('className="session-state-message session-state-collapsible"');
    expect(transcriptSource).toContain('<summary className="session-state-summary">');
    expect(stylesSource).toContain('.session-state-collapsible[open] .session-state-arrow');
  });
  test('keeps out-of-band messages out of session-state rendering even when their body matches a notice', () => {
    expect(isSessionStateMessage({
      role: 'user',
      interrupted: true,
      content: 'ASYNC DELEGATION BATCH COMPLETE — deleg_09c06147',
    })).toBe(false);
  });

  test('formats known special notices with inline details while leaving ordinary inline prose alone', () => {
    expect(parseSessionStateMessage('[Sender|123]\nhello')).toBeNull();
    expect(parseSessionStateMessage('[Note]\nordinary prose')).toEqual({ notice: 'Note', tasks: [], collapsible: true, details: 'ordinary prose' });
    expect(parseSessionStateMessage('ordinary [Note] prose')).toBeNull();
    expect(parseSessionStateMessage('[ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0] human follow-up')).toMatchObject({
      notice: 'ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0',
      collapsible: true,
      details: 'human follow-up',
    });
    expect(parseSessionStateMessage(' [ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0]\ndetails')).toMatchObject({ collapsible: true, details: 'details' });
    expect(parseSessionStateMessage('\n[ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0]\ndetails')).toMatchObject({ collapsible: true, details: 'details' });
  });

  test('folds aggregate background and system notices without a closing bracket on the first line', () => {
    expect(parseSessionStateMessage('[IMPORTANT: 2 background processes completed for this session.\nTreat these results as one completion batch.')).toEqual({
      notice: 'IMPORTANT: 2 background processes completed for this session.',
      tasks: [],
      collapsible: true,
      details: 'Treat these results as one completion batch.',
    });
    expect(parseSessionStateMessage('[IMPORTANT: 2 background subagent delegations completed for this session. Treat these results as one completion batch and send at most one consolidated user-facing response.\n\n[ASYNC DELEGATION BATCH COMPLETE — deleg_1dcc5fe4]')).toMatchObject({
      notice: 'IMPORTANT: 2 background subagent delegations completed for this session. Treat these results as one completion batch and send at most one consolidated user-facing response.',
      collapsible: true,
    });
    expect(parseSessionStateMessage("[System note: A new message has arrived. The conversation history contains pending tool outputs from an interrupted turn.\n\n[Alliumcepa Triplef|1698432746]\n继续")).toMatchObject({
      notice: 'System note: A new message has arrived. The conversation history contains pending tool outputs from an interrupted turn.',
      collapsible: true,
    });
  });

  test('formats the standing-goal notice and preserves the following markdown list', () => {
    const parsed = parseSessionStateMessage('[Continuing toward your standing goal]\n- first item\n- second item');
    expect(parsed).toEqual({
      notice: 'Continuing toward your standing goal',
      tasks: [],
      collapsible: true,
      details: '- first item\n- second item',
    });
    expect(markdownText(parsed?.details || '')).toContain('<ul><li>first item</li><li>second item</li></ul>');
  });
});
