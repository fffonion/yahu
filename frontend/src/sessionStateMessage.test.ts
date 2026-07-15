import { describe, expect, test } from 'bun:test';
import { parseSessionStateMessage } from './sessionStateMessage';

describe('session state message formatting', () => {
  test('parses the preserved task list notice and checkbox states', () => {
    expect(parseSessionStateMessage(`[Your active task list was preserved across context compression]
- [>] verify. Build and deploy (in_progress)
- [ ] ship. Commit and push (pending)
- [x] inspect. Inspect the source (completed)
- [-] obsolete. Remove old work (cancelled)`)).toEqual({
      notice: 'Your active task list was preserved across context compression',
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
    });
  });

  test('parses async delegation completion as an information notice without dropping its details', () => {
    expect(parseSessionStateMessage(`[ASYNC DELEGATION BATCH COMPLETE — deleg_41e1ea8f]
A background fan-out has finished.

--- ✗ TASK 1/1: review timed out ---`)).toEqual({
      notice: 'ASYNC DELEGATION BATCH COMPLETE — deleg_41e1ea8f',
      tasks: [],
      details: 'A background fan-out has finished.\n\n--- ✗ TASK 1/1: review timed out ---',
    });
  });

  test('leaves sender prefixes and bracketed prose to their existing renderers', () => {
    expect(parseSessionStateMessage('[Sender|123]\nhello')).toBeNull();
    expect(parseSessionStateMessage('[Note]\nordinary prose')).toBeNull();
    expect(parseSessionStateMessage('ordinary [Note] prose')).toBeNull();
    expect(parseSessionStateMessage('[ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0] human follow-up')).toBeNull();
    expect(parseSessionStateMessage(' [ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0]\ndetails')).toBeNull();
    expect(parseSessionStateMessage('\n[ASYNC DELEGATION BATCH COMPLETE -- deleg_89bc41f0]\ndetails')).toBeNull();
  });
});
