import { describe, expect, test } from 'bun:test';
import { buildCronPatch, cronEditableValues } from './cronEditor';

describe('cron editor helpers', () => {
  test('extracts editable values from cron job objects', () => {
    expect(cronEditableValues({ name: 'job', schedule: { display: '0 9 * * *' }, prompt: 'do it', script: 'x.py', deliver: 'telegram' })).toEqual({
      name: 'job', schedule: '0 9 * * *', prompt: 'do it', script: 'x.py', deliver: 'telegram',
    });
  });

  test('uses PATCH body shape accepted by the Hermes API server', () => {
    expect(buildCronPatch({ name: 'job', schedule: '0 9 * * *', prompt: 'do it', script: '', deliver: 'telegram' })).toEqual({
      name: 'job', schedule: '0 9 * * *', prompt: 'do it', script: null, deliver: 'telegram',
    });
  });
});
