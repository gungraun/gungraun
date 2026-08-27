//! Wildcard matching of perf metric names against user-provided patterns.

use simplematch::{DoWild, Options};

const DOWILD_OPTIONS: Options<u8> = Options::new()
    .case_insensitive(true)
    .enable_escape(true)
    .enable_classes(true);

/// Returns whether a perf metric name matches a wildcard pattern.
///
/// Both sides are matched case-insensitively with `*`, `?`, and `[...]` character classes; `\`
/// escapes a wildcard character.
///
/// `:` and `/` are treated as equivalent separators: both sides are normalized to `/` before
/// matching, so a pattern like `task-clock/u` matches the perf metric `task-clock:u` and vice
/// versa.
pub fn matches(pattern: &str, metric: &str) -> bool {
    let pattern = normalize(pattern);
    let metric = normalize(metric);

    pattern.dowild_with(metric, DOWILD_OPTIONS)
}

/// Normalizes perf metric separators
pub fn normalize(value: &str) -> String {
    value.trim_end_matches(['/', ':']).bytes().fold(
        String::with_capacity(value.len()),
        |mut acc, byte| {
            if byte == b':' {
                acc.push('/');
            } else {
                acc.push(byte as char);
            }
            acc
        },
    )
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{matches, normalize};

    #[test]
    fn test_normalize_separator_strips_trailing_separators() {
        assert_eq!(normalize("task-clock:"), "task-clock");
        assert_eq!(normalize("task-clock/"), "task-clock");
        assert_eq!(normalize("::"), "");
    }

    #[rstest]
    #[case::identical_separators("task-clock/u", "task-clock/u", true)]
    #[case::slash_pattern_matches_colon_metric("task-clock/u", "task-clock:u", true)]
    #[case::colon_pattern_matches_slash_metric("task-clock:u", "task-clock/u", true)]
    #[case::plain_pattern_matches_trailing_colon_metric("task-clock", "task-clock:", true)]
    #[case::trailing_slash_pattern_matches_plain_metric("task-clock/", "task-clock", true)]
    #[case::wildcard_normalizes_separator("task-*:u", "task-clock/u", true)]
    #[case::character_class_normalizes_separator("task-clock[:/]u", "task-clock/u", true)]
    #[case::escaped_separator_needs_literal_escape_char(r"task-clock\:u", "task-clock/u", false)]
    #[case::different_metrics_do_not_match("task-clock/u", "cpu-clock:u", false)]
    fn test_matches_normalizes_separators(
        #[case] pattern: &str,
        #[case] metric: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(matches(pattern, metric), expected);
    }
}
