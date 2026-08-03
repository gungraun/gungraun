#![expect(clippy::redundant_pub_crate)]

#[path = "bench/assert.rs"]
mod assert;
#[path = "bench/config.rs"]
mod config;
#[path = "bench/expected_files.rs"]
mod expected_files;
#[path = "bench/filter.rs"]
mod filter;
#[path = "bench/io.rs"]
mod io;
#[path = "bench/runner.rs"]
mod runner;

use std::collections::HashMap;

use anyhow::{Context, anyhow, bail};
use runner::{Partition, SystemTestRunner, TEMPLATE_DATA};

fn main() -> anyhow::Result<()> {
    // The cli args:
    // positional arguments
    let mut benches = Vec::default();
    // --filter=some_wildcard_filter_*
    let mut filter = Option::default();
    // --partition=x/y
    let mut partition = Option::default();
    // --continue
    let mut resume = false;

    for arg in std::env::args().skip(1) {
        match arg.split_once('=') {
            Some(("--filter", value)) => filter = Some(value.to_owned()),
            Some(("--partition", value)) => {
                let (part_str, total_str) = value
                    .split_once('/')
                    .ok_or_else(|| anyhow!("Invalid partition: {value}"))?;
                let part = part_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition part: {part_str}"))?;
                let total = total_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition total: {total_str}"))?;

                if total == 0 {
                    bail!("The total of a partition should be greater than zero");
                }
                if part == 0 || part > total {
                    bail!("The part of a partition should be within bounds: 0 < x <= total");
                }

                partition = Some(Partition { part, total });
            }
            Some(_) => bail!("Invalid argument: {arg}"),
            None if arg == "--continue" => resume = true,
            None => benches.push(arg),
        }
    }

    let runner = SystemTestRunner::new(&benches, filter.as_deref(), partition, resume)?;

    let mut map = HashMap::new();
    map.insert(
        "target_dir_sanitized".to_owned(),
        minijinja::Value::from_serialize(
            runner
                .tests
                .target_directory
                .display()
                .to_string()
                .replace('/', "_"),
        ),
    );

    TEMPLATE_DATA
        .set(map)
        .map_err(|_| anyhow!("Failed to initialize template data"))?;

    runner.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::filter::*;

    #[rstest]
    #[case::lib_bench("Command: target/release/deps/test_lib_bench_threads-c2a88f916ff580f9")]
    #[case::bin_bench("Command: target/release/deps/test_bin_bench_threads-c2a88f916ff580f9")]
    fn test_command_re(#[case] haystack: &str) {
        assert!(COMMAND_RE.is_match(haystack));
    }

    #[rstest]
    #[case::just_root(" /", " <__ABS_PATH__>/")]
    #[case::with_single_component(" /some", " <__ABS_PATH__>/some")]
    #[case::with_two_components(" /some/final", " <__ABS_PATH__>/final")]
    #[case::with_mixed_characters(" /wi-th_/123/final", " <__ABS_PATH__>/final")]
    #[case::with_text_before(
        "some text before /wi-th_/123/final",
        "some text before <__ABS_PATH__>/final"
    )]
    #[case::with_text_after(
        " /wi-th_/123/final some text after",
        " <__ABS_PATH__>/final some text after"
    )]
    fn test_absolute_path_re(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(
            ABSOLUTE_PATH_RE.replace_all(haystack, "$1<__ABS_PATH__>$2"),
            replaced
        );
    }

    #[rstest]
    #[case::valgrind(
        "Instructions:                                       1234|1234                 \
         (12345678%) [1234.1234x]"
    )]
    #[case::valgrind_na(
        "Instructions:                                        123|N/A                  (*********)"
    )]
    #[case::number_in_event(
        "L1 Hits:                                             123|1                    \
         (1.234567%) [2.3456789x]"
    )]
    #[case::valgrind_with_special(
        "Total read+write:                                 123456|12345                \
         (1.234567%) [123.45678%]"
    )]
    #[case::perf_without_unit(
        "cpu_core/instructions/u:                             N/A|1234                 (*********)"
    )]
    #[case::perf_with_unit(
        "task-clock/u [us]:                                 0.123|1.234                \
         (123456.8%) [1234.1234x]"
    )]
    #[case::perf_rse(
        "  rse% (sig.thr) [sig.fact]                        2.345|3.4567890            \
         (9.876543%) [234.56789x]"
    )]
    #[case::perf_samples(
        "  samples                                     1000000000|20000000000000000000 \
         (3456.123%) [1234.1234x]"
    )]
    fn test_numbers_re(#[case] haystack: &str) {
        assert!(NUMBERS_RE.is_match(haystack));
    }

    #[rstest]
    #[case::perf_with_unit("task-clock/u [us]:", "task-clock/u:     ")]
    #[case::perf_without_unit("cpu_core/instructions/u:", "cpu_core/instructions/u:")]
    #[case::non_unit_brackets("rse% (sig.thr) [sig.fact]", "rse% (sig.thr) [sig.fact]")]
    fn test_filter_unit(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(filter_unit(haystack), replaced);
    }

    #[rstest]
    #[case::no_decimal_and_unit(
        "Performance has regressed: Instructions (133 -> 196) regressed by +47.3684% (>+0.00000%)",
        "Performance has regressed: Instructions (<__NUM__> -> <__NUM__>) regressed by \
         +<__PERCENT__>% (>+<__NUM__>%)"
    )]
    #[case::with_decimal(
        "Performance has regressed: Some (1.234 -> 2.345) regressed by +47.3684% (>+0.00000%)",
        "Performance has regressed: Some (<__NUM__> -> <__NUM__>) regressed by +<__PERCENT__>% \
         (>+<__NUM__>%)"
    )]
    #[case::with_decimal_and_unit(
        "Performance has regressed: Some (1.234 [ms] -> 2.345 [ms]) regressed by +47.3684% \
         (>+0.00000%)",
        "Performance has regressed: Some (<__NUM__> -> <__NUM__>) regressed by +<__PERCENT__>% \
         (>+<__NUM__>%)"
    )]
    fn test_regression_soft_re(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(
            REGRESSION_SOFT_RE.replace(
                haystack,
                "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__NUM__>$9"
            ),
            replaced
        );
    }

    #[rstest]
    #[case::callgrind(
        "Performance has regressed: Instructions (70021) exceeds limit by 69821 (>200)"
    )]
    #[case::perf_no_unit(
        "Performance has regressed: cpu_core/instructions/u [*instructions*] (7002804) exceeds \
         limit by 6997804 (>5000)"
    )]
    #[case::perf_with_unit(
        "Performance has regressed: task-clock:u [*task-clock*] (601.931 [us]) exceeds limit by \
         501.931 [us] (>100 [us])"
    )]
    fn test_regression_hard_re(#[case] haystack: &str) {
        assert!(REGRESSION_HARD_RE.is_match(haystack));
    }

    #[rstest]
    #[case::callgrind("Callgrind: Instructions (70021): 70021 exceeds limit of 200 by 69821")]
    #[case::perf_no_unit(
        "Perf: cpu_core/instructions/u [*instructions*] (7002804): 7002804 exceeds limit of 5000 \
         by 6997804"
    )]
    #[case::perf_with_unit(
        "Perf: task-clock:u [*task-clock*] (632.461 [us]): 632.461 [us] exceeds limit of 100 [us] \
         by 532.461 [us]"
    )]
    fn test_summary_hard_regression_re(#[case] haystack: &str) {
        assert!(SUMMARY_REGRESSION_HARD_RE.is_match(haystack));
    }

    #[rstest]
    #[case::valgrind("Total bytes (16 -> 20): +25.0000% exceeds limit of +0.00000%")]
    #[case::perf_no_unit(
        "Perf: cpu_core/instructions/u [*instructions*] (1234 -> 4567): +1234% exceeds limit of \
         +1.234%"
    )]
    #[case::perf_with_unit(
        "Perf: task-clock:u [*task-clock*] (38.3450 [us] -> 74111.1 [us]): +193175% exceeds limit \
         of +0.00000%"
    )]
    fn test_summary_soft_regression_re(#[case] haystack: &str) {
        assert!(SUMMARY_REGRESSION_SOFT_RE.is_match(haystack));
    }

    #[rstest]
    #[case::one_path_component("Expected '/root/exit-with'", &["'/root/exit-with'", "/exit-with"])]
    #[case::two_path_component(
        "Expected '/some/root/exit-with'",
        &["'/some/root/exit-with'", "/exit-with"]
    )]
    #[case::multiple_path_component(
        "Expected '/some/root/and/more/path/components/exit-with'",
        &["'/some/root/and/more/path/components/exit-with'", "/exit-with"]
    )]
    #[case::two_apostrophes(
        "Expected '/root/exit-with' to exit with '1' but it succeeded",
        &["'/root/exit-with'", "/exit-with"]
    )]
    fn test_absolute_path_apostrophe_re(#[case] haystack: &str, #[case] matches: &[&str]) {
        if matches.is_empty() {
            assert!(!ABSOLUTE_PATH_APOSTROPHE_RE.is_match(haystack));
        } else {
            let caps = ABSOLUTE_PATH_APOSTROPHE_RE
                .captures(haystack)
                .expect("The regex should succeed to match");

            let caps_debug_string = format!("{caps:?}");
            assert_eq!(caps.len(), matches.len(), "{caps_debug_string}");

            for (cap, mat) in caps.iter().zip(matches) {
                assert_eq!(
                    cap.expect("The capture should be present").as_str(),
                    *mat,
                    "{caps_debug_string}"
                );
            }
        }
    }
}
