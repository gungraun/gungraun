use gungraun_runner::api::{DhatMetric, EntryPoint};
use gungraun_runner::metrics::model::Metrics;
use gungraun_runner::runner::dhat::json_parser::parse;
use gungraun_runner::runner::dhat::model::{DhatData, ProgramPoint};
use gungraun_runner::runner::dhat::tree::{Data, DhatTree, Tree};
use gungraun_runner::summary::model::ToolMetrics;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::util::common::Fixtures;

fn sorted_program_points(data: &DhatData) -> Vec<ProgramPoint> {
    let mut program_points = data.program_points.clone();
    program_points.sort_by(|a, b| a.frames.cmp(&b.frames));
    program_points
}

#[rstest]
#[case::heap("dhat/dhat.with_entry_point.out")]
#[case::ad_hoc("dhat/dhat.ad_hoc_mode.out")]
#[case::copy("dhat/dhat.copy_mode.out")]
fn test_dhat_data_dhat_tree_round_trip_for_mode(#[case] fixture: &str) {
    let path = Fixtures::get_path_of(fixture);
    let data: DhatData = parse(&path).unwrap();

    let tree = DhatTree::from_json(data.clone());
    let actual = DhatData::from(tree);

    assert_eq!(actual.metadata, data.metadata);
    assert_eq!(actual.frame_table, data.frame_table);
    assert_eq!(sorted_program_points(&actual), sorted_program_points(&data));
}

#[test]
fn test_dhat_tree_when_ad_hoc_mode() {
    let path = Fixtures::get_path_of("dhat/dhat.ad_hoc_mode.out");
    let dhat_data: DhatData = parse(&path).unwrap();
    let data = Data::from(&dhat_data.program_points[0]);
    let mut expected_tree = DhatTree::with_metadata(dhat_data.metadata.clone());
    expected_tree.insert(&[1, 2, 3, 4], &data, &dhat_data.frame_table);

    let mut metrics = Metrics::empty();
    metrics.insert_all(&[
        (DhatMetric::TotalUnits, 15.into()),
        (DhatMetric::TotalEvents, 1.into()),
    ]);
    let expected_metrics = ToolMetrics::Dhat(metrics);

    let actual = DhatTree::from_json(dhat_data);

    assert_eq!(actual, expected_tree);
    assert_eq!(actual.metrics(), expected_metrics);
}

#[test]
fn test_dhat_tree_when_copy_mode() {
    let path = Fixtures::get_path_of("dhat/dhat.copy_mode.out");
    let mut dhat_data: DhatData = parse(&path).unwrap();
    let data = Data::from(
        dhat_data
            .program_points
            .iter()
            .find(|program_point| program_point.frames == [1, 2, 3, 4])
            .unwrap(),
    );
    let mut expected_tree = DhatTree::with_metadata(dhat_data.metadata.clone());
    expected_tree.insert(&[1, 2, 3, 4], &data, &dhat_data.frame_table);

    let mut metrics = Metrics::empty();
    metrics.insert_all(&[
        (DhatMetric::TotalBytes, 20.into()),
        (DhatMetric::TotalBlocks, 1.into()),
    ]);
    let expected_metrics = ToolMetrics::Dhat(metrics);

    dhat_data.filter_program_points(&EntryPoint::Default, &[]);
    let actual = DhatTree::from_json(dhat_data);

    assert_eq!(actual, expected_tree);
    assert_eq!(actual.metrics(), expected_metrics);
}

#[test]
fn test_dhat_tree_when_entry_point_and_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let mut data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    expected.insert(
        &[1],
        &Data::from(&data.program_points[1]),
        &data.frame_table,
    );
    data.filter_program_points(&EntryPoint::Default, &["malloc".to_owned()]);
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}

#[test]
fn test_dhat_tree_when_entry_point_and_no_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let mut data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    data.filter_program_points(&EntryPoint::Default, &[]);
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}

#[test]
fn test_dhat_tree_when_entry_point_custom_and_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let mut data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    expected.insert(
        &[5],
        &Data::from(&data.program_points[2]),
        &data.frame_table,
    );
    data.filter_program_points(
        &EntryPoint::Custom("test_dhat::*".to_owned()),
        &["calloc".to_owned()],
    );
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}

#[test]
fn test_dhat_tree_when_entry_point_custom_no_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let mut data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    data.filter_program_points(&EntryPoint::Custom("test_dhat::*".to_owned()), &[]);
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}

#[test]
fn test_dhat_tree_when_no_entry_point_but_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let mut data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    data.filter_program_points(&EntryPoint::None, &["test_dhat::tool::*".to_owned()]);
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}

#[test]
fn test_dhat_tree_when_no_entry_point_no_frames() {
    let path = Fixtures::get_path_of("dhat/dhat.with_entry_point.out");
    let data: DhatData = parse(&path).unwrap();
    let mut expected = DhatTree::with_metadata(data.metadata.clone());
    expected.insert(
        &[1, 2, 3, 4],
        &Data::from(&data.program_points[0]),
        &data.frame_table,
    );
    expected.insert(
        &[1],
        &Data::from(&data.program_points[1]),
        &data.frame_table,
    );
    expected.insert(
        &[5],
        &Data::from(&data.program_points[2]),
        &data.frame_table,
    );
    let actual = DhatTree::from_json(data);

    assert_eq!(actual, expected);
}
