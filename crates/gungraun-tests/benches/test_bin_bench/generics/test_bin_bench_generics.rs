use std::ffi::OsString;

use gungraun::prelude::*;

pub const ECHO: &str = env!("CARGO_BIN_EXE_echo");

#[binary_benchmark]
#[bench::foo("foo")]
#[bench::args_foo(args = ("foo"))]
#[benches::args_foo_bar(args = ["foo", "bar"])]
fn single_param<T: Into<OsString>>(arg_t: T) -> Command {
    Command::new(ECHO).arg(arg_t).build()
}

#[binary_benchmark]
#[bench::foo("foo")]
#[bench::args_foo(args = ("foo"))]
#[benches::foo_bar("foo", "bar")]
#[benches::args_foo_bar(args = ["foo", "bar"])]
fn single_param_in_where_clause<T>(arg_t: T) -> Command
where
    T: Into<OsString>,
{
    Command::new(ECHO).arg(arg_t).build()
}

#[binary_benchmark]
#[bench::just_consts(consts = (321))]
#[bench::just_consts_expression(consts = ({1 + 20}))]
#[bench::empty_args(args = (), consts = (321))]
#[benches::multiple_consts(consts = [321, 654])]
#[benches::multiple_consts_expression(consts = [{1 + 20}, {2 + 20}])]
#[benches::multiple_consts_empty_args(args = [], consts = [321, 654])]
fn single_consts_parameters_without_args<const C: usize>() -> Command {
    Command::new(ECHO).arg(C.to_string()).build()
}

#[binary_benchmark]
#[bench::just_consts(consts = (321, 654))]
#[bench::empty_args(args = (), consts = (321, 654))]
#[benches::multiple_consts(consts = [(321, 654), (654, 987)])]
#[benches::multiple_consts_empty_args(args = [], consts = [(321, 654), (654, 987)])]
fn multiple_consts_parameter_without_args<const C: usize, const D: u64>() -> Command {
    Command::new(ECHO)
        .arg(C.to_string())
        .arg(D.to_string())
        .build()
}

#[binary_benchmark]
#[bench::args_and_consts(args = (123), consts = (321, 654))]
#[benches::args_and_consts_same_count(args = [123, 456], consts = [(321, 654), (654, 987)])]
#[benches::more_args_than_consts(args = [123, 456], consts = [(321, 654)])]
#[benches::more_consts_than_args(args = [123], consts = [(321, 654), (645, 987)])]
fn multiple_consts_and_type_param<const C: usize, T, const D: u64>(arg_t: T) -> Command
where
    T: std::fmt::Display,
{
    Command::new(ECHO)
        .arg(C.to_string())
        .arg(D.to_string())
        .arg(arg_t.to_string())
        .build()
}

binary_benchmark_group!(
    name = type_parameter_group,
    benchmarks = [single_param, single_param_in_where_clause]
);

binary_benchmark_group!(
    name = consts_group,
    benchmarks = [
        single_consts_parameters_without_args,
        multiple_consts_parameter_without_args,
        multiple_consts_and_type_param
    ]
);
main!(binary_benchmark_groups = [type_parameter_group, consts_group]);
