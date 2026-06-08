export function normalizeWindowsVerbatimPath(p: string): string {
  if (process.platform !== 'win32') return p;
  if (p.startsWith('\\\\?\\UNC\\')) return '\\\\' + p.slice(8);
  if (p.startsWith('\\\\?\\')) return p.slice(4);
  return p;
}
