use std::collections::VecDeque;

use crate::terminal::{TerminalSnapshot, TerminalStyledLine};

pub const THOUGHT_SAMPLE_INTERVAL_MS: u64 = 500;
pub const THOUGHT_SAMPLE_CAPACITY: usize = 8;
pub const THOUGHT_REPEAT_THRESHOLD: usize = 3;
pub const THOUGHT_MIN_SAMPLE_LEN: usize = 48;
pub const THOUGHT_WINDOW_LINE_COUNT: usize = 12;
pub const THOUGHT_MAX_SAMPLE_CHARS: usize = 400;
pub const THOUGHT_PREFIX_OVERLAP_RATIO: f32 = 0.85;

fn snapshot_line_text(line: &TerminalStyledLine) -> String {
    line.runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn line_is_noise(lower: &str) -> bool {
    if lower.is_empty() {
        return true;
    }
    const NOISE_MARKERS: &[&str] = &[
        "turn complete",
        "hooks",
        "question",
        "enter to submit answer",
        "approve",
        "interrupted",
        "conversation interrupted",
        "plan mode",
        "esc ",
        "yes",
        "no",
        "confirm",
        "cancel",
        "? for help",
    ];
    NOISE_MARKERS.iter().any(|marker| lower.contains(marker))
}

pub fn extract_thought_window(snapshot: &TerminalSnapshot) -> Option<String> {
    let mut collected = Vec::new();
    for line in snapshot.lines.iter().rev() {
        let text = snapshot_line_text(line);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if line_is_noise(&lower) {
            continue;
        }
        collected.push(trimmed.to_owned());
        if collected.len() >= THOUGHT_WINDOW_LINE_COUNT {
            break;
        }
    }
    if collected.is_empty() {
        return None;
    }
    collected.reverse();
    let joined = collected.join("\n");
    if joined.len() < THOUGHT_MIN_SAMPLE_LEN {
        return None;
    }
    Some(joined)
}

pub fn normalize_thought_sample(text: &str) -> String {
    let collapsed = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if collapsed.len() <= THOUGHT_MAX_SAMPLE_CHARS {
        return collapsed;
    }
    collapsed
        .chars()
        .rev()
        .take(THOUGHT_MAX_SAMPLE_CHARS)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

pub fn thought_sample_changed_enough(previous: Option<&str>, next: &str) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let prev_norm = normalize_thought_sample(previous);
    let next_norm = normalize_thought_sample(next);
    if prev_norm == next_norm {
        return false;
    }
    let min_len = prev_norm.len().min(next_norm.len());
    if min_len == 0 {
        return true;
    }
    let shared_prefix = prev_norm
        .chars()
        .zip(next_norm.chars())
        .take_while(|(left, right)| left == right)
        .count();
    let overlap = shared_prefix as f32 / min_len as f32;
    overlap < THOUGHT_PREFIX_OVERLAP_RATIO
}

fn consecutive_prefix_overlap_ratio(left: &str, right: &str) -> f32 {
    let min_len = left.len().min(right.len());
    if min_len == 0 {
        return 0.0;
    }
    let shared_prefix = left
        .chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .count();
    shared_prefix as f32 / min_len as f32
}

pub fn detect_repetitive_thought_pattern(samples: &[String]) -> bool {
    if samples.is_empty() {
        return false;
    }
    let normalized: Vec<String> = samples
        .iter()
        .map(|sample| normalize_thought_sample(sample))
        .filter(|sample| sample.len() >= THOUGHT_MIN_SAMPLE_LEN)
        .collect();
    if normalized.len() < 2 {
        return false;
    }
    if normalized.len() >= THOUGHT_REPEAT_THRESHOLD {
        let latest = normalized.last().expect("normalized len checked");
        let repeat_count = normalized.iter().filter(|sample| sample == &latest).count();
        if repeat_count >= THOUGHT_REPEAT_THRESHOLD {
            return true;
        }
    }
    let left = normalized.get(normalized.len() - 2).expect("len checked");
    let right = normalized.last().expect("len checked");
    consecutive_prefix_overlap_ratio(left, right) >= THOUGHT_PREFIX_OVERLAP_RATIO
}

pub fn push_thought_sample(samples: &mut VecDeque<String>, sample: String) {
    if normalize_thought_sample(&sample).len() < THOUGHT_MIN_SAMPLE_LEN {
        return;
    }
    if samples
        .back()
        .is_some_and(|previous| !thought_sample_changed_enough(Some(previous), &sample))
    {
        return;
    }
    samples.push_back(sample);
    while samples.len() > THOUGHT_SAMPLE_CAPACITY {
        samples.pop_front();
    }
}

pub fn thought_loop_cleared(samples: &[String]) -> bool {
    let normalized: Vec<String> = samples
        .iter()
        .map(|sample| normalize_thought_sample(sample))
        .filter(|sample| sample.len() >= THOUGHT_MIN_SAMPLE_LEN)
        .collect();
    if normalized.len() < 2 {
        return true;
    }
    let left = normalized.get(normalized.len() - 2).expect("len checked");
    let right = normalized.last().expect("len checked");
    left != right && consecutive_prefix_overlap_ratio(left, right) < THOUGHT_PREFIX_OVERLAP_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{TerminalColor, TerminalStyle, TerminalStyledRun};

    fn test_run(text: &str) -> TerminalStyledRun {
        TerminalStyledRun {
            text: text.to_owned(),
            style: TerminalStyle {
                fg: TerminalColor {
                    r: 220,
                    g: 220,
                    b: 220,
                },
                bg: TerminalColor {
                    r: 20,
                    g: 24,
                    b: 30,
                },
                italic: false,
                underline: false,
                strike: false,
            },
            column: 0,
            display_width: text.chars().count().max(1),
        }
    }

    fn line(text: &str) -> TerminalStyledLine {
        TerminalStyledLine {
            runs: vec![test_run(text)],
        }
    }

    fn snapshot_with_lines(lines: &[&str]) -> TerminalSnapshot {
        TerminalSnapshot {
            lines: lines.iter().map(|text| line(text)).collect(),
            cursor: None,
            cursor_line: None,
        }
    }

    fn long_thought(prefix: &str) -> String {
        format!(
            "{prefix} I need to reconsider the same approach and keep evaluating the options carefully before proceeding with the next implementation step."
        )
    }

    #[test]
    fn detect_repetitive_thought_pattern_flags_triple_repeat() {
        let sample = long_thought("alpha");
        let samples = vec![sample.clone(), sample.clone(), sample];
        assert!(detect_repetitive_thought_pattern(&samples));
    }

    #[test]
    fn detect_repetitive_thought_pattern_ignores_short_text() {
        let samples = vec!["short".to_owned(), "short".to_owned(), "short".to_owned()];
        assert!(!detect_repetitive_thought_pattern(&samples));
    }

    #[test]
    fn detect_repetitive_thought_pattern_flags_high_prefix_overlap() {
        let left = long_thought("beta");
        let right = format!("{left} extra tail that should not matter much");
        let samples = vec![left, right];
        assert!(detect_repetitive_thought_pattern(&samples));
    }

    #[test]
    fn extract_thought_window_filters_ui_noise() {
        let snapshot =
            snapshot_with_lines(&["turn complete", "HOOKS Stop", &long_thought("gamma")]);
        let window = extract_thought_window(&snapshot).expect("thought window");
        assert!(window.contains("gamma"));
        assert!(!window.to_ascii_lowercase().contains("turn complete"));
    }

    #[test]
    fn thought_loop_cleared_when_latest_samples_diverge() {
        let left = long_thought("delta");
        let right = long_thought("epsilon");
        let samples = vec![left, right];
        assert!(thought_loop_cleared(&samples));
    }
}
