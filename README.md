<!-- spell-checker: ignore fixt binstall libtest eprintln usize Gjengset -->
<!-- markdownlint-disable MD041 MD033 -->

<h1 align="center">Gungraun</h1>

High-precision, one-shot and consistent benchmarking framework/harness for
Rust.<br>All Valgrind tools at your fingertips.

[Guide] | [API Docs] | [Changelog]<br>
[![Github action][github-action-badge]][github-action-url]
[![Crates.io][crates-io-badge]][crates-io-url]
[![docs.rs][docs-rs-badge]][docs-rs-url]

[crates-io-badge]: https://img.shields.io/crates/v/gungraun
[crates-io-url]: https://crates.io/crates/gungraun
[docs-rs-badge]: https://img.shields.io/docsrs/gungraun/latest
[docs-rs-url]: https://docs.rs/gungraun/0.19.2/gungraun/
[github-action-badge]:
    https://github.com/gungraun/gungraun/actions/workflows/cicd.yml/badge.svg
[github-action-url]:
    https://github.com/gungraun/gungraun/actions/workflows/cicd.yml
[API Docs]: https://docs.rs/crate/gungraun/
[Changelog]: https://github.com/gungraun/gungraun/blob/main/CHANGELOG.md

Gungraun leverages Valgrind's profiling tools like
[Callgrind][callgrind-manual], [Cachegrind][cachegrind-manual] and
[DHAT][dhat-manual] to provide extremely accurate and consistent measurements of
Rust code, making it perfectly suited to run in environments like a CI. Gungraun
aids in analyzing and optimizing code paths from the source code level down to
the assembly instruction level.

Gungraun is:

- **Precise**: High-precision measurements of `Instruction` counts,
  `Estimated Cycles` and many other metrics allow you to reliably detect very
  small optimizations and regressions of your code.
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

See the [guide] and api documentation at [docs.rs][api-docs] for all the
details.

## Quickstart/Documentation

To get started read the [guide] and see some introductory examples in
[Quickstart for library benchmarks][quickstart-library] or [Quickstart for
binary benchmarks][quickstart-binary]. A small migration check-list can be found
in the [Guide][migration-checklist].

If you need help or have questions, don't hesitate to [open an
issue][open-an-issue] or a [discussion]

## Design philosophy and goals

Gungraun benchmarks are designed to be runnable with `cargo bench`. The
benchmark files are expanded to a benchmarking harness which replaces the native
benchmark harness of Rust. Gungraun is a benchmarking and profiling framework
that can quickly and reliably detect performance regressions and optimizations
even in noisy environments with a precision that is impossible to achieve with
wall-clock time based benchmarks. At the same time, we want to abstract the
complicated parts and repetitive tasks away and provide an easy to use and
intuitive api. Gungraun tries to stay out of your way and applies sensible
default settings so you can focus more on profiling and your code!

## How far are we?

Gungraun is in a mature development stage and is already [in use][in-use].
Nevertheless, you may experience big changes between a minor version bump. The
main profiling tools `Callgrind`, `Cachegrind` and `DHAT` are fully integrated,
with full support for benchmarking async, multi-threaded and multi-process
applications. Please read our [Vision](./VISION.md) to learn more about the
ideas and the direction the future path might take.

## When not to use Gungraun

Although Gungraun is useful in many projects, there are cases where Gungraun is
not a good fit.

- If you need wall-clock times, Gungraun cannot help you much. The estimation of
  cpu cycles merely correlates to wall-clock times but is not a replacement for
  wall-clock times. The cycles estimation is primarily designed to be a relative
  metric to be used for comparison.
- Gungraun cannot be run on Windows or targets that support neither the
  Valgrind-based tools nor Linux perf.

## Contributing

Thanks for helping to improve this project! A guideline about contributing to
Gungraun can be found in the [CONTRIBUTING.md](./CONTRIBUTING.md) file.

You have an idea for a new feature, are missing a functionality or have found a
bug? [Open an issue][open-an-issue].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as in
[License](#license), without any additional terms or conditions.

## Pronunciation and origin of Gungraun

Like `valgrind`, the name has its roots in old norse mythology and is a
composition of two words. The first is `gungnir`, Odin's legendary spear, in the
sense of one shot (benchmark execution) and one hit never missing its target.
The second word is `raun` which simply means test.

The first syllable is pronounced like the english word `gun`. The second `g` is
silent. The last syllable `raun` can be pronounced like the english word `rain`.

## Links

- Gungraun/Iai-Callgrind is [mentioned][talk] in a talk at [RustNation
  UK][rustnation] about [Towards Impeccable Rust][talk-video] by Jon Gjengset.
- Gungraun is listed in the [The Rust Performance Book][perf-book].
- Gungraun and the predecessor Iai-Callgrind are supported by [bencher].

## Related Projects

- [Criterion-rs][criterion]: A Statistics-driven benchmarking library for Rust.
  Wall-clock times based benchmarks.
- [hyperfine]: A command-line benchmarking tool. Wall-clock time based
  benchmarks.
- [divan]: Statistically-comfy benchmarking library. Wall-clock times based
  benchmarks.
- [dhat-rs]: Provides heap profiling and ad hoc profiling capabilities to Rust
  programs, similar to those provided by DHAT.
- [cargo-valgrind]: A cargo subcommand, that runs Valgrind and collects its
  output in a helpful manner.

## Credits

Gungraun is forked from <https://github.com/bheisler/iai> and the original idea
is from Brook Heisler (@bheisler).

Gungraun is powered by [Valgrind].

## License

Gungraun is dual licensed under the Apache 2.0 license and the MIT license at
your option.

According to [Valgrind's documentation][valgrind-client-request-mechanism]:

> The Valgrind headers, unlike most of the rest of the code, are under a
> BSD-style license, so you may include them without worrying about license
> incompatibility.

We have included the original license where we made use of the original header
files.

[api-docs]: https://docs.rs/gungraun/latest/gungraun/
[bencher]: https://bencher.dev/learn/benchmarking/rust/gungraun/
[cargo-valgrind]: https://github.com/jfrimmel/cargo-valgrind
[criterion]: https://github.com/bheisler/criterion.rs
[dhat-rs]: https://github.com/nnethercote/dhat-rs
[discussion]: https://github.com/gungraun/gungraun/discussions
[divan]: https://github.com/nvzqz/divan
[guide]: https://gungraun.github.io/gungraun/
[hyperfine]: https://github.com/sharkdp/hyperfine
[in-use]: https://github.com/gungraun/gungraun/network/dependents
[kcachegrind]: https://kcachegrind.github.io/html/Home.html
[migration-checklist]:
    https://gungraun.github.io/gungraun/latest/html/migration/iai-callgrind-to-gungraun.html
[open-an-issue]: https://github.com/gungraun/gungraun/issues
[perf-book]: https://nnethercote.github.io/perf-book/benchmarking.html
[quickstart-binary]:
    https://gungraun.github.io/gungraun/latest/html/benchmarks/binary_benchmarks/quickstart.html
[quickstart-library]:
    https://gungraun.github.io/gungraun/latest/html/benchmarks/library_benchmarks/quickstart.html
[rustnation]: https://www.rustnationuk.com/
[talk]: https://youtu.be/qfknfCsICUM?t=1228
[talk-video]: https://www.youtube.com/watch?v=qfknfCsICUM
[Valgrind]: https://valgrind.org/
[valgrind-client-request-mechanism]:
    https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
[callgrind-manual]: https://valgrind.org/docs/manual/cl-manual.html
[cachegrind-manual]: https://valgrind.org/docs/manual/cg-manual.html
[dhat-manual]: https://valgrind.org/docs/manual/dh-manual.html
[callgrind-annotate]:
    https://valgrind.org/docs/manual/cl-manual.html#cl-manual.callgrind_annotate-options
