import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { mergeMessageWindow } from './chatMessageWindow';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

type Msg = { id: string };

const ids = (messages: Msg[]) => messages.map((message) => message.id);

describe('chat message window pagination state', () => {
  test('latest history keeps watch updates that arrived while the request was in flight', () => {
    type LiveMsg = Msg & { content?: string };
    const result = mergeMessageWindow<LiveMsg>({
      current: [{ id: '21', content: 'live update' }, { id: '22', content: 'new tool row' }],
      chunk: [{ id: '20', content: 'history' }, { id: '21', content: 'stale skeleton' }],
      direction: 'latest',
      limit: 10,
      hasOlder: false,
      hasNewer: false,
      pageHasOlder: true,
      pageHasNewer: false,
    });

    expect(ids(result.messages)).toEqual(['20', '21', '22']);
    expect(result.messages[1]?.content).toBe('live update');
    expect(result.hasOlder).toBe(true);
    expect(result.hasNewer).toBe(false);
  });

  test('latest refresh keeps cached older messages before the fetched latest chunk', () => {
    const result = mergeMessageWindow<Msg>({
      current: [{ id: '10' }, { id: '11' }, { id: '12' }, { id: '13' }],
      chunk: [{ id: '12' }, { id: '13' }],
      direction: 'latest',
      limit: 10,
      hasOlder: true,
      hasNewer: false,
      pageHasOlder: true,
      pageHasNewer: false,
    });

    expect(ids(result.messages)).toEqual(['10', '11', '12', '13']);
  });

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

  test('latest history response merges with watch updates instead of replacing them', () => {
    const source = appSource();
    expect(source).toContain('current: messagesRef.current,');

  });

  test('app ignores stale watch events from a previous active session', () => {
    const source = appSource();
    expect(source).toContain('if (activeSessionIdRef.current !== watchedSessionId) return;');
  });
});
