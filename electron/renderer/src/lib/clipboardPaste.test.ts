import { describe, expect, it } from 'vitest';
import {
  looksLikePathList,
  shouldReadNativeClipboardFilePaths,
  shouldReadNativeClipboardImage,
  snapshotClipboardPaste,
} from './clipboardPaste';

describe('clipboardPaste', () => {
  it('does not request native image reads for normal text paste', () => {
    const snapshot = snapshotClipboardPaste({
      getData: () => 'hello from another app',
      types: ['text/plain'],
      files: { length: 0 },
      items: [{ kind: 'string', type: 'text/plain' }],
    });
    expect(shouldReadNativeClipboardImage(snapshot)).toBe(false);
    expect(shouldReadNativeClipboardFilePaths(snapshot)).toBe(false);
  });

  it('requests native image reads when the paste event contains an image item', () => {
    const snapshot = snapshotClipboardPaste({
      getData: () => '',
      types: ['Files'],
      files: { length: 1 },
      items: [{ kind: 'file', type: 'image/png' }],
    });
    expect(shouldReadNativeClipboardImage(snapshot)).toBe(true);
    expect(shouldReadNativeClipboardFilePaths(snapshot)).toBe(true);
  });

  it('allows text path lists to use the file path fallback', () => {
    const snapshot = snapshotClipboardPaste({
      getData: () => 'C:\\work\\file.ts\n/home/me/image.png',
      types: ['text/plain'],
      files: { length: 0 },
      items: [{ kind: 'string', type: 'text/plain' }],
    });
    expect(shouldReadNativeClipboardFilePaths(snapshot)).toBe(true);
    expect(shouldReadNativeClipboardImage(snapshot)).toBe(false);
  });

  it('recognizes common absolute path formats', () => {
    expect(looksLikePathList('C:\\work\\file.ts')).toBe(true);
    expect(looksLikePathList('\\\\server\\share\\file.ts')).toBe(true);
    expect(looksLikePathList('/tmp/file.ts')).toBe(true);
    expect(looksLikePathList('plain text')).toBe(false);
  });
});
