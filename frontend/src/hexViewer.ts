const BYTES_PER_ROW = 16;
const HALF_ROW = BYTES_PER_ROW / 2;
const HEX_GROUP_WIDTH = HALF_ROW * 3 - 1;

const byteHex = (value: number) => value.toString(16).padStart(2, '0');
const printableAscii = (value: number) => value >= 0x20 && value <= 0x7e ? String.fromCharCode(value) : '.';

export function formatHexDump(bytes: Uint8Array): string {
  const rows: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += BYTES_PER_ROW) {
    const row = bytes.subarray(offset, offset + BYTES_PER_ROW);
    const first = Array.from(row.subarray(0, HALF_ROW), byteHex).join(' ').padEnd(HEX_GROUP_WIDTH);
    const second = Array.from(row.subarray(HALF_ROW), byteHex).join(' ').padEnd(HEX_GROUP_WIDTH);
    const ascii = Array.from(row, printableAscii).join('');
    rows.push(`${offset.toString(16).padStart(8, '0')}  ${first}  ${second}  |${ascii}|`);
  }
  return rows.join('\n');
}
