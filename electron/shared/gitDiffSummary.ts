export interface GitDiffSummary {
  status: 'ready' | 'error';
  addedLines: number;
  removedLines: number;
  error?: string;
}

type TextLineEncoding = 'bytes' | 'utf16le' | 'utf16be';

export function parseGitNumstatLine(line: string): { added: number; removed: number } {
  const columns = line.split('\t', 3);
  if (columns.length < 2) return { added: 0, removed: 0 };

  const added = Number.parseInt(columns[0], 10);
  const removed = Number.parseInt(columns[1], 10);
  return {
    added: Number.isFinite(added) ? added : 0,
    removed: Number.isFinite(removed) ? removed : 0,
  };
}

export function parseGitNumstatTotals(stdout: string): { addedLines: number; removedLines: number } {
  let addedLines = 0;
  let removedLines = 0;

  for (const line of stdout.split(/\r?\n/)) {
    if (!line) continue;
    const parsed = parseGitNumstatLine(line);
    addedLines += parsed.added;
    removedLines += parsed.removed;
  }

  return { addedLines, removedLines };
}

export function parseGitPathList(stdout: string): string[] {
  return stdout
    .split('\0')
    .filter((path) => path.length > 0);
}

export function countTextLineBytes(bytes: Uint8Array): number | undefined {
  const detected = detectTextLineEncoding(bytes);
  if (!detected) return undefined;

  const data = bytes.subarray(detected.offset);
  if (detected.encoding === 'bytes') {
    return countByteLines(data, 0x0a);
  }
  return countUtf16Lines(data, detected.encoding === 'utf16le');
}

function detectTextLineEncoding(bytes: Uint8Array): { encoding: TextLineEncoding; offset: number } | undefined {
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return { encoding: 'utf16le', offset: 2 };
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    return { encoding: 'utf16be', offset: 2 };
  }
  if (bytes.includes(0)) {
    const encoding = detectBomlessUtf16Encoding(bytes);
    return encoding ? { encoding, offset: 0 } : undefined;
  }
  return { encoding: 'bytes', offset: 0 };
}

function detectBomlessUtf16Encoding(bytes: Uint8Array): TextLineEncoding | undefined {
  if (bytes.length < 2 || bytes.length % 2 !== 0) {
    return undefined;
  }

  let littleEndianPairs = 0;
  let bigEndianPairs = 0;
  for (let index = 0; index < bytes.length; index += 2) {
    const firstIsNul = bytes[index] === 0;
    const secondIsNul = bytes[index + 1] === 0;
    if (!firstIsNul && secondIsNul) {
      littleEndianPairs += 1;
    } else if (firstIsNul && !secondIsNul) {
      bigEndianPairs += 1;
    } else {
      return undefined;
    }
  }

  if (littleEndianPairs > 0 && bigEndianPairs === 0) {
    return 'utf16le';
  }
  if (bigEndianPairs > 0 && littleEndianPairs === 0) {
    return 'utf16be';
  }
  return undefined;
}

function countByteLines(bytes: Uint8Array, newlineByte: number): number {
  if (bytes.length === 0) return 0;

  let lineBreaks = 0;
  for (const byte of bytes) {
    if (byte === newlineByte) lineBreaks += 1;
  }
  return bytes[bytes.length - 1] === newlineByte ? lineBreaks : lineBreaks + 1;
}

function countUtf16Lines(bytes: Uint8Array, littleEndian: boolean): number | undefined {
  if (bytes.length === 0) return 0;
  if (bytes.length % 2 !== 0) return undefined;

  let lineBreaks = 0;
  let lastCodeUnit: number | undefined;
  for (let index = 0; index < bytes.length; index += 2) {
    const codeUnit = littleEndian
      ? bytes[index] | (bytes[index + 1] << 8)
      : (bytes[index] << 8) | bytes[index + 1];
    if (codeUnit === 0x000a) lineBreaks += 1;
    lastCodeUnit = codeUnit;
  }

  return lastCodeUnit === 0x000a ? lineBreaks : lineBreaks + 1;
}

export function gitDiffSummaryLabel(summary: GitDiffSummary | undefined, loading: boolean): string {
  if (loading) return '...';
  if (!summary || summary.status === 'error') return '--';
  if (summary.addedLines === 0 && summary.removedLines === 0) return '';
  return `+${summary.addedLines} -${summary.removedLines}`;
}
