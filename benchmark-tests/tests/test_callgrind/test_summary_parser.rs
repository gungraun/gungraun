use gungraun_runner::api::{EventKind, Tool};
use gungraun_runner::runner::callgrind::model::Metrics;
use gungraun_runner::runner::callgrind::parser::CallgrindParser;
use gungraun_runner::runner::callgrind::summary_parser::SummaryParser;
use gungraun_runner::runner::tool::path::ToolOutputPathKind;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::util::common::{Fixtures, assert_parse_error};

// Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw
#[rstest]
#[case::no_records("no_records.with_summary_and_totals", [0, 0, 0, 0, 0, 0, 0, 0, 0])]
#[case::with_records("no_entry_point", [325_259, 78145, 35789, 1595, 2119, 850, 1558, 1485, 799])]
#[case::summary_and_totals("summary_and_totals", [11, 12, 13, 14, 15, 16, 17, 18, 19])]
#[case::no_summary_but_totals("no_summary_but_totals", [11, 12, 13, 14, 15, 16, 17, 18, 19])]
#[case::summary_no_totals("summary_no_totals", [1, 2, 3, 4, 5, 6, 7, 8, 9])]
fn test_summary_parser(#[case] fixture: &str, #[case] costs: [u64; 9]) {
    use gungraun_runner::api::Tool;
    use gungraun_runner::runner::tool::path::ToolOutputPathKind;

    let expected_costs = Metrics::with_metric_kinds([
        (EventKind::Ir, costs[0]),
        (EventKind::Dr, costs[1]),
        (EventKind::Dw, costs[2]),
        (EventKind::I1mr, costs[3]),
        (EventKind::D1mr, costs[4]),
        (EventKind::D1mw, costs[5]),
        (EventKind::ILmr, costs[6]),
        (EventKind::DLmr, costs[7]),
        (EventKind::DLmw, costs[8]),
    ]);

    let callgrind_output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        fixture,
    );

    let parser = SummaryParser::new(&callgrind_output);
    let actual_costs = parser.parse(&callgrind_output).unwrap();

    assert_eq!(actual_costs.len(), 1);
    assert_eq!(actual_costs[0].2, expected_costs);
}

#[test]
fn test_summary_parser_when_not_found_then_error() {
    let callgrind_output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        "no_summary_no_totals",
    );

    let result = SummaryParser::new(&callgrind_output).parse(&callgrind_output);
    assert_parse_error(
        &callgrind_output.to_path(),
        result,
        "No summary or totals line found",
    );
}
