use std::borrow::Cow;
use std::fmt::Write;
use std::io::BufRead;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::config::CapturedOutput;

// The regex patterns working on the `stdout` must not include the indentation. The indentation can
// be different depending on the `show_grid` option and starts either with 2 spaces (`  `) or if
// `show_grid` is `true` with a pipe character (`|`)
static NUMBERS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            (?<desc>.+?\s*)(?<comp1>[0-9.]+|N/A)\|(?<comp2>[0-9.]+|N/A)
            (?<diff>
                (?<diff_percent>(?<white1>\s*)(?<percent>\(.*\)))
                (?<diff_factor>(?<white2>\s*)(?<factor>\[.*\]))?
            )?",
    )
    .expect("Regex should compile")
});

// Do not match (*********); those placeholder lines should stay unchanged.
static NUMBERS_DIFF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\([^)*]*\)(?:\s+\[[^\]]+\])?$").expect("Regex should compile"));

static UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<prefix>)(?<unit>\s*\[[^\]]+\])(?<suffix>:\s*)").expect("Regex should compile")
});

static RUNNING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[ ]+Running .*$").expect("Regex should compile"));

static PROCESS_DID_NOT_EXIT_SUCCESSFULLY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([ ]+process didn't exit successfully: `)(.*)(` \(exit status: .*\).*)$")
        .expect("Regex should compile")
});

// Performance has regressed: Instructions (123 -> 196) regressed by +47.3684% (>+0.00000%)
// Performance has regressed: Some (123.4 [ms] -> 456.7 [ms]) regressed by +47.3684% (>+0.00000%)
static REGRESSION_SOFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
                ^(Performance\ has\ regressed:\s*[^0-9]+\() # 1: prefix
                ([0-9.]+)                                   # first int/decimal
                (?:\s*\[\S+\])?                             # ignore units
                (\s*->\s*)                                  # 3: arrow with whitespace
                ([0-9.]+)                                   # second int/decimal
                (?:\s*\[\S+\])?                             # ignore units
                (\)\s*regressed\s*by\s*[+-])                # 5: middle part
                ([0-9.]+)                                   # third int/decimal
                (%\s*\([><][+-])                            # 7: suffix start
                ([0-9.]+)                                   # forth int/decimal
                (%\)\s*)                                    # 9: suffix end
                $",
    )
    .expect("Regex should compile")
});

// * Performance has regressed: Instructions (70021) exceeds limit by 69821 (>200)
// * Performance has regressed: cpu_core/instructions/u [*instructions*] (7002804) exceeds limit by
//   6997804 (>5000)
// * Performance has regressed: task-clock:u [*task-clock*] (601.931 [us]) exceeds limit by 501.931
//   [us] (>100 [us])
// $1<__NUM__>$3<__NUM__>$5<__NUM__>$7
static REGRESSION_HARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^(Performance\s*has\s*regressed:\s*[^0-9]+\()
            ([^)]+)
            (\)\s*exceeds\s*limit\s*by\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*\([><])
            ([^)]+)
            (\))$",
    )
    .expect("Regex should compile")
});

// Instructions (357182 -> 357704): +0.14614% exceeds limit of +0.00000%
// $1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__PERCENT__>$9
static SUMMARY_REGRESSION_SOFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^(\s*[^0-9]+\()
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*->\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\):\s*[+-])
            ([0-9.]+)
            (%\s*exceeds\s*limit\s*of\s*[+-])
            ([0-9.]+)
            (%\s*)
            $",
    )
    .expect("Regex should compile")
});

// * Callgrind: Instructions (70021): 70021 exceeds limit of 200 by 69821
// * Perf: cpu_core/instructions/u [*instructions*] (7002804): 7002804 exceeds limit of 5000 by
//   6997804
// * Perf: task-clock:u [*task-clock*] (602.920 [us]): 602.920 [us] exceeds limit of 100 [us] by
//   502.920 [us]
//
// $1<__NUM__>$3<__NUM__>$5<__LIMIT__>$7<__DIFF__>$9
static SUMMARY_REGRESSION_HARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^(\s*[^0-9]+\()
            ([0-9.]+(?:\s*\[[^)]+\])?)
            (\):\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*exceeds\s*limit\s*of\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*by\s*)
            ([0-9.]+(?:\s*\[\S+\])?)$",
    )
    .expect("Regex should compile")
});

// Command: target/release/deps/test_lib_bench_threads-c2a88f916ff580f9
static COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Command:)(\s*target/release/deps/test_(lib|bin)_bench_.+-[a-z0-9]+\s*.*)$")
        .expect("Regex should compile")
});

static PID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(p?pid:\s*)([0-9]+)(\s+)?").expect("Regex should compile"));

static DETAILS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^Details:").expect("Regex should compile"));

static NOT_DETAILS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?:\S)|(?:[a-zA-Z]))").expect("Regex should compile"));

// `  ## pid: <__PID__> part: 1 thread: 3   |pid: <__PID__> part: 1 thread: 3`
static FRAGMENT_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(##(?: \S+: \S+)+)(\s*)([|].*)$").expect("Regex should compile")
});

static ABSOLUTE_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s+|^|')([/][^/]*)+").expect("Regex should compile"));

// Gungraun result: Success. 2 completed without regressions; 0 regressed; 0 filtered;
// 2 benchmarks finished in 0.296s
static SUMMARY_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Gungraun result:.*finished in\s*)([0-9.]+)(s$)").expect("Regex should compile")
});

static THREAD_PANICKED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?<start>thread '.*' )(?<pid>\([0-9]+\))?",
        r"(?<end>\s*panicked at .*)$"
    ))
    .expect("Regex should compile")
});

static ABSOLUTE_PATH_APOSTROPHE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("[']([/][^/']+)+[']").expect("Regex should compile"));

impl CapturedOutput {
    pub fn normalize_coverage_stdout(stdout: &str) -> String {
        let mut result = String::new();
        for line in stdout.lines() {
            let (indent, line) = if line.starts_with("  ") || line.starts_with('|') {
                (&line[0..2], &line[2..])
            } else {
                (&line[0..0], line)
            };

            let line = if line.starts_with("Reads bytes") || line.starts_with("Writes bytes") {
                NUMBERS_DIFF_RE.replace(line, "(         )")
            } else {
                Cow::Borrowed(line)
            };

            writeln!(result, "{indent}{line}").unwrap();
        }
        result
    }

    pub fn filter_stderr(stderr: &[u8]) -> String {
        let mut result = String::new();
        let mut start = false;
        let mut first = false;
        for line in stderr.lines().map(Result::unwrap) {
            if !start {
                if RUNNING_RE.is_match(&line) {
                    start = true;
                    first = true;
                }
                continue;
            } else if first {
                if line.trim().is_empty() {
                    first = false;
                    continue;
                }
                first = false;
            } else {
                // do nothing
            }

            let line = if let Some(caps) = THREAD_PANICKED.captures(&line) {
                let mut new = String::with_capacity(line.len());
                new.push_str(caps.name("start").unwrap().as_str());
                if caps.name("pid").is_some() {
                    new.push_str("(<__PID__>)");
                }

                new.push_str(caps.name("end").unwrap().as_str());

                new
            } else {
                line
            };

            let line = PROCESS_DID_NOT_EXIT_SUCCESSFULLY_RE.replace(&line, "$1<__PATH__>$3");
            let line = ABSOLUTE_PATH_APOSTROPHE_RE.replace(&line, "'<__ABS_PATH__>$1'");
            let line = REGRESSION_SOFT_RE
                .replace(&line, "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__NUM__>$9");
            let line = REGRESSION_HARD_RE.replace(&line, "$1<__NUM__>$3<__DIFF__>$5<__LIMIT__>$7");
            writeln!(result, "{line}").unwrap();
        }
        result
    }

    pub fn filter_stdout(&self, stdout: &[u8]) -> String {
        let mut result = String::new();
        let mut details = false;
        for line in stdout.lines().map(Result::unwrap) {
            let (indent, line) = if line.starts_with("  ") || line.starts_with('|') {
                (&line[0..2], &line[2..])
            } else {
                (&line[0..0], line.as_str())
            };

            let line = if let Some(caps) = THREAD_PANICKED.captures(line) {
                let mut new = String::with_capacity(line.len());
                new.push_str(caps.name("start").unwrap().as_str());
                if caps.name("pid").is_some() {
                    new.push_str("(<__PID__>)");
                }

                new.push_str(caps.name("end").unwrap().as_str());

                new
            } else {
                line.to_owned()
            };

            let line = line.as_str();

            // The `  Details: ...` can contain platform, toolchain specific information about a
            // tool run and make the benchmark tests flaky. So, we filter the details. The
            // (multiline) details usually look like this in the original output:
            //
            // ```
            //   Command:            target/release/deps/test_lib_bench_tools-85f9071c66a70881
            //   Details:            # Thread 1
            //                       #   Total intervals: 0 (Interval Size 100000000)
            //                       #   Total instructions: 459813
            //                       #   Total reps: 499
            //                       #   Unique reps: 5
            //                       #   Total fldcw instructions: 0
            //   Command:            target/release/sort
            //   Details:            # Thread 1
            //                       #   Total intervals: 1 (Interval Size 100000000)
            //                       #   Total instructions: 104432528
            //                       #   Total reps: 457
            //                       #   Unique reps: 4
            //                       #   Total fldcw instructions: 0
            // ```
            //
            // and are transformed into: (The benchmark `Command` is also filtered. See below.)
            //
            // ```
            //   Command: <__COMMAND__>
            //   Details: <__DETAILS__>
            //   Command:            target/release/sort
            //   Details: <__DETAILS__>
            // ```
            if details {
                if NOT_DETAILS_RE.is_match(line) {
                    details = false;
                } else {
                    continue;
                }
            } else if DETAILS_RE.is_match(line) {
                writeln!(result, "{indent}Details: <__DETAILS__>").unwrap();
                details = true;
                continue;
            } else {
                // do nothing
            }

            if let Some(caps) = NUMBERS_RE.captures(line) {
                let mut string = String::new();
                let desc = filter_unit(caps.name("desc").unwrap().as_str());
                let comp1 = {
                    let cap = caps.name("comp1").unwrap().as_str();
                    if cap.parse::<f64>().is_ok() {
                        " ".repeat(cap.len())
                    } else {
                        cap.to_owned()
                    }
                };
                let comp2 = {
                    let cap = caps.name("comp2").unwrap().as_str();
                    if cap.parse::<f64>().is_ok() {
                        " ".repeat(cap.len())
                    } else {
                        cap.to_owned()
                    }
                };
                write!(string, "{desc}{comp1}|{comp2}").unwrap();

                // RAM Hits (and EstimatedCycles, L1, LL Hits) events are unreliable across
                // different systems/toolchains and deviate by a few counts up or down. So to keep
                // the output comparison more reliable we change this line from (for example)
                //
                //   RAM Hits:             179|209             (-14.3541%) [-1.16760x]
                //   RAM Hits:             179|179             (No Change)
                //
                // to
                //
                //   RAM Hits:             179|209             (         )
                //
                // and
                //
                //   RAM Hits:             179|N/A             (*********)
                //
                // to
                //
                //   RAM Hits:                |N/A             (*********)

                // Callgrind/Cachegrind
                if desc.starts_with("RAM Hits")
                    || desc.starts_with("Estimated Cycles")
                    || desc.starts_with("LL Hits")
                    || desc.starts_with("L1 Hits")
                    || desc.starts_with("SysTime")
                    || desc.starts_with("SysCpuTime")
                    // Error tools like Memcheck
                    || desc.starts_with("Suppressed Errors")
                    || desc.starts_with("Suppressed Contexts")
                    // DHAT
                    || desc.starts_with("At t-gmax bytes")
                    || desc.starts_with("At t-gmax blocks")
                {
                    if caps.name("diff_percent").is_some() {
                        let white1 = caps.name("white1").unwrap().as_str();
                        let percent = caps.name("percent").unwrap().as_str();
                        if percent == "(*********)" {
                            write!(string, "{white1}{percent}").unwrap();
                        } else {
                            write!(string, "{white1}(         )").unwrap();
                        }
                    }
                } else {
                    if caps.name("diff_percent").is_some() {
                        let white1 = caps.name("white1").unwrap().as_str();
                        let percent = caps.name("percent").unwrap().as_str();
                        let num = &percent[1..percent.len() - 2];
                        let pos = num.find(['+', '-', '>']);

                        match pos {
                            Some(pos)
                                if num[pos + 1..].parse::<f64>().is_ok()
                                    || percent == "(---inf---)"
                                    || percent == "(+++inf+++)" =>
                            {
                                write!(string, "{white1}(        %)").unwrap();
                            }
                            Some(_) | None if self.has_tolerance && percent == "(No change)" => {
                                write!(string, "{white1}(Tolerance)").unwrap();
                            }
                            Some(_) | None => {
                                write!(string, "{white1}{percent}").unwrap();
                            }
                        }
                    }
                    if caps.name("diff_factor").is_some() {
                        let white2 = caps.name("white2").unwrap().as_str();
                        let factor = caps.name("factor").unwrap().as_str();
                        let num = &factor[1..factor.len() - 2];
                        let pos = num.find(['+', '-', ' ']);

                        match pos {
                            Some(pos)
                                if num[pos + 1..].parse::<f64>().is_ok()
                                    || factor == "[---inf---]"
                                    || factor == "[+++inf+++]" =>
                            {
                                write!(string, "{white2}[        x]").unwrap();
                            }
                            Some(_) | None => {
                                write!(string, "{white2}{factor}").unwrap();
                            }
                        }
                    }
                }
                writeln!(result, "{indent}{string}").unwrap();
            } else {
                let line = if COMMAND_RE.is_match(line) {
                    // Filter the benchmark command of library benchmarks because it has a random
                    // hash in it's name
                    COMMAND_RE.replace(line, "$1 <__COMMAND__>")
                } else {
                    // Replace absolute paths
                    ABSOLUTE_PATH_RE.replace_all(line, "$1<__ABS_PATH__>$2")
                };

                // Filter the pids and parent pids
                let line = PID_RE.replace_all(&line, |caps: &Captures| {
                    format!("{}<__PID__>{}", &caps[1], caps.get(3).map_or("", |_| " "))
                });

                // Fix the spaces after replacement of pids
                let line = FRAGMENT_HEADER_RE.replace_all(&line, |caps: &Captures| {
                    let caps_1 = &caps[1];
                    let caps_3 = &caps[3];
                    if caps_1.len() < 40 {
                        format!(
                            "{caps_1}{}{caps_3}",
                            " ".repeat(gungraun_runner::runner::format::LEFT_WIDTH - caps_1.len())
                        )
                    } else {
                        format!("{caps_1} {caps_3}")
                    }
                });
                let line = SUMMARY_REGRESSION_SOFT_RE.replace_all(
                    &line,
                    "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__PERCENT__>$9",
                );

                let line = SUMMARY_REGRESSION_HARD_RE
                    .replace_all(&line, "$1<__NUM__>$3<__NUM__>$5<__LIMIT__>$7<__DIFF__>$9");

                let line = SUMMARY_LINE_RE.replace_all(&line, "$1<__SECONDS__>$3");
                writeln!(result, "{indent}{line}").unwrap();
            }
        }

        result
    }
}

fn filter_unit(desc: &str) -> Cow<'_, str> {
    UNIT_RE.replace(desc, |caps: &Captures| {
        format!(
            "{}{}{}",
            &caps["prefix"],
            &caps["suffix"],
            " ".repeat(caps["unit"].len())
        )
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

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
