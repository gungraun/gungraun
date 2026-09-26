# Migration

From version `0.17.0` onwards this project's name is `Gungraun`. The packages
were renamed from `iai-callgrind` to `gungraun`, `iai-callgrind-runner` to
`gungraun-runner` and `iai-callgrind-macros` to `gungraun-macros`.

## Migration Check-List

- Update the library: Rename `iai-callgrind` to `gungraun` in your `Cargo.toml`
  and use a version `>=0.17.0`.
- Update all usages of `use iai_callgrind` to `use gungraun`.
- Update the binary: Uninstall the old binary with
  `cargo uninstall iai-callgrind-runner`. Install the new binary for example
  with binstall: `cargo binstall gungraun-runner@0.20.0`
- Update any scripts which installed `iai-callgrind-runner` in the CI to use
  `gungraun-runner`.
- If you are parsing the benchmark output: The summary line has changed from
  `Iai-Callgrind result: Ok, ...` to `Gungraun result: Ok, ...`
- Update any environment variable names to use the `GUNGRAUN` prefix instead of
  `IAI_CALLGRIND`. For example `IAI_CALLGRIND_LOG=warn` -> `GUNGRAUN_LOG=warn`
  or `IAI_CALLGRIND_VALGRIND_INCLUDE=...` to `GUNGRAUN_VALGRIND_INCLUDE=...`.
- If you want to keep the old benchmark output from the `target/iai` directory,
  simply rename it to `target/gungraun`.
- Rename benchmark files to use `gungraun` instead of `iai` or `iai_callgrind`.
  This will also change the output directory. If you want to keep the old files,
  for example when renaming the old benchmarks of the
  `my_iai_callgrind_benchmarks.rs` file to `my_gungraun_benchmarks.rs`, then the
  output directory changes from
  `target/iai/my_package/my_iai_callgrind_benchmarks` to
  `target/gungraun/my_package/my_gungraun_benchmarks`

From version `0.18.2` onwards the client requests were extracted into a separate
package `valgrind-requests`. The following environment variables were renamed:

- `IAI_CALLGRIND_VALGRIND_INCLUDE`, `IAI_CALLGRIND_.*_VALGRIND_INCLUDE`,
  `GUNGRAUN_VALGRIND_INCLUDE`, `GUNGRAUN_.*_VALGRIND_INCLUDE` to
  `VALGRIND_REQUESTS_VALGRIND_INCLUDE`, `VALGRIND_REQUESTS_.*_VALGRIND_INCLUDE`
