//! The library of gungraun-macros

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(attr(warn(unused))))]
#![doc(test(attr(allow(unused_extern_crates))))]

mod bin_bench;
mod common;
pub(crate) mod defaults;
mod derive_macros;
mod lib_bench;

use proc_macro::TokenStream;
use proc_macro_error3::proc_macro_error;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    workspace_root: String,
}

impl CargoMetadata {
    fn try_new() -> Option<Self> {
        std::process::Command::new(option_env!("CARGO").unwrap_or("cargo"))
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .ok()
            .and_then(|output| serde_json::de::from_slice(&output.stdout).ok())
    }
}

/// The `#[library_benchmark]` attribute lets you define a benchmark function which you can later
/// use in the `library_benchmark_groups!` macro.
///
/// This attribute accepts the following parameters:
/// * `config`: Accepts a `LibraryBenchmarkConfig`
/// * `setup`: A global setup function which is applied to all following [`#[bench]`][bench] and
///   [`#[benches]`][benches] attributes if not overwritten by a `setup` parameter of these
///   attributes.
/// * `teardown`: Similar to `setup` but takes a global `teardown` function.
///
/// A short introductory example on the usage including the `setup` parameter:
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// fn my_setup(value: u64) -> String {
///     format!("{value}")
/// }
///
/// fn my_other_setup(value: u64) -> String {
///     format!("{}", value + 10)
/// }
///
/// #[library_benchmark(setup = my_setup)]
/// #[bench::first(21)]
/// #[benches::multiple(42, 84)]
/// #[bench::last(args = (102), setup = my_other_setup)]
/// fn my_bench(value: String) {
///     println!("{value}");
/// }
/// # fn main() {}
/// ```
///
/// The `#[library_benchmark]` attribute can be applied in two ways.
///
/// 1. Using the `#[library_benchmark]` attribute as a standalone without [`#[bench]`][bench] or
///    [`#[benches]`][benches] is fine for simple function calls without parameters.
/// 2. We mostly need to benchmark cases which would need to be set up for example with a vector,
///    but everything we set up within the benchmark function itself would be attributed to the
///    event counts. The second form of this attribute macro uses the [`#[bench]`][bench] and
///    [`#[benches]`][benches] attributes to set up benchmarks with different cases. The main
///    advantage is that the setup costs and event counts aren't attributed to the benchmark (and
///    opposed to the old API we don't have to deal with Callgrind arguments, toggles,
///    inline(never), ...)
///
/// # The `#[bench]` attribute
///
/// The basic structure is `#[bench::some_id(/* parameters */)]`. The part after the `::` must be an
/// id unique within the same `#[library_benchmark]`. This attribute accepts the following
/// parameters:
///
/// * __`args`__: A tuple with a list of arguments which are passed to the benchmark function. The
///   parentheses also need to be present if there is only a single argument (`#[bench::my_id(args =
///   (10))]`).
/// * __`config`__: Accepts a `LibraryBenchmarkConfig`
/// * __`setup`__: A function which takes the arguments specified in the `args` parameter and passes
///   its return value to the benchmark function.
/// * __`teardown`__: A function which takes the return value of the benchmark function.
/// * __`consts`__: A tuple with const expressions for const generic parameters of the benchmark
///   function. The number and order must match the const generics in the function signature.
///   Parentheses are required: `consts = (321)` for a single const generic, `consts = (321, 654)`
///   for multiple. Const expressions like `consts = ({ 1 + 20 })` are also supported.
///
/// If no other parameters besides `args` are present you can simply pass the arguments as a list of
/// values. Instead of `#[bench::my_id(args = (10, 20))]`, you could also use the shorter
/// `#[bench::my_id(10, 20)]`.
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// use std::hint::black_box;
///
/// // Assume this is a function in your library which you want to benchmark
/// fn some_func(value: u64) -> u64 {
///     42
/// }
///
/// #[library_benchmark]
/// #[bench::some_id(42)]
/// fn bench_some_func(value: u64) -> u64 {
///     black_box(some_func(black_box(value)))
/// }
/// # fn main() {}
/// ```
///
/// # The `#[benches]` attribute
///
/// The `#[benches]` attribute lets you define multiple benchmarks in one go. This attribute accepts
/// the same parameters as the [`#[bench]`][bench] attribute: `args`, `config`, `setup`, `teardown`,
/// `consts` and additionally the `iter` and `file` parameter. In contrast to the `args` parameter
/// in [`#[bench]`][bench], `args` takes an array of arguments. Similarly, `consts` takes an array
/// of const expressions. The id (`#[benches::id(/* parameters */)]`) is getting suffixed with the
/// index of the current element of the `args` array.
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// use std::hint::black_box;
///
/// fn setup_worst_case_array(start: i32) -> Vec<i32> {
///     if start.is_negative() {
///         (start..0).rev().collect()
///     } else {
///         (0..start).rev().collect()
///     }
/// }
///
/// #[library_benchmark]
/// #[benches::multiple(vec![1], vec![5])]
/// #[benches::with_setup(args = [1, 5], setup = setup_worst_case_array)]
/// fn bench_bubble_sort_with_benches_attribute(input: Vec<i32>) -> Vec<i32> {
///     black_box(my_lib::bubble_sort(black_box(input)))
/// }
/// # fn main() {}
/// ```
///
/// Usually the `arguments` are passed directly to the benchmarking function as it can be seen in
/// the `#[benches::multiple(...)]` case. In `#[benches::with_setup(...)]`, the arguments are passed
/// to the `setup` function and the return value of the `setup` function is passed as argument to
/// the benchmark function. The above `#[library_benchmark]` is pretty much the same as
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// # fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] }
/// # fn setup_worst_case_array(_: i32) -> Vec<i32> { vec![] }
/// use std::hint::black_box;
///
/// #[library_benchmark]
/// #[bench::multiple_0(vec![1])]
/// #[bench::multiple_1(vec![5])]
/// #[bench::with_setup_0(setup_worst_case_array(1))]
/// #[bench::with_setup_1(setup_worst_case_array(5))]
/// fn bench_bubble_sort_with_benches_attribute(input: Vec<i32>) -> Vec<i32> {
///     black_box(bubble_sort(black_box(input)))
/// }
/// # fn main() {}
/// ```
///
/// but a lot more concise especially if a lot of values are passed to the same `setup` function.
///
/// The `iter` parameter accepts an iterator expression as argument (anything that implements
/// [`std::iter::IntoIterator`]) which creates a benchmark case per iterator element. For example
/// the following will create three benchmark cases `0`, `1` and `2`  from the `0..3` range:
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// # mod my_lib { pub fn u64_to_string(_: u64) -> String { "0".to_owned() } }
/// use std::hint::black_box;
///
/// #[library_benchmark]
/// #[benches::from_iter(iter = 0..3)]
/// fn some_bench(num: u64) -> String {
///     black_box(my_lib::u64_to_string(black_box(num)))
/// }
/// # fn main() {}
/// ```
///
/// The `file` parameter reads the specified file line by line creating a benchmark from each line.
/// The line is passed to the benchmark function as `String` or if the `setup` parameter is also
/// present to the `setup` function. A small example assuming you have a file `benches/inputs`
/// (relative paths are interpreted to the workspace root) with the following content
///
/// ```text
/// 1
/// 11
/// 111
/// ```
///
/// then
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// # mod my_lib { pub fn string_to_u64(_line: String) -> Result<u64, String> { Ok(0) } }
/// use std::hint::black_box;
///
/// #[library_benchmark]
/// #[benches::by_file(file = "gungraun-macros/fixtures/inputs")]
/// fn some_bench(line: String) -> Result<u64, String> {
///     black_box(my_lib::string_to_u64(black_box(line)))
/// }
/// # fn main() {}
/// ```
///
/// The above is roughly equivalent to the following but with the `args` parameter
///
/// ```rust,ignore
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// # mod my_lib { pub fn string_to_u64(_line: String) -> Result<u64, String> { Ok(0) } }
/// use std::hint::black_box;
///
/// #[library_benchmark]
/// #[benches::by_file(args = [1.to_string(), 11.to_string(), 111.to_string()])]
/// fn some_bench(line: String) -> Result<u64, String> {
///     black_box(my_lib::string_to_u64(black_box(line)))
/// }
/// # fn main() {}
/// ```
///
/// # More Examples
///
/// The `#[library_benchmark]` attribute as a standalone
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// use std::hint::black_box;
///
/// fn some_func() -> u64 {
///     42
/// }
///
/// #[library_benchmark]
/// // If possible, it's best to return something from a benchmark function
/// fn bench_my_library_function() -> u64 {
///     // The `black_box` is needed to tell the compiler to not optimize what's inside the
///     // black_box or else the benchmarks might return inaccurate results.
///     black_box(some_func())
/// }
/// # fn main() {
/// # }
/// ```
///
/// In the following example we pass a single argument with `Vec<i32>` type to the
/// benchmark. Wrap both input and output values in `black_box` to prevent the
/// compiler from optimizing away computations.
///
/// ```rust
/// # use gungraun_macros::library_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// use std::hint::black_box;
///
/// // Our function we want to test
/// fn some_func_with_array(array: Vec<i32>) -> Vec<i32> {
///     // do something with the array and return a new array
///     # array
/// }
///
/// // This function is used to create a worst case array for our `some_func_with_array`
/// fn setup_worst_case_array(start: i32) -> Vec<i32> {
///     if start.is_negative() {
///         (start..0).rev().collect()
///     } else {
///         (0..start).rev().collect()
///     }
/// }
///
/// // This benchmark is setting up multiple benchmark cases with the advantage that the setup
/// // costs for creating a vector (even if it is empty) aren't attributed to the benchmark.
/// #[library_benchmark]
/// #[bench::empty(vec![])]
/// #[bench::worst_case_6(vec![6, 5, 4, 3, 2, 1])]
/// // Function calls are fine too
/// #[bench::worst_case_4000(setup_worst_case_array(4000))]
/// // The argument of the benchmark function defines the type of the argument from the `bench`
/// // cases.
/// fn bench_some_func_with_array(array: Vec<i32>) -> Vec<i32> {
///     // Wrap both input and output in `black_box`.
///     black_box(some_func_with_array(black_box(array)))
/// }
///
/// // The following benchmark uses the `#[benches]` attribute to setup multiple benchmark cases
/// // in one go
/// #[library_benchmark]
/// #[benches::multiple(vec![1], vec![5])]
/// // Reroute the `args` to a `setup` function and use the setup function's return value as
/// // input for the benchmarking function
/// #[benches::with_setup(args = [1, 5], setup = setup_worst_case_array)]
/// fn bench_using_the_benches_attribute(array: Vec<i32>) -> Vec<i32> {
///     black_box(some_func_with_array(black_box(array)))
/// }
/// # fn main() { }
/// ```
///
/// [bench]: #the-bench-attribute
/// [benches]: #the-benches-attribute
#[proc_macro_attribute]
#[proc_macro_error]
pub fn library_benchmark(args: TokenStream, input: TokenStream) -> TokenStream {
    match lib_bench::render(args.into(), input.into()) {
        Ok(stream) => stream.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Used to annotate functions building the to be benchmarked `gungraun::Command`
///
/// This macro works almost the same way as the [`macro@crate::library_benchmark`] attribute. Please
/// see there for the basic usage.
///
/// # Differences to the `#[library_benchmark]` attribute
///
/// Any `config` parameter takes a `BinaryBenchmarkConfig` instead of a `LibraryBenchmarkConfig`.
/// All functions annotated with the `#[binary_benchmark]` attribute need to return an
/// `gungraun::Command`. Also, the annotated function itself is not benchmarked. Instead, this
/// function serves the purpose of a builder for the `Command` which is getting benchmarked.
/// So, any code within this function is evaluated only once when all `Commands` in this benchmark
/// file are collected and built. You can put any code in the function which is necessary to build
/// the `Command` without attributing any event counts to the benchmark results which is why the
/// `setup` and `teardown` parameters work differently in binary benchmarks.
///
/// The `setup` and `teardown` parameters of `#[binary_benchmark]`, `#[bench]` and of `#[benches]`
/// take an expression instead of a function pointer. The expression of the `setup` (`teardown`)
/// parameter is evaluated and executed not until before (after) the `Command` is executed (not
/// __built__). There's a special case if `setup` or `teardown` are a function pointer like in
/// library benchmarks. In this case the `args` from `#[bench]` or `#[benches]` are passed to the
/// function AND `setup` or `teardown` respectively.
///
/// For example (Suppose your crate's binary is named `my-foo`)
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// # use gungraun_macros::binary_benchmark;
/// # #[macro_export]
/// # macro_rules! perf_log { ($($arg:tt)*) => {}; }
/// # #[macro_export]
/// # macro_rules! perf_enable { () => { std::ptr::null_mut::<()>() }; }
/// # #[macro_export]
/// # macro_rules! perf_disable { ($control_token:expr) => {}; }
/// # pub mod gungraun {
/// # include!("__doc_stubs.rs");
/// # }
/// # use gungraun::BinaryBenchmarkConfig;
/// use std::path::PathBuf;
///
/// use gungraun::Sandbox;
/// use gungraun::prelude::*;
///
/// // In binary benchmarks there's no need to return a value from the setup function
/// # #[allow(unused)]
/// fn simple_setup() {
///     println!("Put code in here which will be run before the actual command");
/// }
///
/// // It is good style to write any setup function idempotent, so it doesn't depend on the
/// // `teardown` to have run. The `teardown` function isn't executed if the benchmark
/// // command fails to run successfully.
/// # #[allow(unused)]
/// fn create_file(path: &str) {
///     // You can for example create a file here which should be available for the `Command`
///     std::fs::File::create(path).unwrap();
/// }
///
/// # #[allow(unused)]
/// fn teardown() {
///     // Let's clean up this temporary file after we have used it
///     std::fs::remove_file("file_from_setup_function.txt").unwrap();
/// }
///
/// #[binary_benchmark]
/// #[bench::just_a_fixture("benches/fixture.json")]
/// // First big difference to library benchmarks! `my_setup` is not evaluated right away and the
/// // return value of `simple_setup` is not used as input for the `bench_foo` function. Instead,
/// // `simple_setup()` is executed before the execution of the `Command`.
/// #[bench::with_other_fixture_and_setup(
///     args = ("benches/other_fixture.txt"),
///     setup = simple_setup()
/// )]
/// // Here, setup is a function pointer, what tells us to route `args` to `setup` AND `bench_foo`
/// #[bench::file_from_setup(
///     args = ("file_from_setup_function.txt"),
///     setup = create_file,
///     teardown = teardown()
/// )]
/// // Just an small example for the basic usage of the `#[benches]` attribute
/// #[benches::multiple("benches/fix_1.txt", "benches/fix_2.txt")]
/// // We're using a `BinaryBenchmarkConfig` in binary benchmarks to configure these benchmarks to
/// // run in a sandbox.
/// #[benches::multiple_with_config(
///     args = ["benches/fix_1.txt", "benches/fix_2.txt"],
///     config = BinaryBenchmarkConfig::default()
///         .sandbox(Sandbox::new(true)
///             .fixtures(["benches/fix_1.txt", "benches/fix_2.txt"])
///         )
/// )]
/// // All functions annotated with `#[binary_benchmark]` need to return a `gungraun::Command`
/// fn bench_foo(path: &str) -> gungraun::Command {
///     let path = PathBuf::from(path);
///     // We can put any code in here which is needed to configure the `Command`.
///     let stdout = if path.extension().unwrap() == "txt" {
///         gungraun::Stdio::Inherit
///     } else {
///         gungraun::Stdio::File(path.with_extension("out"))
///     };
///     // Configure the command depending on the arguments passed to this function and the code
///     // above
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .stdout(stdout)
///         .arg(path)
///         .build()
/// }
/// # fn main() {
/// # // To avoid the unused warning
/// # let _ = (bench_foo::__BENCHES[0].func);
/// # }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn binary_benchmark(args: TokenStream, input: TokenStream) -> TokenStream {
    match bin_bench::render(args.into(), input.into()) {
        Ok(stream) => stream.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// For internal use only.
///
/// The old `macro_rules! impl_traits` was easy to overlook in the source code files and this derive
/// macro is just a much nicer way to do the same.
///
/// We use this derive macro to spare us the manual implementation of
///
/// * `From<Outer> for Inner`
/// * `From<&Outer> for Inner` (which clones the value)
/// * `From<&mut Outer> for Inner` (which also just clones the value)
///
/// for our builder tuple structs which wrap the inner type from the gungraun-runner api. So,
/// our builders don't need a build method, which is just cool.
#[proc_macro_derive(IntoInner)]
#[proc_macro_error]
pub fn into_inner(item: TokenStream) -> TokenStream {
    match derive_macros::render_into_inner(item.into()) {
        Ok(stream) => stream.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
