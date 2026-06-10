export function normalizeBrowserUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) return '';

  const lower = trimmed.toLowerCase();

  // Reject unsupported schemes
  if (lower.startsWith('file:') || lower.startsWith('data:') || lower.startsWith('javascript:')) {
    return '';
  }

  // Already has valid scheme
  if (lower.startsWith('http://') || lower.startsWith('https://')) {
    return trimmed;
  }

  // Localhost and loopback addresses use http
  if (
    lower.startsWith('localhost') ||
    lower.startsWith('127.0.0.1') ||
    lower.startsWith('0.0.0.0') ||
    lower.startsWith('[::1]')
  ) {
    return 'http://' + trimmed;
  }

  // All other domains use https
  return 'https://' + trimmed;
}

export function isAllowedBrowserScheme(url: string): boolean {
  const lower = url.trim().toLowerCase();
  return lower.startsWith('http://') || lower.startsWith('https://');
}
