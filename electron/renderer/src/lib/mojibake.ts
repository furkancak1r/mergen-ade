/**
 * Mojibake repair: CP1252 -> UTF-8 decode chain.
 * Iterates up to 5 rounds treating bytes as CP1252 then decoding as UTF-8.
 */

const MAX_REPAIR_ROUNDS = 5;

function cp1252Bytes(text: string): Uint8Array {
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    // CP1252 mapping: bytes 0x80-0x9F have specific mappings, but for repair
    // we treat char codes 0-255 as direct byte values.
    bytes[i] = code < 256 ? code : 0x3F; // '?' for out-of-range
  }
  return bytes;
}

function utf8Decode(bytes: Uint8Array): string | null {
  try {
    const decoder = new TextDecoder('utf-8', { fatal: true });
    return decoder.decode(bytes);
  } catch {
    return null;
  }
}

export function repairMojibake(text: string): string {
  // If text contains any code points outside CP1252 range (0-255), it's already UTF-8
  let hasNonCp1252 = false;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) > 255) {
      hasNonCp1252 = true;
      break;
    }
  }
  if (hasNonCp1252) return text;

  let current = text;
  for (let round = 0; round < MAX_REPAIR_ROUNDS; round++) {
    const bytes = cp1252Bytes(current);
    const repaired = utf8Decode(bytes);
    if (repaired === null) break;
    if (repaired === current) break;
    current = repaired;
  }
  return current;
}

/**
 * Disk-aware repair: returns repaired path only when it actually exists.
 * Falls back to original if repaired does not exist.
 */
export async function repairMojibakePath(path: string, existsFn: (p: string) => Promise<boolean>): Promise<string> {
  const repaired = repairMojibake(path);
  if (repaired === path) return path;
  const ok = await existsFn(repaired);
  return ok ? repaired : path;
}

/**
 * Unconditional repair for user-facing display text.
 */
export function repairMojibakeDisplay(text: string): string {
  return repairMojibake(text);
}

/**
 * Char-safe truncation for UI display. Never splits multi-byte UTF-8 sequences.
 */
export function cappedHoverText(text: string, maxChars: number): string {
  if (text.length <= maxChars) return text;
  // Use Array.from to work with Unicode code points/char clusters
  const chars = Array.from(text);
  if (chars.length <= maxChars) return text;
  return chars.slice(0, maxChars).join('') + '…';
}
