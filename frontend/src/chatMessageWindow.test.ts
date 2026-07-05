import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { mergeMessageWindow } from './chatMessageWindow';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

type Msg = { id: string };

const ids = (messages: Msg[]) => messages.map((message) => message.id);

describe('chat message window pagination state', () => {
  test('prepending older history keeps newer=false when the current tail is still loaded', () => {
    const result = mergeMessageWindow<Msg>({
      current: [{ id: '30' }, { id: '31' }],
      chunk: [{ id: '10' }, { id: '11' }],
      direction: 'older',
      limit: 10,
      hasOlder: true,
      hasNewer: false,
      pageHasOlder: false,
      pageHasNewer: true,
    });

    expect(ids(result.messages)).toEqual(['10', '11', '30', '31']);
    expect(result.hasOlder).toBe(false);
    expect(result.hasNewer).toBe(false);
  });

  test('prepending older history marks newer=true only when trimming the newest tail', () => {
    const result = mergeMessageWindow<Msg>({
      current: [{ id: '30' }, { id: '31' }, { id: '32' }],
      chunk: [{ id: '10' }, { id: '11' }],
      direction: 'older',
      limit: 4,
      hasOlder: true,
      hasNewer: false,
      pageHasOlder: false,
      pageHasNewer: true,
    });

    expect(ids(result.messages)).toEqual(['10', '11', '30', '31']);
    expect(result.hasOlder).toBe(false);
    expect(result.hasNewer).toBe(true);
  });

  test('duplicate older page stops older pagination instead of repeating the same request', () => {
    const result = mergeMessageWindow<Msg>({
      current: [{ id: '10' }, { id: '11' }],
      chunk: [{ id: '10' }, { id: '11' }],
      direction: 'older',
      limit: 10,
      hasOlder: true,
      hasNewer: false,
      pageHasOlder: true,
      pageHasNewer: true,
    });

    expect(ids(result.messages)).toEqual(['10', '11']);
    expect(result.hasOlder).toBe(false);
    expect(result.hasNewer).toBe(false);
  });

  test('duplicate newer page stops newer pagination instead of bouncing windows', () => {
    const result = mergeMessageWindow<Msg>({
      current: [{ id: '10' }, { id: '11' }],
      chunk: [{ id: '10' }, { id: '11' }],
      direction: 'newer',
      limit: 10,
      hasOlder: false,
      hasNewer: true,
      pageHasOlder: true,
      pageHasNewer: true,
    });

    expect(ids(result.messages)).toEqual(['10', '11']);
    expect(result.hasOlder).toBe(false);
    expect(result.hasNewer).toBe(false);
  });

  test('app ignores stale watch events from a previous active session', () => {
    const source = appSource();
    expect(source).toContain('if (activeSessionIdRef.current !== watchedSessionId) return;');
  });
});
