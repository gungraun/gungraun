//! Shared calibration helpers used by benchmark execution.
//!
//! These functions are intended for internal use by `gungraun`, `gungraun-macros` and
//! `gungraun-runner`.

use std::time::{Duration, Instant};

/// Estimates the optimal number of repetitions a benchmark should execute per measurement
///
/// The calibration loop uses a batched `setup -> work -> teardown` pipeline. For each sampled
/// iteration count, it first executes all `setup` calls in a batch, then executes `work` once for
/// each setup result, and finally executes `teardown` for each produced output. This batching model
/// assumes that grouping all setup calls before all work calls, and all teardown calls after the
/// timed work batch, preserves the benchmark's intended semantics.
///
/// Calibration samples progressively larger iteration counts until `max_calibration_time` expires.
/// For each round, it computes a per-iteration cost from the timed work batch and retains the
/// minimum observed value as the most stable estimate of the benchmark's steady-state cost.
///
/// This function is based on <https://arxiv.org/pdf/1608.04295>, which provides the theoretical
/// background for Julia's benchmarking methodology. This algorithm estimates "n, the optimal number
/// of benchmark repetitions required to minimize timer error and maximize the number of data points
/// obtainable within a time budget".
pub fn calibrate_linear<I, O, S, W, T>(
    max_calibration_time: Duration,
    setup: S,
    work: W,
    teardown: T,
) -> u64
where
    S: Fn() -> I,
    W: Fn(I) -> O,
    T: Fn(O),
{
    // This resolution is an overestimate of `timer accuracy/timer precision` and timer precision
    // assumed to be around ~1ns. most machines will be higher resolution than this, but we're
    // playing it safe
    let resolution = 1_000;
    let mut rounds = Vec::with_capacity(resolution);
    let calibration_start = Instant::now();

    for iterations in 1..=resolution {
        let inputs = std::iter::repeat_with(&setup)
            .take(iterations)
            .collect::<Vec<_>>();

        let start = Instant::now();
        let outputs = inputs.into_iter().map(&work).collect::<Vec<_>>();
        let elapsed = start.elapsed();

        for output in outputs {
            teardown(output);
        }

        let ns_per_iteration = elapsed.as_nanos() / iterations as u128;

        rounds.push(ns_per_iteration);

        if calibration_start.elapsed() >= max_calibration_time {
            break;
        }
    }

    let ns_per_iteration = rounds
        .iter()
        .min_by(Ord::cmp)
        .expect("There should be at least one element");
    if *ns_per_iteration < 2000 {
        // The function should already return values within the clamp region, but to prevent
        // rounding errors and to ensure monotonic behavior, clamp the result of logistic between
        // 1000 and 9.
        #[expect(
            clippy::cast_precision_loss,
            reason = "`ns_per_iteration` is smaller than 2000 when casting to f64"
        )]
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the clamped values are too small for u64 truncation"
        )]
        #[expect(clippy::cast_sign_loss, reason = "the clamped values are positive")]
        {
            logistic(1019.0, 9.0, -0.0125, *ns_per_iteration as f64, 235.0, 0.35).clamp(9.0, 1000.0)
                as u64
        }
    } else if *ns_per_iteration < 10000 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "`ns_per_iteration < 10000`, so this value fits in u64"
        )]
        {
            (11 - ns_per_iteration / 1000) as u64
        }
    } else {
        1
    }
}

/// The generalized logistic function
#[inline]
pub fn logistic(u: f64, l: f64, k: f64, t: f64, t0: f64, nu: f64) -> f64 {
    ((u - l) / (1.0 + (-k * (t - t0)).exp()).powf(nu)) + l
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn calibrate_linear_smoke_test_returns_at_least_one() {
        let repetitions = calibrate_linear(
            Duration::from_millis(1),
            || 1_u64,
            |input| input + 1,
            |_| {},
        );

        assert!(repetitions >= 1);
    }
}
