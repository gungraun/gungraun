use std::hint::black_box;

/// See issue https://github.com/gungraun/gungraun/issues/198
/// Generic bench arguments cause compilation failure
///
/// After the fix the benchmark should now compile
use gungraun::prelude::*;

#[derive(Debug)]
struct A;

fn input_a() -> A {
    A
}

#[derive(Debug)]
struct B;

fn input_b() -> B {
    B
}

fn print_debug<T>(input: T) -> usize
where
    T: std::fmt::Debug,
{
    let output = format!("BENCH: {input:?}");
    println!("{output}");
    output.len()
}

fn setup_with_type_parameter<T>(arg_t: T) -> usize
where
    T: std::fmt::Debug,
{
    let output = format!("SETUP: arg: {arg_t:?}");
    println!("{output}");
    output.len()
}

fn teardown_with_type_parameter<T>(arg_t: T)
where
    T: std::fmt::Debug,
{
    println!("TEARDOWN: arg: {arg_t:?}")
}

fn setup_with_const_parameter<const C: usize>() -> usize {
    let output = format!("SETUP: const: {C}");
    println!("{output}");
    output.len()
}

fn teardown_with_const_parameter<const C: usize, T>(arg_t: T)
where
    T: std::fmt::Debug,
{
    println!("TEARDOWN: const: {C}, arg: {arg_t:?}")
}

#[library_benchmark]
#[bench::a(input_a())]
#[bench::b(input_b())]
fn single_param<I: std::fmt::Debug>(input: I) -> usize {
    black_box(print_debug(black_box(input)))
}

#[library_benchmark]
#[bench::a(input_a())]
#[bench::b(input_b())]
fn single_param_in_where_clause<I>(input: I) -> usize
where
    I: std::fmt::Debug,
{
    black_box(print_debug(black_box(input)))
}

#[library_benchmark]
#[bench::just_consts(consts = (321))]
#[bench::empty_args(args = (), consts = (321))]
#[benches::multiple_consts(consts = [321, 654])]
#[benches::multiple_consts_empty_args(args = [], consts = [321, 654])]
fn single_consts_parameter_without_args<const C: usize>() -> usize {
    black_box(print_debug(black_box(C)))
}

#[library_benchmark]
#[bench::same_amount_of_arguments_single_case(args = (123), consts = (321))]
#[bench::same_amount_of_arguments_single_case_const_expression(args = (123), consts = ({ 1 + 20 }))]
// generates 2 benchmarks with last args argument repeated
#[benches::single_arg_multiple_consts(args = [123], consts = [321, 654])]
#[benches::single_arg_multiple_consts_expressions(args = [123], consts = [{ 1 + 20 }, { 2 + 20 }])]
// generates 2 benchmarks with last consts argument repeated
#[benches::multiple_args_single_consts(args = [123, 456], consts = [321])]
#[benches::multiple_args_single_consts_expression(args = [123, 456], consts = [{ 1 + 20 }])]
#[benches::multiple_arguments_same_amount(args = [123, 456], consts = [321, 654])]
#[benches::multiple_arguments_same_amount_const_expression(
    args = [123, 456],
    consts = [{ 1 + 20 }, { 2 + 20 }]
)]
// generates 3 benchmarks with last args argument repeated
#[benches::multiple_arguments_more_consts_than_args(args = [123, 456], consts = [321, 654, 987])]
#[benches::multiple_arguments_more_consts_expression_than_args(
    args = [123, 456],
    consts = [{1 + 20 }, { 2 + 20 }, { 3 + 20 }]
)]
// generates 3 benchmarks with last consts argument repeated
#[benches::multiple_arguments_more_args_than_consts(args = [123, 456, 789], consts = [321, 654])]
#[benches::multiple_arguments_more_args_than_consts_expression(
    args = [123, 456, 789],
    consts = [{ 1 + 20 }, { 2 + 20 }]
)]
fn single_consts_parameter_with_args<const C: usize>(arg: i32) -> usize {
    black_box(print_debug(black_box(C + arg as usize)))
}

#[library_benchmark]
#[bench::just_consts(consts = (321, 654))]
#[bench::empty_args(args = (), consts = (321, 654))]
#[benches::single_consts(consts = [(321, 654)])]
#[benches::multiple_consts(consts = [(321, 654), (654, 987)])]
fn multiple_consts_parameters<const C: usize, const D: i32>() -> usize {
    black_box(print_debug(C + D as usize))
}

#[library_benchmark]
#[bench::single(args = (123), consts = (321))]
#[benches::multiple_same_count(args = [123, 456], consts = [321, 654])]
#[benches::multiple_more_consts_than_args(args = [123], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(args = [123, 456], consts = [321])]
fn const_then_type_parameter<const C: usize, T>(arg: T) -> usize
where
    T: std::fmt::Debug,
{
    black_box(print_debug(black_box(C)) + print_debug(black_box(arg)))
}

#[library_benchmark]
#[bench::single(args = (123), consts = (321))]
#[benches::multiple_same_count(args = [123, 456], consts = [321, 654])]
#[benches::multiple_more_consts_than_args(args = [123], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(args = [123, 456], consts = [321])]
fn type_parameter_then_const<T, const C: usize>(arg: T) -> usize
where
    T: std::fmt::Debug,
{
    black_box(print_debug(black_box(C)) + print_debug(black_box(arg)))
}

#[library_benchmark]
#[bench::single(args = (123u64, 456i32), consts = (321))]
#[benches::multiple_same_count(args = [(123u64, 456i32), (456u64, 789i32)], consts = [321, 654])]
#[benches::multiple_more_consts_than_args(args = [(123u64, 456i32)], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(
    args = [(123u64, 456i32), (456u64, 789i32)],
    consts = [321]
)]
fn mixed_type_parameters_and_consts<T, const C: usize, U>(arg_t: T, arg_u: U) -> usize
where
    T: std::fmt::Debug,
    U: std::fmt::Debug,
{
    black_box(
        print_debug(black_box(C)) + print_debug(black_box(arg_t)) + print_debug(black_box(arg_u)),
    )
}

#[library_benchmark]
#[bench::single(args = (&123u64, &456i32), consts = (321))]
#[benches::multiple_same_count(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321, 654]
)]
#[benches::multiple_more_consts_than_args(args = [(&123u64, &456i32)], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321]
)]
fn lifetimes_then_const<'a, 'b, const C: usize>(
    arg_t: &'a u64,
    arg_u: &'b i32,
) -> (&'a u64, &'b i32) {
    print_debug(C);
    print_debug(arg_t);
    print_debug(arg_u);

    black_box((black_box(arg_t), black_box(arg_u)))
}

#[library_benchmark]
#[bench::single(args = (&123u64, &456i32), consts = (321))]
#[benches::multiple_same_count(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321, 654]
)]
#[benches::multiple_more_consts_than_args(args = [(&123u64, &456i32)], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321]
)]
fn const_then_lifetimes<const C: usize, 'a, 'b>(
    arg_t: &'a u64,
    arg_u: &'b i32,
) -> (&'a u64, &'b i32) {
    print_debug(C);
    print_debug(arg_t);
    print_debug(arg_u);

    black_box((black_box(arg_t), black_box(arg_u)))
}

#[library_benchmark]
#[bench::single(args = (&123u64, &456i32), consts = (321))]
#[benches::multiple_same_count(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321, 654]
)]
#[benches::multiple_more_consts_than_args(args = [(&123u64, &456i32)], consts = [321, 654])]
#[benches::multiple_more_args_than_consts(
    args = [(&123u64, &456i32), (&456u64, &789i32)],
    consts = [321]
)]
fn mixed_const_and_lifetimes<'b, const C: usize, 'a>(
    arg_t: &'a u64,
    arg_u: &'b i32,
) -> (&'a u64, &'b i32) {
    print_debug(C);
    print_debug(arg_t);
    print_debug(arg_u);

    black_box((black_box(arg_t), black_box(arg_u)))
}

#[library_benchmark]
#[bench::single(args = (&123u64, &456i32), consts = (321))]
#[benches::multiple_same_count(
    args = [(&123u64, &456i32), (&123u64, &456i32)],
    consts = [321, 654]
)]
#[benches::multiple_more_consts_than_args(args = [(&123u64, &456i32)], consts = [312, 654])]
#[benches::multiple_more_args_than_consts(
    args = [(&123u64, &456i32), (&123u64, &456i32)],
    consts = [321]
)]
fn mixed_type_parameters_lifetime_and_consts<T, 'a, const C: usize, U, 'b>(
    arg_t: &'a T,
    arg_u: &'b U,
) -> (&'a T, &'b U)
where
    T: std::fmt::Debug,
    U: std::fmt::Debug,
{
    print_debug(C);
    print_debug(arg_t);
    print_debug(arg_u);

    black_box((black_box(arg_t), black_box(arg_u)))
}

#[library_benchmark]
#[bench::plain(args = ("foo"), setup = setup_with_type_parameter)]
#[benches::multiple_plain(args = ["foo"], setup = setup_with_type_parameter)]
#[bench::with_explicit_type_parameter(
    args = ("bar"),
    setup = setup_with_type_parameter::<&str>
)]
#[benches::multiple_with_explicit_type_parameter(
    args = ["bar"],
    setup = setup_with_type_parameter::<&str>
)]
#[bench::simple_consts(args = (), setup = setup_with_const_parameter::<321>)]
#[benches::multiple_simple_consts(args = [(), ()], setup = setup_with_const_parameter::<321>)]
#[bench::with_const_expression(args = (), setup = setup_with_const_parameter::<{ 1 + 20 }>)]
#[benches::multiple_with_const_expression(
    args = [(), ()],
    setup = setup_with_const_parameter::<{ 1 + 20 }>
)]
fn with_setup_when_type_and_const_parameter(arg: usize) -> usize {
    black_box(print_debug(black_box(arg)))
}

#[library_benchmark]
#[bench::plain(args = (42), teardown = teardown_with_type_parameter)]
#[benches::multiple_plain(args = [123, 456], teardown = teardown_with_type_parameter)]
#[bench::with_explicit_type_parameter(
    args = (42),
    teardown = teardown_with_type_parameter::<usize>
)]
#[benches::multiple_with_explicit_type_parameter(
    args = [123, 456],
    teardown = teardown_with_type_parameter::<usize>
)]
#[bench::simple_consts(args = (42), teardown = teardown_with_const_parameter::<321, usize>)]
#[benches::multiple_simple_consts(
    args = [123, 456],
    teardown = teardown_with_const_parameter::<321, usize>
)]
#[bench::with_const_expression(
    args = (42),
    teardown = teardown_with_const_parameter::<{ 1 + 20 }, usize>
)]
#[benches::multiple_with_const_expression(
    args = [123, 456],
    teardown = teardown_with_const_parameter::<{ 1 + 20 }, usize>
)]
fn with_teardown_when_type_and_const_parameter(arg: u64) -> usize {
    black_box(black_box(arg) as usize)
}

library_benchmark_group!(
    name = type_parameter_group,
    benchmarks = [single_param, single_param_in_where_clause]
);
library_benchmark_group!(
    name = const_group,
    benchmarks = [
        single_consts_parameter_without_args,
        single_consts_parameter_with_args,
        multiple_consts_parameters,
        const_then_type_parameter,
        type_parameter_then_const,
        mixed_type_parameters_and_consts,
        lifetimes_then_const,
        const_then_lifetimes,
        mixed_const_and_lifetimes,
        mixed_type_parameters_lifetime_and_consts
    ]
);

library_benchmark_group!(
    name = generic_setup_and_teardown_group,
    benchmarks = [
        with_setup_when_type_and_const_parameter,
        with_teardown_when_type_and_const_parameter
    ]
);

main!(
    library_benchmark_groups = [
        type_parameter_group,
        const_group,
        generic_setup_and_teardown_group
    ]
);
