pub fn update_terminal_title(input: &str, fallback_index: usize, max_len: usize) -> String {
    truncate_terminal_title(&terminal_title_text(input, fallback_index), max_len)
}

pub fn terminal_title_text(input: &str, fallback_index: usize) -> String {
    let compact = sanitize_input(input);

    if compact.is_empty() {
        format!("Terminal {fallback_index}")
    } else {
        compact
    }
}

pub fn truncate_terminal_title(input: &str, max_len: usize) -> String {
    truncate_readable(input, max_len)
}

fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_readable(input: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    if input.chars().count() <= max_len {
        return input.to_owned();
    }

    if max_len <= 3 {
        return input.chars().take(max_len).collect();
    }

    let mut trimmed: String = input.chars().take(max_len - 3).collect();
    trimmed.push_str("...");
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_fallback_when_input_empty() {
        assert_eq!(update_terminal_title("", 3, 40), "Terminal 3");
        assert_eq!(update_terminal_title("   \t\n", 2, 40), "Terminal 2");
    }

    #[test]
    fn compacts_whitespace_and_keeps_text() {
        let title = update_terminal_title("git    status   -sb", 1, 40);
        assert_eq!(title, "git status -sb");
    }

    #[test]
    fn removes_control_characters() {
        let title = update_terminal_title("echo hi\u{0007}", 1, 40);
        assert_eq!(title, "echo hi");
    }

    #[test]
    fn truncates_long_input() {
        let title = update_terminal_title("abcdefghijklmnopqrstuvwxyz0123456789XYZ", 1, 12);
        assert_eq!(title, "abcdefghi...");
    }

    #[test]
    fn truncates_unicode_safely() {
        let title = update_terminal_title("terminal command sample", 1, 14);
        assert!(title.is_char_boundary(title.len()));
        assert!(title.chars().count() <= 14);
    }

    #[test]
    fn keeps_full_terminal_title_text_without_truncation() {
        let title = terminal_title_text("abcdefghijklmnopqrstuvwxyz0123456789XYZ", 1);
        assert_eq!(title, "abcdefghijklmnopqrstuvwxyz0123456789XYZ");
    }
}
