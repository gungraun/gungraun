//! Contains macros which together define a benchmark harness that can be used in place of the
//! standard benchmark harness. This allows the user to run Gungraun benchmarks with `cargo bench`.

/// [low level api](`crate::binary_benchmark_group`) only: Use to add a `#[binary_benchmark]` to a
/// [`crate::BinaryBenchmarkGroup`]
///
/// # Examples
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::binary_benchmark_attribute;
/// use gungraun::prelude::*;
///
/// #[binary_benchmark]
/// fn bench_binary() -> gungraun::Command {
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .arg("foo")
///         .build()
/// }
///
/// binary_benchmark_group!(
///     name = my_group,
///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
///         group.binary_benchmark(binary_benchmark_attribute!(bench_binary));
///     }
/// );
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! binary_benchmark_attribute {
    ($name:ident) => {{
        let mut binary_benchmark = $crate::BinaryBenchmark::new(stringify!($name));
        binary_benchmark.config = $name::__get_config();

        for internal_bench in $name::__BENCHES {
            let mut bench = if let Some(id) = internal_bench.id_display {
                $crate::Bench::new(id)
            } else {
                $crate::Bench::new(stringify!($name))
            };

            match internal_bench.func {
                $crate::__internal::InternalBinFunctionKind::Default(func) => {
                    bench.command(func());
                }
                $crate::__internal::InternalBinFunctionKind::Iter(func) => {
                    bench
                        .commands
                        .push($crate::__internal::InternalCommandKind::Iter(
                            func().iter().map(Into::into).collect(),
                        ));
                }
            }

            bench.setup = internal_bench.setup;
            bench.teardown = internal_bench.teardown;

            if let Some(config) = internal_bench.config {
                bench.config(config());
            }

            binary_benchmark.bench(bench);
        }
        binary_benchmark
    }};
}

/// The `gungraun::main` macro expands to a `main` function which runs all the benchmarks.
///
/// Using Gungraun requires disabling the benchmark harness. This can be done like so in the
/// `Cargo.toml` file:
///
/// ```toml
/// [[bench]]
/// name = "my_bench"
/// harness = false
/// ```
///
/// To be able to run any gungraun benchmarks, you'll also need the `gungraun-runner`
/// installed with the binary somewhere in your `$PATH` for example with
///
/// ```shell
/// cargo install gungraun-runner
/// ```
///
/// `my_bench` has to be a rust file inside the 'benches' directory.
///
/// # Library Benchmarks
///
/// The [`crate::main`] macro has one form to run library benchmarks:
///
/// ```rust
/// # use gungraun::prelude::*;
/// # #[library_benchmark]
/// # fn bench_fibonacci() { }
/// # library_benchmark_group!(
/// #    name = some_group,
/// #    benchmarks = bench_fibonacci
/// # );
/// # fn main() {
/// main!(library_benchmark_groups = some_group);
/// # }
/// ```
///
/// which accepts the following top-level arguments in this order (separated by a comma):
///
/// * __`config`__ (optional): Optionally specify a [`crate::LibraryBenchmarkConfig`] valid for all
///   benchmark groups
/// * __`setup`__ (optional): A setup function or any valid expression which is run before all
///   benchmarks
/// * __`teardown`__ (optional): A setup function or any valid expression which is run after all
///   benchmarks
/// * __`library_benchmark_groups`__ (mandatory): The __name__ of one or more
///   [`library_benchmark_group!`](crate::library_benchmark_group) macros. Multiple __names__ are
///   expected to be a comma separated array (`library_benchmark_groups = [group_1, group_2]`).
///
/// A library benchmark consists of
/// [`library_benchmark_groups`](crate::library_benchmark_group) and with
/// [`#[library_benchmark]`](crate::library_benchmark) annotated benchmark functions.
///
/// ```rust
/// use std::hint::black_box;
///
/// use gungraun::prelude::*;
///
/// fn fibonacci(n: u64) -> u64 {
///     match n {
///         0 => 1,
///         1 => 1,
///         n => fibonacci(n - 1) + fibonacci(n - 2),
///     }
/// }
///
/// #[library_benchmark]
/// #[bench::short(10)]
/// #[bench::long(30)]
/// fn bench_fibonacci(value: u64) -> u64 {
///     black_box(fibonacci(black_box(value)))
/// }
///
/// library_benchmark_group!(name = bench_fibonacci_group, benchmarks = bench_fibonacci);
///
/// # fn main() {
/// main!(library_benchmark_groups = bench_fibonacci_group);
/// # }
/// ```
///
/// If you need to pass arguments to Valgrind's Callgrind or other tools, you can specify arguments
/// via [`crate::Callgrind::args`], [`crate::Dhat::args`], ...:
///
/// ```rust
/// # use gungraun::prelude::*;
/// # use gungraun::Callgrind;
/// # #[library_benchmark]
/// # fn bench_fibonacci() { }
/// # library_benchmark_group!(
/// #    name = some_group,
/// #    benchmarks = bench_fibonacci
/// # );
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args([
///         "--arg-with-flags=yes",
///         "arg-without-flags=is_ok_too"
///     ])),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
///
/// See also [Callgrind Command-line
/// options](https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options).
///
/// For an in-depth description of library benchmarks and more examples see the
/// [guide](https://gungraun.github.io/gungraun/latest/html/benchmarks/library_benchmarks.html).
///
/// # Binary Benchmarks
///
/// Setting up binary benchmarks is almost the same as setting up library benchmarks but using the
/// `#[binary_benchmark]` macro. For example, if you're crate's binary is called `my-foo`:
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::prelude::*;
///
/// #[binary_benchmark]
/// #[bench::hello_world("hello world")]
/// #[bench::foo("foo")]
/// fn bench_binary(arg: &str) -> gungraun::Command {
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .arg(arg)
///         .build()
/// }
///
/// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
///
/// # fn main() {
/// main!(binary_benchmark_groups = my_group);
/// # }
/// ```
///
/// See the documentation of [`crate::binary_benchmark_group`] and [`crate::Command`] for more
/// details.
#[macro_export]
macro_rules! main {
    (
        $( config = $config:expr; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        binary_benchmark_groups =
    ) => {
        compile_error!("The binary_benchmark_groups argument needs at least one \
            `name` of a `binary_benchmark_group!`");
    };
    (
        $( config = $config:expr; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        binary_benchmark_groups = $( $group:ident ),+ $(,)*
    ) => {
        fn __run() -> Result<(), $crate::__internal::error::Errors> {
            if option_env!("IAI_CALLGRIND_RUNNER").is_some() &&
                option_env!("GUNGRAUN_RUNNER").is_none() {
                    eprintln!(
                        "gungraun: WARNING: With version 0.17.0, the environment variable
                        `IAI_CALLGRIND_RUNNER` has changed to `GUNGRAUN_RUNNER`.\n The
                        `GUNGRAUN_RUNNER` variable has to point to the absolute path of the new
                        `gungraun-runner` executable."
                    );
            }

            let mut this_args = std::env::args();
            let mut runner = $crate::__internal::Runner::new(
                option_env!("GUNGRAUN_RUNNER").or_else(||
                            option_env!("CARGO_BIN_EXE_gungraun-runner")
                ),
                &$crate::__internal::BenchmarkKind::BinaryBenchmark,
                env!("CARGO_MANIFEST_DIR"),
                env!("CARGO_PKG_NAME"),
                file!(),
                module_path!(),
                this_args.next().unwrap(),
            );

            let mut config: Option<$crate::__internal::InternalBinaryBenchmarkConfig> = None;
            $(
                config = Some($config.into());
            )?

            let mut groups_builder = $crate::__internal::bin_bench::GroupsBuilder::new(
                config, this_args.collect(), __run_setup(false), __run_teardown(false),
            );

            $(
                let mut group = $crate::BinaryBenchmarkGroup::default();
                $group::$group(&mut group);

                groups_builder.add_group(
                        group,
                        stringify!($group).to_owned(),
                        &module_path!(),
                        $group::__IS_ATTRIBUTE,
                        $group::__get_config(),
                        $group::__run_setup(false),
                        $group::__run_teardown(false),
                        $group::__compare_by_id(),
                        $group::__max_parallel(),
                        $group::__BENCHES
                );
            )+

            let groups = groups_builder.build()?;
            let encoded = $crate::bincode::serde::encode_to_vec(
                &groups, $crate::bincode::config::legacy()
            ).expect("Encoded benchmark");
            runner.exec(encoded)
        }

        fn __run_setup(__run: bool) -> bool {
            let mut __has_setup = false;
            $(
                __has_setup = true;
                if __run {
                    $setup;
                }
            )?
            __has_setup
        }

        fn __run_teardown(__run: bool) -> bool {
            let mut __has_teardown = false;
            $(
                __has_teardown = true;
                if __run {
                    $teardown;
                }
            )?
            __has_teardown
        }

        fn main() {
            const GROUPS: &[
                (
                    fn(bool) -> bool,
                    fn(bool) -> bool,
                    fn(usize, usize, Option<usize>),
                    fn(usize, usize, Option<usize>)
                )
            ] = &[
                $(
                    (
                        $group::__run_setup,
                        $group::__run_teardown,
                        $group::__run_bench_setup,
                        $group::__run_bench_teardown,
                    )
                ),*
            ];

            let mut args_iter = std::env::args().skip(1);
            if args_iter
                .next()
                .as_ref()
                .map_or(false, |value| value == "--gungraun-run")
            {
                let _ = args_iter
                    .next()
                    .expect("A benchmark run mode should be present");
                let mut current = args_iter.next().expect("At least two values should be present");
                let next = args_iter.next();
                match (current.as_str(), next) {
                    ("setup", None) => {
                        __run_setup(true);
                    },
                    ("teardown", None) => {
                        __run_teardown(true);
                    },
                    (index, Some(next)) => {
                        let main_index = index
                            .parse::<usize>()
                            .expect("A valid main index should be present");
                        let current = next;
                        let next = args_iter.next();

                        match (current.as_str(), next) {
                            ("setup", None) => {
                                (GROUPS[main_index].0)(true);
                            },
                            ("teardown", None) => {
                                (GROUPS[main_index].1)(true);
                            }
                            (key @ ("setup" | "teardown"), Some(next)) => {
                                let group_index = next
                                        .parse::<usize>()
                                        .expect("The group index should be a valid number");
                                let bench_index = args_iter
                                        .next()
                                        .expect("The bench index should be present")
                                        .parse::<usize>()
                                        .expect("The bench index should be a valid number");
                                let iter_index = args_iter
                                    .next()
                                    .and_then(|a| a.parse::<usize>().ok());
                                if key == "setup" {
                                    (GROUPS[main_index].2)(
                                        group_index,
                                        bench_index,
                                        iter_index
                                    );
                                } else {
                                    (GROUPS[main_index].3)(
                                        group_index,
                                        bench_index,
                                        iter_index
                                    );
                                }
                            }
                            (name, _) => panic!(
                                "Invalid function '{}' in group with index '{}'",
                                name, main_index
                            )
                        }
                    }
                    (name, _) => panic!(
                        "Invalid configuration with value '{}' found in this scope",
                        name
                    )
                }
            } else {
                if let Err(errors) = __run() {
                    eprintln!("{errors}");
                    std::process::exit(1);
                }
            };
        }
    };
    (
        $( config = $config:expr, $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        binary_benchmark_groups = $group:ident
    ) => {
        main!(
            $( config = $config; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            binary_benchmark_groups = $group
        );
    };
    (
        $( config = $config:expr, $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        binary_benchmark_groups = [ $( $group:ident ),+ $(,)* ]
    ) => {
        main!(
            $( config = $config; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            binary_benchmark_groups = $( $group ),+
        );
    };
    (
        $( config = $config:expr; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        library_benchmark_groups =
    ) => {
        compile_error!("The library_benchmark_groups argument needs at least one \
            `name` of a `library_benchmark_group!`");
    };
    (
        $( config = $config:expr ; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        library_benchmark_groups = $( $group:ident ),+ $(,)*
    ) => {
        #[inline(never)]
        fn __run() {
            if option_env!("IAI_CALLGRIND_RUNNER").is_some() &&
                option_env!("GUNGRAUN_RUNNER").is_none() {
                    eprintln!(
                        "gungraun: WARNING: With version 0.17.0, the environment variable
                        `IAI_CALLGRIND_RUNNER` has changed to `GUNGRAUN_RUNNER`.\n The
                        `GUNGRAUN_RUNNER` variable has to point to the absolute path of the new
                        `gungraun-runner` executable."
                    );
            }

            let mut this_args = std::env::args();
            let mut runner = $crate::__internal::Runner::new(
                option_env!("GUNGRAUN_RUNNER").or_else(||
                    option_env!("CARGO_BIN_EXE_gungraun-runner")
                ),
                &$crate::__internal::BenchmarkKind::LibraryBenchmark,
                env!("CARGO_MANIFEST_DIR"),
                env!("CARGO_PKG_NAME"),
                file!(),
                module_path!(),
                this_args.next().unwrap(),
            );

            let mut config: Option<$crate::__internal::InternalLibraryBenchmarkConfig> = None;
            $(
                config = Some($config.into());
            )?

            let mut groups_builder = $crate::__internal::lib_bench::GroupsBuilder::new(
                config, this_args.collect(), __run_setup(false), __run_teardown(false),
            );

            $(
                groups_builder.add_group(
                    stringify!($group).to_owned(),
                    $group::__get_config(),
                    $group::__compare_by_id(),
                    $group::__max_parallel(),
                    $group::__run_setup(false),
                    $group::__run_teardown(false),
                    $group::__BENCHES
                );
            )+

            let encoded = $crate::bincode::serde::encode_to_vec(
                groups_builder.build(), $crate::bincode::config::legacy()
            ).expect("Encoded benchmark");

            if let Err(errors) = runner.exec(encoded) {
                eprintln!("{errors}");
                std::process::exit(1);
            }
        }

        #[inline(never)]
        fn __run_setup(__run: bool) -> bool {
            let mut __has_setup = false;
            $(
                __has_setup = true;
                if __run {
                    $setup;
                }
            )?
            __has_setup
        }

        #[inline(never)]
        fn __run_teardown(__run: bool) -> bool {
            let mut __has_teardown = false;
            $(
                __has_teardown = true;
                if __run {
                    $teardown;
                }
            )?
            __has_teardown
        }

        /// Keep the logic to run the benchmark function within the main function to avoid heap
        /// allocations in functions other than main which are part of the benchmarking framework.
        /// DHAT needs a fallback (matched with `file::*::*`) to the benchmark function (matched
        /// with `*::__gungraun_wrapper_mod::*`) since the benchmark function is sometimes
        /// inlined even if annotated with `#[inline(never)]`.
        fn main() {
            const GROUPS: &[
                (
                    fn(bool) -> bool,
                    fn(bool) -> bool,
                    fn($crate::__internal::InternalBenchRunMode, usize, usize, Option<usize>)
                )
            ] = &[
                $(
                    (
                        $group::__run_setup,
                        $group::__run_teardown,
                        $group::__run
                    )
                ),*
            ];

            let mut args_iter = std::hint::black_box(std::env::args()).skip(1);
            if args_iter
                .next()
                .as_ref()
                .map_or(false, |value| value == "--gungraun-run")
            {
                let mut current = std::hint::black_box(
                    args_iter.next().expect("A mode should be present")
                );

                let mode = std::hint::black_box(
                    $crate::__internal::InternalBenchRunMode::from_id(&current)
                        .expect("The benchmark run mode should be valid")
                );

                current = std::hint::black_box(
                    args_iter.next().expect("Expecting a function type")
                );

                let mut next = std::hint::black_box(args_iter.next());
                match current.as_str() {
                    "setup" if next.is_none() => {
                        __run_setup(true);
                    },
                    "teardown" if next.is_none() => {
                        __run_teardown(true);
                    },
                    _ => {
                        let main_index = std::hint::black_box(
                            current.parse::<usize>()
                            .expect("The main index should be a valid integer")
                        );

                        current = next.expect("A setup/teardown or group index should be present");
                        next = std::hint::black_box(args_iter.next());
                        match current.as_str() {
                            "setup" if next.is_none() => {
                                (GROUPS[main_index].0)(true);
                            },
                            "teardown" if next.is_none() => {
                                (GROUPS[main_index].1)(true);
                            }
                            _ => {
                                let group_index = std::hint::black_box(
                                    current
                                        .parse::<usize>()
                                        .expect("Expecting a valid group index")
                                );
                                let bench_index = std::hint::black_box(
                                    next.expect("A bench index should be present")
                                        .parse::<usize>()
                                        .expect("Expecting a valid bench index")
                                );
                                let iter_index = std::hint::black_box(
                                    args_iter
                                        .next()
                                        .and_then(|a| a.parse::<usize>().ok())
                                );
                                (GROUPS[main_index].2)
                                    (mode, group_index, bench_index, iter_index);
                            }
                        }
                    }
                    name => panic!("function '{}' not found in this scope", name)
                }
            } else {
                std::hint::black_box(__run());
            };
        }
    };
    (
        $( config = $config:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        library_benchmark_groups = $group:ident
    ) => {
        main!(
            $( config = $config; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            library_benchmark_groups = $group
        );
    };
    (
        $( config = $config:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        library_benchmark_groups = [ $( $group:ident ),+ $(,)* ]
    ) => {
        main!(
            $( config = $config; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            library_benchmark_groups = $( $group ),+
        );
    };
    ( $( $some:tt )* ) => {
        compile_error!(
            "You are using an invalid or deprecated syntax of the main! macro to set up \
            benchmarks. See the documentation of this macro, the README \
            (https://github.com/gungraun/gungraun) and docs \
            (https://docs.rs/gungraun/latest/gungraun/) for further details."
        );
        pub fn main() {}
    };
}

/// Macro used to define a group of binary benchmarks
///
/// There are two apis to set up binary benchmarks. The recommended way is to [use the
/// `#[binary_benchmark]` attribute](#using-the-high-level-api-with-the-binary-benchmark-attribute).
/// But, if you find yourself in the situation that the attribute isn't enough you can fall back to
/// the [low level api](#the-low-level-api) or even [intermix both
/// styles](#intermixing-both-apis).
///
/// # The macro's arguments in detail:
///
/// The following top-level arguments are accepted (in this order):
///
/// ```rust
/// # use gungraun::prelude::*;
/// # use gungraun::BinaryBenchmarkGroup;
/// # fn run_setup() {}
/// # fn run_teardown() {}
/// # #[binary_benchmark]
/// # fn bench_1() -> gungraun::Command { gungraun::Command::new("some") }
/// # #[binary_benchmark]
/// # fn bench_2() -> gungraun::Command { gungraun::Command::new("some") }
/// binary_benchmark_group!(
///     name = my_group,
///     config = BinaryBenchmarkConfig::default(),
///     compare_by_id = false,
///     max_parallel = 1,
///     setup = run_setup(),
///     teardown = run_teardown(),
///     benchmarks = [bench_1, bench_2]
/// );
/// # fn main() {
/// # my_group::my_group(&mut BinaryBenchmarkGroup::default());
/// # }
/// ```
///
/// * __`name`__ (mandatory): A unique name used to identify the group for the `main!` macro
/// * __`config`__ (optional): A [`crate::BinaryBenchmarkConfig`]
/// * __`compare_by_id`__ (optional): The default is false. If true, all commands from the functions
///   specified in the `benchmarks` argument, are compared with each other as long as the ids (the
///   part after the `::` in `#[bench::id(...)]`) match.
/// * __`max_parallel`__ (optional): The default is no limit. If set to a value, `0` means no limit
///   (same as not specifying this option), `1` disables parallel execution for this group, and
///   values `>= 2` limit the maximum number of parallel benchmarks to run within this group. This
///   option does nothing unless parallel execution is enabled with `--parallel` or
///   `GUNGRAUN_PARALLEL`.
/// * __`setup`__ (optional): A function which is executed before all benchmarks in this group
/// * __`teardown`__ (optional): A function which is executed after all benchmarks in this group
/// * __`benchmarks`__ (mandatory): A `,`-separated array of `#[binary_benchmark]` annotated
///   function names you want to put into this group. Or, if you want to use the low level api
///
///   `|IDENTIFIER: &mut BinaryBenchmarkGroup| EXPRESSION`
///
///   or the shorter `|IDENTIFIER| EXPRESSION`
///
///   where `IDENTIFIER` is the identifier of your choice for the `BinaryBenchmarkGroup` (we use
///   `group` throughout our examples) and `EXPRESSION` is the code where you make use of the
///   `BinaryBenchmarkGroup` to set up the binary benchmarks
///
/// # Using the high-level api with the `#[binary benchmark]` attribute
///
/// A small introductory example which demonstrates the basic setup (assuming a crate's binary is
/// named `my-foo`):
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::BinaryBenchmarkGroup;
/// use gungraun::prelude::*;
///
/// #[binary_benchmark]
/// #[bench::hello_world("hello world")]
/// #[bench::foo("foo")]
/// #[benches::multiple("bar", "baz")]
/// fn bench_binary(arg: &str) -> gungraun::Command {
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .arg(arg)
///         .build()
/// }
///
/// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
///
/// # fn main() {
/// gungraun::main!(binary_benchmark_groups = my_group);
/// # }
/// ```
///
/// To be benchmarked a `binary_benchmark_group` has to be added to the `main!` macro by adding its
/// name to the `binary_benchmark_groups` argument of the `main!` macro. See there for further
/// details about the [`crate::main`] macro. See the documentation of [`crate::binary_benchmark`]
/// for more details about the attribute itself and the inner attributes `#[bench]` and
/// `#[benches]`.
///
/// # The low-level api
///
/// Using the low-level api has advantages but when it comes to stability in terms of usability, the
/// low level api might be considered less stable. What does this mean? If we have to make changes
/// to the inner workings of gungraun which not necessarily change the high-level api it is
/// more likely that the low-level api has to be adjusted. This implies you might have to adjust
/// your benchmarks more often with a version update of `gungraun`. Hence, it is recommended to
/// use the high-level api as much as possible and only use the low-level api under special
/// circumstances. You can also [intermix both styles](#intermixing-both-apis)!
///
/// The low-level api mirrors the high-level constructs as close as possible. The
/// [`crate::BinaryBenchmarkGroup`] is a special case, since we use the information from the
/// `binary_benchmark_group!` macro [arguments](#the-macros-arguments-in-detail) (__`name`__,
/// __`config`__, ...) to create the `BinaryBenchmarkGroup` and pass it to the `benchmarks`
/// argument.
///
/// That being said, here's the basic usage:
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::prelude::*;
/// use gungraun::{Bench, BinaryBenchmark};
///
/// binary_benchmark_group!(
///     // All the other options from the `binary_benchmark_group` are used as usual
///     name = my_group,
///     // Note there's also the shorter form `benchmarks = |group|` but in the examples we want
///     // to be more explicit
///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
///         // We have chosen `group` to be our identifier but it can be anything
///         group.binary_benchmark(
///             // This is the equivalent of the `#[binary_benchmark]` attribute. The `id`
///             // mirrors the function name of the `#[binary_benchmark]` annotated function.
///             BinaryBenchmark::new("some_id").bench(
///                 // The equivalent of the `#[bench]` attribute.
///                 Bench::new("my_bench_id").command(
///                     // The `Command` stays the same
///                     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///                         .arg("foo")
///                         .build(),
///                 ),
///             ),
///         )
///     }
/// );
/// # fn main() {}
/// ```
///
/// Depending on your IDE, it's nicer to work with the code after the `|group: &mut
/// BinaryBenchmarkGroup|` if it resides in a separate function rather than the macro itself as in
///
/// ```rust
/// use gungraun::prelude::*;
/// use gungraun::{Bench, BinaryBenchmark, BinaryBenchmarkGroup};
///
/// fn setup_my_group(group: &mut BinaryBenchmarkGroup) {
///     // Enjoy all the features of your IDE ...
/// }
///
/// binary_benchmark_group!(
///     name = my_group,
///     benchmarks = |group: &mut BinaryBenchmarkGroup| setup_my_group(group)
/// );
/// # fn main() {}
/// ```
///
/// The list of all structs and macros used exclusively in the low-level api:
/// * [`crate::BinaryBenchmarkGroup`]
/// * [`crate::BinaryBenchmark`]: Mirrors the `#[binary_benchmark]` attribute
/// * [`crate::Bench`]: Mirrors the `#[bench]` attribute
/// * [`crate::binary_benchmark_attribute`]: Used to add a `#[binary_benchmark]` attributed function
///   in [`crate::BinaryBenchmarkGroup::binary_benchmark`]
/// * [`crate::BenchmarkId`]: The benchmark id is for example used in
///   [`crate::BinaryBenchmark::new`] and [`crate::Bench::new`]
///
/// Note there's no equivalent for the `#[benches]` attribute. The [`crate::Bench`] behaves exactly
/// as the `#[benches]` attribute if more than a single [`crate::Command`] is added.
///
/// # Intermixing both apis
///
/// For example, if you started with the `#[binary_benchmark]` attribute and noticed you are limited
/// by it to set up all the [`crate::Command`]s the way you want, you can intermix both styles:
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::prelude::*;
/// use gungraun::{Bench, BinaryBenchmark, BinaryBenchmarkGroup, binary_benchmark_attribute};
///
/// #[binary_benchmark]
/// #[bench::foo("foo")]
/// #[benches::multiple("bar", "baz")]
/// fn bench_binary(arg: &str) -> gungraun::Command {
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .arg(arg)
///         .build()
/// }
///
/// fn setup_my_group(group: &mut BinaryBenchmarkGroup) {
///     group
///         // Simply add what you already have with the `binary_benchmark_attribute!` macro.
///         // This macro returns a `BinaryBenchmark`, so you could even add more `Bench`es
///         // to it instead of creating a new one as we do below
///         .binary_benchmark(binary_benchmark_attribute!(bench_binary))
///         .binary_benchmark(
///             BinaryBenchmark::new("did_not_work_with_attribute")
///                 .bench(Bench::new("low_level")
///                     .command(
///                         gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///                             .arg("foo")
///                             .build()
///                     )
///                 )
///         );
/// }
///
/// binary_benchmark_group!(
///     name = my_group,
///     benchmarks = |group: &mut BinaryBenchmarkGroup| setup_my_group(group)
/// );
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! binary_benchmark_group {
    () => {
        compile_error!("A binary_benchmark_group! needs a name and at least 1 benchmark \
            function annotated with #[binary_benchmark] or the low level syntax. See the \
            documentation of this macro for more details.\n\n\
            hint: binary_benchmark_group!(name = some_ident, \
                benchmarks = [ benchmark_1, benchmark_2 ]);");
    };
    (
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks = $( $some:tt )*
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs a unique name. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident, benchmarks = some_binary_benchmark);"
        );
    };
    (
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = $( $some:tt )*
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs a unique name. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident; benchmarks = some_binary_benchmark);"
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr; $(;)* )?
        $( compare_by_id = $compare:literal; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks =
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs at least 1 benchmark function which is annotated with \
            #[binary_benchmark] or you can use the low level syntax. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident; benchmarks = some_binary_benchmark);"
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs at least 1 benchmark function which is annotated with \
            #[binary_benchmark] or you can use the low level syntax. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident; benchmarks = some_binary_benchmark);"
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = $( $function:ident ),+ $(,)*
    ) => {
        pub mod $name {
            use super::*;

            pub const __IS_ATTRIBUTE: bool = true;

            pub const __BENCHES: &[&(
                &'static str,
                fn() -> Option<$crate::__internal::InternalBinaryBenchmarkConfig>,
                &[$crate::__internal::InternalMacroBinBench]
            )]= &[
                $(
                    &(
                        stringify!($function),
                        super::$function::__get_config,
                        super::$function::__BENCHES
                    )
                ),+
            ];

            pub fn __run_setup(__run: bool) -> bool {
                let mut __has_setup = false;
                $(
                    __has_setup = true;
                    if __run {
                        $setup;
                    }
                )?
                __has_setup
            }

            pub fn __run_teardown(__run: bool) -> bool {
                let mut __has_teardown = false;
                $(
                    __has_teardown = true;
                    if __run {
                        $teardown;
                    }
                )?
                __has_teardown
            }

            pub fn __compare_by_id() -> Option<bool> {
                let mut comp = None;
                $(
                    comp = Some($compare);
                )?
                comp
            }

            pub fn __max_parallel() -> Option<usize> {
                let mut __par = None;
                $(
                    let expr: usize = $parallel;
                    __par = Some(expr);
                )?
                __par
            }

            pub fn __get_config() -> Option<$crate::__internal::InternalBinaryBenchmarkConfig> {
                let mut config = None;
                $(
                    config = Some($config.into());
                )?
                config
            }

            pub fn __run_bench_setup(
                group_index: usize, bench_index: usize, iter_index: Option<usize>
            ) {
                match __BENCHES[group_index].2[bench_index].setup {
                    $crate::__internal::InternalBinAssistantKind::Iter(setup) =>
                        setup(iter_index),
                    $crate::__internal::InternalBinAssistantKind::Default(setup) =>
                        setup(),
                    $crate::__internal::InternalBinAssistantKind::None => {
                        panic!("This function was called without a setup function being present")
                    }
                }
            }

            pub fn __run_bench_teardown(
                group_index: usize, bench_index: usize, iter_index: Option<usize>
            ) {
                match __BENCHES[group_index].2[bench_index].teardown {
                    $crate::__internal::InternalBinAssistantKind::Iter(teardown) =>
                        teardown(iter_index),
                    $crate::__internal::InternalBinAssistantKind::Default(teardown) =>
                        teardown(),
                    $crate::__internal::InternalBinAssistantKind::None => {
                        panic!("This function was called without a teardown function being present")
                    }
                }
            }

            pub fn $name(_: &mut $crate::BinaryBenchmarkGroup) {}
        }
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr, $(,)* )?
        $( compare_by_id = $compare:literal, $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks =
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs at least 1 benchmark function which is annotated with \
            #[binary_benchmark] or you can use the low level syntax. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident, benchmarks = some_binary_benchmark);"
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
    ) => {
        compile_error!(
            "A binary_benchmark_group! needs at least 1 benchmark function which is annotated with \
            #[binary_benchmark] or you can use the low level syntax. See the documentation of this \
            macro for further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident, benchmarks = some_binary_benchmark);"
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks = $function:ident
    ) => {
        binary_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = $function
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks = [ $( $function:ident ),+ $(,)* ]
    ) => {
        binary_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = $( $function ),+
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = |$group:ident|
    ) => {
        compile_error!(
            "This low level form of the binary_benchmark_group! needs you to use the \
            `BinaryBenchmarkGroup` to setup benchmarks. See the documentation of this macro for \
            further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident, benchmarks = |group| { \
                group.binary_benchmark(/* BinaryBenchmark::new */); });"
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = |$group:ident: &mut BinaryBenchmarkGroup|
    ) => {
        compile_error!(
            "This low level form of the binary_benchmark_group! needs you to use the \
            `BinaryBenchmarkGroup` to setup benchmarks. See the documentation of this macro for \
            further details.\n\n\
            hint = binary_benchmark_group!(name = some_ident, benchmarks = |group: &mut \
                BinaryBenchmarkGroup| { group.binary_benchmark(/* BinaryBenchmark::new */); });"
        );
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = |$group:ident: &mut BinaryBenchmarkGroup| $body:expr
    ) => {
        pub mod $name {
            use super::*;

            pub const __IS_ATTRIBUTE: bool = false;

            pub const __BENCHES: &[&(
                &'static str,
                fn() -> Option<$crate::__internal::InternalBinaryBenchmarkConfig>,
                &[$crate::__internal::InternalMacroBinBench]
            )]= &[];

            pub fn __run_setup(__run: bool) -> bool {
                let mut __has_setup = false;
                $(
                    __has_setup = true;
                    if __run {
                        $setup;
                    }
                )?
                __has_setup
            }

            pub fn __run_teardown(__run: bool) -> bool {
                let mut __has_teardown = false;
                $(
                    __has_teardown = true;
                    if __run {
                        $teardown;
                    }
                )?
                __has_teardown
            }

            pub fn __get_config() -> Option<$crate::__internal::InternalBinaryBenchmarkConfig> {
                let mut config = None;
                $(
                    config = Some($config.into());
                )?
                config
            }

            pub fn __compare_by_id() -> Option<bool> {
                let mut comp = None;
                $(
                    comp = Some($compare);
                )?
                comp
            }

            pub fn __max_parallel() -> Option<usize> {
                let mut __par = None;
                $(
                    let expr: usize = $parallel;
                    __par = Some(expr);
                )?
                __par
            }

            pub fn __run_bench_setup(
                group_index: usize, bench_index: usize, iter_index: Option<usize>
            ) {
                let mut group = $crate::BinaryBenchmarkGroup::default();
                $name(&mut group);

                let binary_benchmark = group
                    .binary_benchmarks
                    .iter()
                    .nth(group_index)
                    .expect("The group index for setup should be present");
                // In the runner each command is a `BinBench` and it is the index of the command
                // which we're getting back from the runner. So, we have to iterate over the
                // commands of each Bench to extract the correct setup function.
                //
                // commands                           => bench_index => The correct setup function
                // bench.benches[0].commands = [a, b] => 0, 1        => bench.benches[0].setup
                // bench.benches[1].commands = [c]    => 2           => bench.benches[1].setup
                // bench.benches[2].commands = [d, e] => 3, 4        => bench.benches[2].setup
                //
                // We also need to take care of that there can be a global setup function
                // `BinaryBenchmark::setup`, which can be overridden by a `Bench::setup`
                match binary_benchmark
                        .benches
                        .iter()
                        .flat_map(|b| b.commands.iter().map(|c| (b.setup, c)))
                        .nth(bench_index)
                        .map(|(setup, _)| setup)
                        .expect("The bench index for setup should be present") {
                    $crate::__internal::InternalBinAssistantKind::Default(setup) => {
                        setup();
                    }
                    $crate::__internal::InternalBinAssistantKind::Iter(setup) => {
                        setup(iter_index);
                    }
                    $crate::__internal::InternalBinAssistantKind::None => {
                        if let Some(setup) = binary_benchmark.setup {
                            setup();
                        } else {
                            // This branch should be unreachable so we do nothing
                        }
                    }
                }
            }

            pub fn __run_bench_teardown(
                group_index: usize, bench_index: usize, iter_index: Option<usize>
            ) {
                let mut group = $crate::BinaryBenchmarkGroup::default();
                $name(&mut group);

                let binary_benchmark = group
                    .binary_benchmarks
                    .iter()
                    .nth(group_index)
                    .expect("The group index for teardown should be present");
                match binary_benchmark
                        .benches
                        .iter()
                        .flat_map(|b| b.commands.iter().map(|c| (b.teardown, c)))
                        .nth(bench_index)
                        .map(|(teardown, _)| teardown)
                        .expect("The bench index for teardown should be present") {
                    $crate::__internal::InternalBinAssistantKind::Default(teardown) => {
                        teardown();
                    }
                    $crate::__internal::InternalBinAssistantKind::Iter(teardown) => {
                        teardown(iter_index);
                    }
                    $crate::__internal::InternalBinAssistantKind::None => {
                        if let Some(teardown) = binary_benchmark.teardown {
                            teardown();
                        } else {
                            // This branch should be unreachable so we do nothing
                        }
                    }
                }
            }

            #[inline(never)]
            pub fn $name($group: &mut $crate::BinaryBenchmarkGroup) {
                $body;
            }
        }
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr; $(;)* )?
        $( teardown = $teardown:expr; $(;)* )?
        benchmarks = |$group:ident| $body:expr
    ) => {
        binary_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = |$group: &mut BinaryBenchmarkGroup| $body
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr, $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks = |$group:ident: &mut BinaryBenchmarkGroup| $body:expr
    ) => {
        binary_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = |$group: &mut BinaryBenchmarkGroup| $body
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr, $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr, $(,)* )?
        $( teardown = $teardown:expr, $(,)* )?
        benchmarks = |$group:ident| $body:expr
    ) => {
        binary_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = |$group: &mut BinaryBenchmarkGroup| $body
        );
    };
    (
        $( $m:tt )*
    ) => {
        compile_error!(
            "You are using an invalid or deprecated syntax of the binary_benchmark_group! macro \
            to set up binary benchmarks. See the documentation of this macro, the README \
            (https://github.com/gungraun/gungraun) or the docs \
            (https://docs.rs/gungraun/latest/gungraun) for further details.\n\n\
            hint: binary_benchmark_group!(name = some_name, benchmarks = [ bench_1, bench_2 ]);"
        );
    };
}

/// Macro used to define a group of library benchmarks
///
/// A small introductory example which shows the basic setup. This macro only accepts benchmarks
/// annotated with `#[library_benchmark]` ([`crate::library_benchmark`]).
///
/// ```rust
/// use gungraun::prelude::*;
///
/// #[library_benchmark]
/// fn bench_something() -> u64 {
///     42
/// }
///
/// library_benchmark_group!(name = my_group, benchmarks = bench_something);
///
/// # fn main() {
/// gungraun::main!(library_benchmark_groups = my_group);
/// # }
/// ```
///
/// To be benchmarked a `library_benchmark_group` has to be added to the `main!` macro by adding its
/// name to the `library_benchmark_groups` argument of the `main!` macro. See there for further
/// details about the [`crate::main`] macro.
///
/// The following top-level arguments are accepted in this order:
///
/// ```rust
/// # use gungraun::prelude::*;
/// # #[library_benchmark]
/// # fn bench_1() {}
/// # #[library_benchmark]
/// # fn bench_2() {}
/// fn group_setup() {}
/// fn group_teardown() {}
/// library_benchmark_group!(
///     name = my_group,
///     config = LibraryBenchmarkConfig::default(),
///     compare_by_id = false,
///     max_parallel = 1,
///     setup = group_setup(),
///     teardown = group_teardown(),
///     benchmarks = [bench_1, bench_2]
/// );
/// # fn main() {
/// # }
/// ```
///
/// * __`name`__ (mandatory): A unique name used to identify the group for the `main!` macro
/// * __`config`__ (optional): A [`crate::LibraryBenchmarkConfig`] which is applied to all
///   benchmarks within the same group.
/// * __`compare_by_id`__ (optional): The default is false. If true, all benches in the benchmark
///   functions specified with the `benchmarks` argument, across any benchmark groups, are compared
///   with each other as long as the ids (the part after the `::` in `#[bench::id(...)]`) match.
/// * __`max_parallel`__ (optional): The default is no limit. If set to a value, `0` means no limit
///   (same as not specifying this option), `1` disables parallel execution for this group, and
///   values `>= 2` limit the maximum number of parallel benchmarks to run within this group. This
///   option does nothing unless parallel execution is enabled with `--parallel` or
///   `GUNGRAUN_PARALLEL`.
/// * __`setup`__ (optional): A setup function or any valid expression which is run before all
///   benchmarks of this group
/// * __`teardown`__ (optional): A teardown function or any valid expression which is run after all
///   benchmarks of this group
/// * __`benchmarks`__ (mandatory): An array of benchmark functions annotated with
///   `#[library_benchmark]`. For convenience, the brackets can be omitted if there is only a single
///   benchmark function (`benchmarks = bench_1` instead of `benchmarks = [bench_1]`).
#[macro_export]
macro_rules! library_benchmark_group {
    () => {
        compile_error!("A library_benchmark_group! needs a name and at least 1 benchmark function \
            annotated with #[library_benchmark]. See the documentation of this macro for more \
            details.\n\n\
            hint: library_benchmark_group!(name = some_ident, \
                benchmarks = [ bench_1, bench_2 ]);");
    };
    (
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        benchmarks = $( $some:tt )*
    ) => {
        compile_error!("A library_benchmark_group! needs a name\
            \n\nhint: library_benchmark_group!(name = some_ident; benchmarks = ...);");
    };
    (
        name = $name:ident;
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        benchmarks =
    ) => {
        compile_error!(
            "A library_benchmark_group! needs at least 1 benchmark function \
            annotated with #[library_benchmark]\n\n\
            hint: library_benchmark_group!(name = some_ident; \
                benchmarks = some_library_benchmark);");
    };
    (
        name = $name:ident; $(;)*
        $( config = $config:expr ; $(;)* )?
        $( compare_by_id = $compare:literal ; $(;)* )?
        $( max_parallel = $parallel:expr ; $(;)* )?
        $( setup = $setup:expr ; $(;)* )?
        $( teardown = $teardown:expr ; $(;)* )?
        benchmarks = $( $function:ident ),+ $(,)*
    ) => {
        pub mod $name {
            use super::*;

            pub const __BENCHES: &[&(
                &'static str,
                fn() -> Option<$crate::__internal::InternalLibraryBenchmarkConfig>,
                &[$crate::__internal::InternalMacroLibBench]
            )]= &[
                $(
                    &(
                        stringify!($function),
                        super::$function::__get_config,
                        super::$function::__BENCHES
                    )
                ),+
            ];

            #[inline(never)]
            pub fn __get_config() -> Option<$crate::__internal::InternalLibraryBenchmarkConfig> {
                let mut config: Option<$crate::__internal::InternalLibraryBenchmarkConfig> = None;
                $(
                    config = Some($config.into());
                )?
                config
            }

            #[inline(never)]
            pub fn __compare_by_id() -> Option<bool> {
                let mut comp = None;
                $(
                    comp = Some($compare);
                )?
                comp
            }

            #[inline(never)]
            pub fn __max_parallel() -> Option<usize> {
                let mut __par = None;
                $(
                    let expr: usize = $parallel;
                    __par = Some(expr);
                )?
                __par
            }

            #[inline(never)]
            pub fn __run_setup(__run: bool) -> bool {
                let mut __has_setup = false;
                $(
                    __has_setup = true;
                    if __run {
                        $setup;
                    }
                )?
                __has_setup
            }

            #[inline(never)]
            pub fn __run_teardown(__run: bool) -> bool {
                let mut __has_teardown = false;
                $(
                    __has_teardown = true;
                    if __run {
                        $teardown;
                    }
                )?
                __has_teardown
            }

            #[inline(never)]
            pub fn __run(
                mode: $crate::__internal::InternalBenchRunMode,
                group_index: usize,
                bench_index: usize,
                iter_index: Option<usize>
            ) {
                match __BENCHES[group_index].2[bench_index].func {
                    $crate::__internal::InternalLibFunctionKind::Iter(func) => {
                        (func)(mode, iter_index);
                    }
                    $crate::__internal::InternalLibFunctionKind::Default(func) => {
                        (func)(mode);
                    }
                }
            }
        }
    };
    (
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        benchmarks = $( $some:tt )*
    ) => {
        compile_error!("A library_benchmark_group! needs a name\n\n\
            hint: library_benchmark_group!(name = some_ident, benchmarks = ...);");
    };
    (
        name = $name:ident,
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        benchmarks =
    ) => {
        compile_error!(
            "A library_benchmark_group! needs at least 1 benchmark function \
            annotated with #[library_benchmark]\n\n\
            hint: library_benchmark_group!(name = some_ident, \
                benchmarks = some_library_benchmark);");
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        benchmarks = $function:ident
    ) => {
        library_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = $function
        );
    };
    (
        name = $name:ident, $(,)*
        $( config = $config:expr , $(,)* )?
        $( compare_by_id = $compare:literal , $(,)* )?
        $( max_parallel = $parallel:expr , $(,)* )?
        $( setup = $setup:expr , $(,)* )?
        $( teardown = $teardown:expr , $(,)* )?
        benchmarks = [ $( $function:ident ),+ $(,)* ]
    ) => {
        library_benchmark_group!(
            name = $name;
            $( config = $config; )?
            $( compare_by_id = $compare; )?
            $( max_parallel = $parallel; )?
            $( setup = $setup; )?
            $( teardown = $teardown; )?
            benchmarks = $( $function ),+
        );
    };
    (
        $( $m:tt )*
    ) => {
        compile_error!(
            "You are using an invalid or deprecated syntax of the library_benchmark_group! macro \
            to set up library benchmarks. See the documentation of this macro, the README \
            (https://github.com/gungraun/gungraun) or the docs \
            (https://docs.rs/gungraun/latest/gungraun) for further details.\n\n\
            hint: library_benchmark_group!(name = some_name, benchmarks = [ bench_1, bench_2 ]);"
        );
    };
}
