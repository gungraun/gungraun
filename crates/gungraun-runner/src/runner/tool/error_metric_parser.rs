//! Module containing the [`ErrorMetricLogfileParser`] for error checking tools like `Memcheck`

// spell-checker:ignore suppr ctxts
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

use super::logfile_parser::{EMPTY_LINE_RE, EXTRACT_FIELDS_RE, STRIP_PREFIX_RE, parse_header};
use super::parser::{Parser, ParserOutput};
use super::path::ToolOutputPath;
use crate::api::ErrorMetric;
use crate::metrics::model::Metrics;
use crate::summary::model::ToolMetrics;

static EXTRACT_ERROR_SUMMARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    regex::Regex::new(
        "^[^0-9]*(?<errs>[0-9]+)[^0-9]*(?<ctxs>[0-9]+)[^0-9]*(?<s_errs>[0-9]+)[^0-9]*(?\
         <s_ctxs>[0-9]+).*$",
    )
    .expect("Regex should compile")
});

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum State {
    HeaderSpace,
    Body,
}

/// The logfile parser for error metrics
#[derive(Debug)]
pub struct ErrorMetricLogfileParser {
    /// The [`ToolOutputPath`]
    pub output_path: ToolOutputPath,
    /// The path to the root/project directory used to make paths relative
    pub root_dir: PathBuf,
}

impl Parser for ErrorMetricLogfileParser {
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        let file = File::open(&path)
            .with_context(|| format!("Error opening log file '{}'", path.display()))?;

        let mut iter = BufReader::new(file)
            .lines()
            .skip_while(|l| l.as_ref().is_ok_and(|l| l.trim().is_empty()));

        let header = parse_header(&path, &mut iter)?;

        let metrics_prototype = Metrics::from_iter([
            ErrorMetric::Errors,
            ErrorMetric::Contexts,
            ErrorMetric::SuppressedErrors,
            ErrorMetric::SuppressedContexts,
        ]);

        let mut details = vec![];
        let mut metrics = None;

        let mut state = State::HeaderSpace;
        for line in iter {
            let line = line?;
            match &state {
                State::HeaderSpace if EMPTY_LINE_RE.is_match(&line) => {}
                State::HeaderSpace | State::Body => {
                    if state == State::HeaderSpace {
                        state = State::Body;
                    }

                    if let Some(caps) = EXTRACT_FIELDS_RE.captures(&line) {
                        let key = caps.name("key").unwrap().as_str();

                        if key.eq_ignore_ascii_case("error summary") {
                            let error_summary_value = caps.name("value").unwrap().as_str();

                            let caps = EXTRACT_ERROR_SUMMARY_RE
                                .captures(error_summary_value)
                                .context(
                                    "Failed to extract error summary from string".to_owned(),
                                )?;

                            // There might be multiple `ERROR SUMMARY` lines. We only use the last.
                            // The comments in the Valgrind source code (`coregrind/m_errormgr.c`)
                            // state that the error summary line is only reprinted to avoid having
                            // to scroll up.
                            let mut new_metrics: Metrics<ErrorMetric> = metrics_prototype.clone();
                            new_metrics.add_iter_str([
                                caps.name("errs").unwrap().as_str(),
                                caps.name("ctxs").unwrap().as_str(),
                                caps.name("s_errs").unwrap().as_str(),
                                caps.name("s_ctxs").unwrap().as_str(),
                            ])?;

                            metrics = Some(new_metrics);
                            continue;
                        }
                    }

                    // Detail lines might also be matched with `EXTRACT_FIELDS_RE`
                    if let Some(caps) = STRIP_PREFIX_RE.captures(&line) {
                        let rest_of_line = caps.name("rest").unwrap().as_str();
                        details.push(rest_of_line.to_owned());
                    } else {
                        details.push(line);
                    }
                }
            }
        }

        // Remove the last empty lines from the details
        while let Some(last) = details.last() {
            if last.trim().is_empty() {
                details.pop();
            } else {
                break;
            }
        }

        Ok(ParserOutput {
            header,
            path,
            metrics: ToolMetrics::ErrorTool(metrics.context(
                "Failed collecting error metrics: An error summary line should be present",
            )?),
            details,
        })
    }

    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use regex::Captures;
    use rstest::rstest;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct ErrorsFixture {
        ctxs: u64,
        errors: u64,
        suppr_ctxs: u64,
        suppr_errors: u64,
    }

    impl ErrorsFixture {
        fn new(errors: u64, ctxs: u64, suppr_errors: u64, suppr_ctxs: u64) -> Self {
            Self {
                ctxs,
                errors,
                suppr_ctxs,
                suppr_errors,
            }
        }

        fn from_caps(captures: &Captures) -> Self {
            let errors = captures
                .name("errs")
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();
            let ctxs = captures
                .name("ctxs")
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();
            let suppr_errors = captures
                .name("s_errs")
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();
            let suppr_ctxs = captures
                .name("s_ctxs")
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();

            Self {
                ctxs,
                errors,
                suppr_ctxs,
                suppr_errors,
            }
        }
    }

    #[rstest]
    #[case::all_zero(
        "0 errors from 0 contexts (suppressed: 0 from 0)",
        ErrorsFixture::new(0, 0, 0, 0)
    )]
    #[case::all_one(
        "1 errors from 1 contexts (suppressed: 1 from 1)",
        ErrorsFixture::new(1, 1, 1, 1)
    )]
    #[case::all_u64_max(
        "18446744073709551615 errors from 18446744073709551615 contexts (suppressed: \
         18446744073709551615 from 18446744073709551615)",
        ErrorsFixture::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX,)
    )]
    #[case::different_numbers(
        "1 errors from 2 contexts (suppressed: 3 from 4)",
        ErrorsFixture::new(1, 2, 3, 4)
    )]
    #[case::different_numbers_num_digits_gt_1(
        "11 errors from 123 contexts (suppressed: 1345 from 14567)",
        ErrorsFixture::new(11, 123, 1345, 14567)
    )]
    fn test_extract_errors_re(#[case] haystack: &str, #[case] expected_errors: ErrorsFixture) {
        let caps = EXTRACT_ERROR_SUMMARY_RE.captures(haystack).unwrap();
        let actual_errors = ErrorsFixture::from_caps(&caps);

        assert_eq!(actual_errors, expected_errors);
    }
}
