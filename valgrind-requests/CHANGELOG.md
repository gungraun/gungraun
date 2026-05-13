<!--
Added for new features.
Changed for changes in existing functionality.
Deprecated for soon-to-be removed features.
Removed for now removed features.
Fixed for any bug fixes.
Security in case of vulnerabilities.
-->

# Changelog

This is the CHANGELOG for the `valgrind-requests` package. All notable changes
to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to
[Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- ([#618]): Zero-indirection client requests for `s390x/linux`.
- ([#618]): Zero-indirection client requests for `powerpc/linux`,
  `powerpc64/linux`, and `powerpc64le/linux` with Rust 1.95.0 or newer.
- ([#618]): `VALGRIND_REQUESTS_STRATEGY` build configuration with `strict` and
  `fallback` modes to control whether native C FFI fallback is allowed.
- ([#618]): Illumos target detection to support zero-indirection client requests
  like Solaris targets.
- ([#622]): `no_std` support for `valgrind-requests`. Added `alloc` and `std`
  features. Core client request APIs and generated bindings no longer require
  `std`.
- ([#622]): Allocation-free `valgrind_print` and `valgrind_print_backtrace`
  functions which can be used in `no_std` environments unlike the
  `valgrind_printf` macro family.

### Changed

- ([#618]): Updated platform support detection and documentation in the README
  and library docs to describe zero-indirection support, native C FFI fallback
  behavior, and compile errors for targets unsupported by Valgrind.
- ([#618]): Fail early in the build script instead of during the build if there
  is no client request support for this platform by Valgrind.
- ([#622]): Either the `stubs` or `act` feature is now required for a successful
  compilation.

### Fixed

- ([#618]): Android targets no longer require the GNU target environment for
  Valgrind client request support and are added for zero-indirection like Linux
  targets.
- ([#618]): macOS target detection now uses Rust's `macos` target OS instead of
  `darwin`.
- ([#618]): RISC-V target detection now uses Rust's `riscv64` target
  architecture instead of `riscv64gc`.
- ([#618]): The x86_64 x32 ABI is excluded for Linux and Android targets,
  matching Valgrind's platform checks.
- ([#622]): The panic message for unavailable client requests now distinguishes
  detected old Valgrind versions from missing or too old Valgrind headers and
  suggests `VALGRIND_REQUESTS_VALGRIND_INCLUDE` when appropriate.

## [1.0.0] - 2026-04-30

This is the initial release which was largely extracted from the `gungraun`
package. Additionally includes some fixes and missing client requests.

### Added

- ([#604]): Initial extraction from the `gungraun` package.
- ([#603]): Helgrind client requests for mutex lifecycle (`mutex_init_post`,
  `mutex_lock_pre`, `mutex_lock_post`, `mutex_unlock_pre`, `mutex_unlock_post`,
  `mutex_destroy_pre`), semaphore operations (`sem_init_post`, `sem_wait_post`,
  `sem_post_pre`, `sem_destroy_pre`), barrier operations (`barrier_init_pre`,
  `barrier_wait_pre`, `barrier_resize_pre`, `barrier_destroy_pre`),
  `clean_memory_heapblock`, `disable_checking`, `enable_checking`, `get_abits`,
  and `gnat_dependent_master_join`.
- ([#603]): DRD client requests: `annotate_sem_init_pre`,
  `annotate_sem_destroy_post`, `annotate_sem_wait_pre`,
  `annotate_sem_wait_post`, `annotate_sem_post_pre`.
- ([#603]): Valgrind core client requests `replaces_malloc` and `get_toolname`
  from new Valgrind 3.27.0 release.

### Changed

- ([#603]): Refactored `valgrind::disable_error_reporting` to use
  `do_client_request!` macro consistently with other bindings. Unified import
  style to use `super::{...}` consistently instead of mixing `super::arch::`
  paths.

### Fixed

- ([#603]): `helgrind::annotate_rwlock_destroy` incorrectly using
  `GR_HG_PTHREAD_RWLOCK_INIT_POST` instead of
  `GR_HG_PTHREAD_RWLOCK_DESTROY_PRE`.
- ([#603]): `helgrind::annotate_rwlock_released` passing `is_writer_lock` as
  arg1, which Valgrind ignores. Now passes `0` and the parameter is ignored.
- ([#603]): Unnecessary generic parameter `T` on `memcheck::discard`.
- ([#603]): Missing trailing comma in RISC-V64 inline assembly.
- ([#603]): `build.rs`: Use `-isystem` and `-idirafter` instead of `-iquote` for
  include paths.
- ([#603]): `build.rs`: Fix operator precedence for Solaris platform detection.

[#603]: https://github.com/gungraun/gungraun/pull/603
[#604]: https://github.com/gungraun/gungraun/pull/604
[#618]: https://github.com/gungraun/gungraun/pull/618
[#622]: https://github.com/gungraun/gungraun/pull/622
