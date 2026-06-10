export function selectedTextFromRange(text: string, selectionStart: number, selectionEnd: number): string | null {
  const start = Math.max(0, Math.min(selectionStart, selectionEnd, text.length));
  const end = Math.max(0, Math.min(Math.max(selectionStart, selectionEnd), text.length));
  if (start === end) return null;
  return text.slice(start, end);
}
