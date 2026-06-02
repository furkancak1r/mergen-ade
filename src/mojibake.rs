//! Mojibake recovery for project names/paths that were accidentally
//! re-encoded through the UTF-8 -> CP1252 -> UTF-8 chain (sometimes
//! repeatedly). Used by `config::normalize_config_for_current_platform`
//! and by project-add code paths as a defensive net.

/// Maps a single Unicode `char` back to its CP1252 byte, if such a mapping
/// exists. Returns `None` for characters outside the CP1252 range so that
/// `try_repair_once` can bail early.
pub fn cp1252_byte(c: char) -> Option<u8> {
    let code = c as u32;
    if code <= 0x7F {
        return Some(code as u8);
    }
    if (0xA0..=0xFF).contains(&code) {
        return Some(code as u8);
    }
    match c {
        '\u{20AC}' => Some(0x80),
        '\u{0081}' => Some(0x81),
        '\u{201A}' => Some(0x82),
        '\u{0192}' => Some(0x83),
        '\u{201E}' => Some(0x84),
        '\u{2026}' => Some(0x85),
        '\u{2020}' => Some(0x86),
        '\u{2021}' => Some(0x87),
        '\u{02C6}' => Some(0x88),
        '\u{2030}' => Some(0x89),
        '\u{0160}' => Some(0x8A),
        '\u{2039}' => Some(0x8B),
        '\u{0152}' => Some(0x8C),
        '\u{008D}' => Some(0x8D),
        '\u{017D}' => Some(0x8E),
        '\u{008F}' => Some(0x8F),
        '\u{0090}' => Some(0x90),
        '\u{2018}' => Some(0x91),
        '\u{2019}' => Some(0x92),
        '\u{201C}' => Some(0x93),
        '\u{201D}' => Some(0x94),
        '\u{2022}' => Some(0x95),
        '\u{2013}' => Some(0x96),
        '\u{2014}' => Some(0x97),
        '\u{02DC}' => Some(0x98),
        '\u{2122}' => Some(0x99),
        '\u{0161}' => Some(0x9A),
        '\u{203A}' => Some(0x9B),
        '\u{0153}' => Some(0x9C),
        '\u{009D}' => Some(0x9D),
        '\u{017E}' => Some(0x9E),
        '\u{0178}' => Some(0x9F),
        _ => None,
    }
}

/// Attempts a single round of mojibake recovery: re-encodes the input
/// as CP1252 bytes and decodes that byte buffer as UTF-8. Returns
/// `Some(decoded)` only if the round produced valid UTF-8 *and*
/// changed the input, otherwise `None`.
pub fn try_repair_once(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(input.len());
    for c in input.chars() {
        bytes.push(cp1252_byte(c)?);
    }
    let decoded = std::str::from_utf8(&bytes).ok()?;
    if decoded == input {
        return None;
    }
    Some(decoded.to_owned())
}

/// Iteratively recovers from multi-level mojibake. ASCII-only or already
/// clean input is returned unchanged. Bails out as soon as another round
/// would fail (invalid UTF-8 or a no-op), preserving the deepest valid
/// repair. Caps iterations to avoid pathological loops.
pub fn repair_mojibake(input: &str) -> String {
    let mut current = input.to_owned();
    for _ in 0..5 {
        match try_repair_once(&current) {
            Some(next) => current = next,
            None => break,
        }
    }
    current
}

/// Repair a display string: always applies mojibake recovery regardless
/// of whether the result exists on disk. Use for user-facing text that
/// may have been re-encoded (directory names, file names, branch names,
/// project names, tooltips, attachment mentions, etc.).
pub fn repair_mojibake_display(input: &str) -> String {
    repair_mojibake(input)
}

/// Repair a path that may contain mojibake. If the repaired path exists
/// on disk, returns the repaired `PathBuf`. Otherwise returns the original
/// `PathBuf` unchanged. This prevents false repair of paths whose actual
/// on-disk names genuinely contain what looks like mojibake.
pub fn repair_mojibake_path(path: &std::path::Path) -> std::path::PathBuf {
    let s = path.as_os_str().to_string_lossy();
    let repaired = repair_mojibake(&s);
    if repaired != s.as_ref() {
        let repaired_path = std::path::PathBuf::from(&repaired);
        if repaired_path.exists() {
            return repaired_path;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn byte_to_cp1252_char(b: u8) -> char {
        match b {
            0x80 => '\u{20AC}',
            0x82 => '\u{201A}',
            0x83 => '\u{0192}',
            0x84 => '\u{201E}',
            0x85 => '\u{2026}',
            0x86 => '\u{2020}',
            0x87 => '\u{2021}',
            0x88 => '\u{02C6}',
            0x89 => '\u{2030}',
            0x8A => '\u{0160}',
            0x8B => '\u{2039}',
            0x8C => '\u{0152}',
            0x8E => '\u{017D}',
            0x91 => '\u{2018}',
            0x92 => '\u{2019}',
            0x93 => '\u{201C}',
            0x94 => '\u{201D}',
            0x95 => '\u{2022}',
            0x96 => '\u{2013}',
            0x97 => '\u{2014}',
            0x98 => '\u{02DC}',
            0x99 => '\u{2122}',
            0x9A => '\u{0161}',
            0x9B => '\u{203A}',
            0x9C => '\u{0153}',
            0x9E => '\u{017E}',
            0x9F => '\u{0178}',
            other => other as char,
        }
    }

    fn corrupt_once(input: &str) -> String {
        input
            .as_bytes()
            .iter()
            .map(|b| byte_to_cp1252_char(*b))
            .collect()
    }

    #[test]
    fn ascii_input_is_unchanged() {
        assert_eq!(repair_mojibake("hello world"), "hello world");
        assert_eq!(repair_mojibake(""), "");
        assert_eq!(repair_mojibake("project-1"), "project-1");
    }

    #[test]
    fn already_clean_turkish_is_unchanged() {
        assert_eq!(repair_mojibake("Satın Alma"), "Satın Alma");
        assert_eq!(repair_mojibake("ÜRETİM PLANI"), "ÜRETİM PLANI");
    }

    #[test]
    fn single_level_mojibake_is_repaired() {
        assert_eq!(repair_mojibake(&corrupt_once("ö")), "ö");
        assert_eq!(repair_mojibake(&corrupt_once("İ")), "İ");
        assert_eq!(repair_mojibake(&corrupt_once("ü")), "ü");
        assert_eq!(repair_mojibake(&corrupt_once("ÜRETİM")), "ÜRETİM");
    }

    #[test]
    fn double_level_mojibake_is_repaired() {
        let once = corrupt_once("ö");
        let twice = corrupt_once(&once);
        assert_eq!(repair_mojibake(&once), "ö");
        assert_eq!(repair_mojibake(&twice), "ö");
    }

    #[test]
    fn triple_level_mojibake_recovers_to_original() {
        let original = "Ö";
        let mut current = original.to_owned();
        for _ in 0..3 {
            current = corrupt_once(&current);
        }
        assert_eq!(repair_mojibake(&current), original);
    }

    #[test]
    fn triple_mojibake_full_turkish_phrase_recovers() {
        let original = "2026 ÜRETİM PLANI 11.Haftadan İtibaren Güncelleme";
        let mut current = original.to_owned();
        for _ in 0..3 {
            current = corrupt_once(&current);
        }
        assert_eq!(repair_mojibake(&current), original);
    }

    #[test]
    fn invalid_utf8_round_preserves_input() {
        let weird = "\u{FFFD}";
        assert_eq!(repair_mojibake(weird), weird);
    }

    #[test]
    fn repair_mojibake_display_repairs_turkish() {
        let corrupted = corrupt_once("Satın Alma");
        assert_eq!(repair_mojibake_display(&corrupted), "Satın Alma");
        assert_eq!(repair_mojibake_display("Satın Alma"), "Satın Alma");
    }

    #[test]
    fn repair_mojibake_display_repairs_multi_level() {
        let mut c = corrupt_once("ÜRETİM PLANI");
        c = corrupt_once(&c);
        assert_eq!(repair_mojibake_display(&c), "ÜRETİM PLANI");
    }

    #[test]
    fn repair_mojibake_path_returns_original_when_repaired_does_not_exist() {
        let path = std::path::Path::new("C:/nonexistent/Satın Alma");
        let result = repair_mojibake_path(path);
        assert_eq!(result, path);
    }

    #[test]
    fn repair_mojibake_path_returns_repaired_when_it_exists() {
        let dir = std::env::temp_dir().join("mojibake_test_exists");
        let _ = std::fs::remove_dir_all(&dir);
        let clean_path = dir.join("Satın Alma");
        std::fs::create_dir_all(&clean_path).unwrap();
        let corrupted_segment = corrupt_once("Satın Alma");
        let corrupted_path = dir.join(&corrupted_segment);
        let result = repair_mojibake_path(&corrupted_path);
        assert_eq!(
            result, clean_path,
            "repair should map to the existing clean path"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repair_mojibake_path_returns_original_for_clean_path() {
        let path = std::path::Path::new("C:/clean/path");
        let result = repair_mojibake_path(path);
        assert_eq!(result, path);
    }

    #[test]
    fn cp1252_bytes_round_trip_smart_quotes() {
        assert_eq!(cp1252_byte('\u{2019}'), Some(0x92));
        assert_eq!(cp1252_byte('\u{20AC}'), Some(0x80));
        assert_eq!(cp1252_byte('A'), Some(0x41));
        assert_eq!(cp1252_byte('ç'), Some(0xE7));
        assert_eq!(cp1252_byte('İ'), None);
    }
}
