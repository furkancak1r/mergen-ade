export interface ClipboardPasteSnapshot {
  text: string;
  types: string[];
  filesLength: number;
  itemTypes: string[];
  itemKinds: string[];
}

type ClipboardDataLike = {
  getData?: (format: string) => string;
  types?: Iterable<string> | ArrayLike<string>;
  files?: { length: number };
  items?: ArrayLike<{ kind?: string; type?: string } | null>;
};

export const snapshotClipboardPaste = (data: ClipboardDataLike | null | undefined): ClipboardPasteSnapshot => {
  const items = data?.items
    ? Array.from({ length: data.items.length }, (_, index) => data.items?.[index] ?? null).filter((item): item is { kind?: string; type?: string } => Boolean(item))
    : [];
  return {
    text: data?.getData?.('text/plain') ?? data?.getData?.('text') ?? '',
    types: data?.types ? Array.from(data.types as Iterable<string>) : [],
    filesLength: data?.files?.length ?? 0,
    itemTypes: items.map((item) => item.type ?? '').filter(Boolean),
    itemKinds: items.map((item) => item.kind ?? '').filter(Boolean),
  };
};

export const shouldReadNativeClipboardImage = (snapshot: ClipboardPasteSnapshot): boolean => {
  return snapshot.itemTypes.some((type) => type.toLowerCase().startsWith('image/'))
    || snapshot.types.some((type) => type.toLowerCase().startsWith('image/'));
};

export const shouldReadNativeClipboardFilePaths = (snapshot: ClipboardPasteSnapshot): boolean => {
  if (snapshot.filesLength > 0) return true;
  if (snapshot.types.some((type) => type.toLowerCase() === 'files')) return true;
  if (snapshot.itemKinds.some((kind) => kind.toLowerCase() === 'file')) return true;
  return looksLikePathList(snapshot.text);
};

export const looksLikePathList = (text: string): boolean => {
  const lines = text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  if (lines.length === 0) return false;
  return lines.every((line) => looksLikePath(line));
};

const looksLikePath = (text: string): boolean => {
  const unquoted = text.replace(/^["']|["']$/g, '');
  return /^[a-zA-Z]:[\\/]/.test(unquoted)
    || /^\\\\/.test(unquoted)
    || /^\//.test(unquoted)
    || /^~[\\/]/.test(unquoted);
};
