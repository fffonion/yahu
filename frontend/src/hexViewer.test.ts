import { describe, expect, test } from 'bun:test';
import { formatHexDump } from './hexViewer';

describe('workspace hex viewer formatting', () => {
  test('formats bytes as offset, grouped hexadecimal values, and printable ASCII', () => {
    const bytes = Uint8Array.from([0x00, 0x01, 0x0f, 0x10, 0x1f, 0x20, 0x41, 0x7e, 0x7f, 0xff]);

    expect(formatHexDump(bytes)).toBe(
      '00000000  00 01 0f 10 1f 20 41 7e  7f ff                    |..... A~..|',
    );
  });

  test('starts each sixteen-byte row at its hexadecimal offset', () => {
    const dump = formatHexDump(Uint8Array.from({ length: 17 }, (_, index) => index));

    expect(dump.split('\n')[1]).toStartWith('00000010  10');
  });
});
