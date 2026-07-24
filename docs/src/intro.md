# Introduction

Welcome to the Gungraun guide, your comprehensive resource for a one-shot
benchmarking harness and framework that leverages Valgrind's powerful CPU,
cache, and memory profiling tools: [Callgrind][callgrind-manual],
[Cachegrind][cachegrind-manual], and [DHAT][dhat-manual]. Gungraun delivers
highly accurate and consistent measurements of Rust code, making it an ideal
choice for continuous integration (CI) environments. Its flexibility allows you
to access all Valgrind tools, even `Memcheck`, and use
[Valgrind client requests](./client_requests.md) effortlessly.

Gungraun is:

- **Precise**: High-precision measurements of `Instruction` counts and many
  other metrics allow you to reliably detect very small optimizations and
  regressions of your code.
- **Consistent**: Gungraun can take accurate measurements even in virtualized CI
  environments and make them comparable between different systems completely
  negating the noise of the environment.
- **Fast**: Each benchmark is only run once, which is usually much faster than
  benchmarks which measure execution and wall-clock time. Benchmarks measuring
  the wall-clock time have to be run many times to increase their accuracy,
  detect outliers, filter out noise, etc.
- **Visualizable**: Gungraun generates a Callgrind (DHAT, ...) profile of the
  benchmarked code and can be configured to create flamegraph-like charts from
  Callgrind metrics. In general, all Valgrind-compatible tools like
  [callgrind_annotate][callgrind-annotate], [kcachegrind] or `dh_view.html` and
  others to analyze the results in detail are fully supported.
- **Easy**: The API for setting up benchmarks is easy to use and allows you to
  quickly create concise and clear benchmarks. Focus more on profiling and your
  code than on the framework.

## Design Philosophy and Goals

Gungraun benchmarks are designed to be runnable with `cargo bench`. The
benchmark files are expanded to a benchmarking harness which replaces the native
benchmark harness of `rust`. Gungraun is a profiling framework that can quickly
and reliably detect performance regressions and optimizations even in noisy
environments with a precision that is impossible to achieve with wall-clock time
based benchmarks. At the same time, we want to abstract the complicated parts
and repetitive tasks away and provide an easy to use and intuitive API. Gungraun
tries to stay out of your way so you can focus more on profiling and your code!

## When Not to Use Gungraun

Although Gungraun is useful in many projects, there are cases where Gungraun is
not a good fit.

- If you need only wall-clock times, Gungraun cannot help you much. The
  estimation of cpu cycles merely correlates to wall-clock times but is not a
  replacement for wall-clock times. The cycles estimation is primarily designed
  to be a relative metric to be used for comparison.
- Gungraun cannot be run on Windows or targets that support neither the
  Valgrind-based tools nor Linux perf.

## Pronunciation and Origin of the Word Gungraun

Like `Valgrind`, the name has its roots in old norse mythology and is a
composition of two words. The first is `gungnir`, Odin's legendary spear, in the
sense of one shot (benchmark execution) and one hit never missing its target.
The second word is `raun` which simply means test.

The first syllable is pronounced like the english word `gun`. The second `g` is
silent. The last syllable `raun` can be pronounced like the english word `rain`.

## Improving Gungraun

You want to improve the guide? You have an idea for a new feature, are missing a
functionality or have found a bug? We would love to hear about it. You want to
contribute and hack on Gungraun?

Please don't hesitate to [open an issue][open-an-issue].

You want to hack on this guide? The source code of this book lives in [the docs
subdirectory][docs-subdir].

[callgrind-annotate]:
    https://valgrind.org/docs/manual/cl-manual.html#cl-manual.callgrind_annotate-options
[callgrind-manual]: https://valgrind.org/docs/manual/cl-manual.html
[cachegrind-manual]: https://valgrind.org/docs/manual/cg-manual.html
[dhat-manual]: https://valgrind.org/docs/manual/dh-manual.html
[docs-subdir]: https://github.com/gungraun/gungraun/tree/main/docs
[kcachegrind]: https://kcachegrind.github.io/html/Home.html
[open-an-issue]: https://github.com/gungraun/gungraun/issues
