export function sanitizeTitle(title: string): string {
  return title
    .replace(/[\x00-\x1f\x7f-\x9f]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

export function truncateTitle(title: string, maxLength: number): string {
  if (title.length <= maxLength) return title;
  const truncated = title.slice(0, maxLength - 1);
  return truncated + '…';
}
