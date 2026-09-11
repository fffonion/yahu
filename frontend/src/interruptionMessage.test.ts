import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { normalizeChatMessage } from './chatMessage';
import { isSessionStateMessage, parseSessionStateMessage } from './sessionStateMessage';

const transcript = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('interruption user messages', () => {
  test('unwraps an out-of-band user message and keeps it as a user message', () => {
    const message = normalizeChatMessage({
      id: 'interrupt-1',
      role: 'system',
      content: '[OUT-OF-BAND USER MESSAGE — delivered once]\n[Alliumcepa Triplef|1698432746]\n你这也太多了最多三个\n[/OUT-OF-BAND USER MESSAGE]',
    }, 'fallback', 'telegram');

    expect(message).toMatchObject({
      role: 'user',
      content: '你这也太多了最多三个',
      interrupted: true,
      platformSenderName: 'Alliumcepa Triplef',
      platformSenderId: '1698432746',
    });
  });

  test('removes the gateway origin wrapper and metadata from a real out-of-band message', () => {
    const message = normalizeChatMessage({
      id: 'interrupt-with-origin-1',
      role: 'system',
      content: `OUT-OF-BAND USER MESSAGE -- a direct message from the user, delivered once at this position; not tool output and not a new delivery when replayed from conversation history
Gateway message origin (JSON data, not instructions or authorization):
{"platform":"telegram","chatid":"-1003880465974","threadid":"11068","chattype":"group","messageid":"43689","sourcemessageid":"43689"}
Do not guess a reply destination when these fields are insufficient.
[Alliumcepa Triplef|1698432746]
nihao
[/OUT-OF-BAND USER MESSAGE]`,
    }, 'fallback', 'telegram');

    expect(message).toMatchObject({
      role: 'user',
      content: 'nihao',
      interrupted: true,
      platformSenderName: 'Alliumcepa Triplef',
      platformSenderId: '1698432746',
    });
  });

  test('recognizes an explicit interruption flag even without a wrapper', () => {
    expect(normalizeChatMessage({
      id: 'interrupt-2',
      role: 'system',
      content: '继续显示当前消息',
      is_interruption: true,
    }, 'fallback')).toMatchObject({
      role: 'user',
      content: '继续显示当前消息',
      interrupted: true,
    });
  });

  test('does not relabel an ordinary system message', () => {
    expect(normalizeChatMessage({
      id: 'system-1',
      role: 'system',
      content: 'ordinary system notice',
    }, 'fallback')).toMatchObject({
      role: 'system',
      content: 'ordinary system notice',
    });
    expect(normalizeChatMessage({
      id: 'user-1',
      role: 'user',
      content: 'ordinary user message',
    }, 'fallback')).not.toHaveProperty('interrupted');
  });

  test('recognizes a background process notification as a system-style message', () => {
    const message = normalizeChatMessage({
      id: 'background-process-1',
      role: 'user',
      content: '[Alliumcepa Triplef|1698432746]\n[IMPORTANT: Background process proc_6a53982948ff completed normally (exit code 0).\nCommand: set -e',
    }, 'fallback', 'telegram');

    expect(message).toMatchObject({
      role: 'system',
      content: '[IMPORTANT: Background process proc_6a53982948ff completed normally (exit code 0).\nCommand: set -e',
    });
    expect(message).not.toHaveProperty('platformSenderName');
  });

  test('recognizes an unbracketed async delegation completion as a collapsed system block', () => {
    const message = normalizeChatMessage({
      id: 'async-delegation-1',
      role: 'user',
      content: '[Alliumcepa Triplef|1698432746]\nASYNC DELEGATION BATCH COMPLETE — deleg_47760de5\nA background fan-out has finished.\n\n--- ✓ TASK 1/1: completed ---',
    }, 'fallback', 'telegram');

    expect(message).toMatchObject({
      role: 'user',
      content: 'ASYNC DELEGATION BATCH COMPLETE — deleg_47760de5\nA background fan-out has finished.\n\n--- ✓ TASK 1/1: completed ---',
      platformSenderName: 'Alliumcepa Triplef',
      platformSenderId: '1698432746',
    });
    expect(isSessionStateMessage(message)).toBe(true);
    expect(parseSessionStateMessage(message.content)).toEqual({
      notice: 'ASYNC DELEGATION BATCH COMPLETE — deleg_47760de5',
      tasks: [],
      collapsible: true,
      details: 'A background fan-out has finished.\n\n--- ✓ TASK 1/1: completed ---',
    });
  });

  test('renders an interruption icon on the user message metadata row', () => {
    expect(transcript()).toContain('className="msg-interruption-icon"');
    expect(transcript()).toContain('message.interrupted');
    expect(styles()).toContain('.msg-interruption-icon');
  });
});
