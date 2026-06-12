import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parsePlatformSenderMessage } from './chatSender';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('platform sender labels in chat bubbles', () => {
  test('parses a platform sender prefix and removes it from visible message content', () => {
    expect(parsePlatformSenderMessage('[Alliumcepa Triplef|1698432746]\nhello')).toEqual({
      senderName: 'Alliumcepa Triplef',
      senderId: '1698432746',
      content: 'hello',
    });
    expect(parsePlatformSenderMessage('[Alliumcepa Triplef|1698432746] hello')).toEqual({
      senderName: 'Alliumcepa Triplef',
      senderId: '1698432746',
      content: 'hello',
    });
  });

  test('leaves normal bracketed text alone', () => {
    expect(parsePlatformSenderMessage('[not a sender] hello')).toEqual({ content: '[not a sender] hello' });
    expect(parsePlatformSenderMessage('prefix [Allium|123]\nhello')).toEqual({ content: 'prefix [Allium|123]\nhello' });
  });

  test('message rendering uses platform sender name plus small muted id instead of You', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("platformSenderName?: string; platformSenderId?: string");
    expect(source).toContain('const senderLabel = messageSenderLabel(message, assistantName);');
    expect(source).toContain('className="msg-sender-name"');
    expect(source).toContain('className="msg-sender-id"');
    expect(source).toContain('parsePlatformSenderMessage(parts.content)');
    expect(styles).toContain('.msg-sender-id{font-size:11px;color:var(--muted);font-weight:400;margin-left:4px}');
  });
});
