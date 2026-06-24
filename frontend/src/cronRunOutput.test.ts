import { describe, expect, test } from 'bun:test';
import { waitForCronRunOutput } from './cronRunOutput';

describe('waitForCronRunOutput', () => {
  test('polls until the manual run has a newer saved output', async () => {
    const responses = [
      null,
      { timestamp: '2026-06-25_05-37-29', content: 'old output' },
      { timestamp: '2026-06-25_05-38-04', content: 'new output' },
    ];
    let calls = 0;

    const output = await waitForCronRunOutput(
      async () => responses[calls++] ?? null,
      '2026-06-25_05-37-29',
      { attempts: 5, delayMs: 0 },
    );

    expect(calls).toBe(3);
    expect(output?.content).toBe('new output');
  });

  test('keeps polling when a just-triggered job has not written output yet', async () => {
    const responses = [null, null, { timestamp: '2026-06-25_05-38-04', content: 'created output' }];
    let calls = 0;

    const output = await waitForCronRunOutput(
      async () => responses[calls++] ?? null,
      '',
      { attempts: 5, delayMs: 0 },
    );

    expect(calls).toBe(3);
    expect(output?.content).toBe('created output');
  });
});
