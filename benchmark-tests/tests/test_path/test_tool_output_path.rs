use Tool::*;
use gungraun::Tool;
use gungraun_runner::fixtures::tool_output_path_f;
use rstest::rstest;
use tempfile::tempdir;

#[rstest]
#[case::empty(
    Callgrind,
    &[],
    &[]
)]
#[case::callgrind_out_zero_pid(
    Callgrind,
    &["callgrind.function.bench.out.#0"],
    &[]
)]
#[case::callgrind_out_some_pid(
    Callgrind,
    &["callgrind.function.bench.out.#12345"],
    &[]
)]
#[case::callgrind_out_some_pid_with_trail(
    Callgrind,
    &["callgrind.function.bench.out.#12345-xx-10-rew"],
    &[]
)]
#[case::callgrind_log_zero_pid(
    Callgrind,
    &["callgrind.function.bench.log.#0"],
    &[]
)]
#[case::callgrind_log_some_pid(
    Callgrind,
    &["callgrind.function.bench.log.#12345"],
    &[]
)]
#[case::callgrind_log_some_pid_with_trail(
    Callgrind,
    &["callgrind.function.bench.log.#12345-xx-10-rew"],
    &[]
)]
#[case::callgrind_xtree_some_pid(
    Callgrind,
    &["callgrind.function.bench.xtree.#12345"],
    &[]
)]
#[case::callgrind_xleak_some_pid(
    Callgrind,
    &["callgrind.function.bench.xleak.#12345"],
    &[]
)]
#[case::callgrind_type_does_not_matter_some_pid(
    Callgrind,
    &["callgrind.function.bench.does_not_matter.#12345"],
    &[]
)]
#[case::callgrind_old(
    Callgrind,
    &["callgrind.function.bench.out.old.#12345"],
    &[]
)]
#[case::callgrind_base_foo(
    Callgrind,
    &["callgrind.function.bench.out.base@foo.#12345"],
    &[]
)]
#[case::callgrind_multiple(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "callgrind.function.bench.out.#54321"],
    &[]
)]
#[case::callgrind_multiple_different_types(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "callgrind.function.bench.log.#12354"],
    &[]
)]
#[case::callgrind_dhat_no_clear(
    Callgrind,
    &["dhat.function.bench.out.#12345"],
    &["dhat.function.bench.out.#12345"]
)]
#[case::callgrind_multiple_mixed_dhat_no_clear(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "dhat.function.bench.out.#12345"],
    &["dhat.function.bench.out.#12345"]
)]
#[case::tool_does_not_match_then_no_clear(
    DHAT,
    &["callgrind.function.bench.out.#12345"],
    &["callgrind.function.bench.out.#12345"]
)]
#[case::name_does_not_match_then_no_clear(
    Callgrind,
    &["callgrind.a.b.out.#12345"],
    &["callgrind.a.b.out.#12345"]
)]
#[case::missing_point_then_no_clear(
    Callgrind,
    &["callgrind.function.bench.out#12345"],
    &["callgrind.function.bench.out#12345"]
)]
fn test_clear_temp_files(
    #[case] tool: Tool,
    #[case] files: &[&str],
    #[case] expected_files: &[&str],
) {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(tool)
        .init(true)
        .files(files.iter().map(|f| (*f, "")))
        .fx();

    output_path
        .clear_temp_files(false)
        .expect("Clearing the temporary files should succeed");

    let dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");

    assert_eq!(dir_entries, expected_files);
}

#[rstest]
#[case::simple_out_then_no_change(
    Memcheck,
    &["memcheck.function.bench.out"],
    &["memcheck.function.bench.out"]
)]
#[case::simple_log_then_no_change(
    Memcheck,
    &["memcheck.function.bench.log"],
    &["memcheck.function.bench.log"]
)]
#[case::already_sanitized_no_change(
    Memcheck,
    &["memcheck.function.bench.12345.out"],
    &["memcheck.function.bench.12345.out"],
)]
#[case::one_pid_and_modifier(
    Memcheck,
    &["memcheck.function.bench.out.#12345.cal"],
    &["memcheck.function.bench.cal.out"]
)]
#[case::one_pid_without_modifier(
    Memcheck,
    &["memcheck.function.bench.out.#12345"],
    &["memcheck.function.bench.out"]
)]
#[case::two_pids_and_modifier(
    Memcheck,
    &["memcheck.function.bench.out.#12345.cal", "memcheck.function.bench.out.#54321.cal"],
    &["memcheck.function.bench.cal.12345.out", "memcheck.function.bench.cal.54321.out"]
)]
#[case::two_pids_and_modifier_already_sanitized(
    Memcheck,
    &["memcheck.function.bench.cal.12345.out", "memcheck.function.bench.cal.54321.out"],
    &["memcheck.function.bench.cal.12345.out", "memcheck.function.bench.cal.54321.out"]
)]
#[case::two_pids_without_modifier(
    Memcheck,
    &["memcheck.function.bench.out.#12345", "memcheck.function.bench.out.#54321"],
    &["memcheck.function.bench.12345.out", "memcheck.function.bench.54321.out"]
)]
#[case::base_with_modifier(
    Memcheck,
    &["memcheck.function.bench.out.base@foo.#12345.cal"],
    &["memcheck.function.bench.cal.out.base@foo"]
)]
#[case::old_file_then_no_change(
    Memcheck,
    &["memcheck.function.bench.out.old.#12345"],
    &["memcheck.function.bench.out.old.#12345"]
)]
fn test_sanitize_generic(
    #[case] tool: Tool,
    #[case] files: &[&str],
    #[case] expected_files: &[&str],
) {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(tool)
        .init(true)
        .files(files.iter().map(|f| (*f, "something")))
        .fx();

    output_path.sanitize().unwrap();

    let mut dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");

    dir_entries.sort();

    assert_eq!(dir_entries, expected_files);
}

#[test]
fn test_when_sanitize_bbv_has_multiple_threads_then_thread_modifier_is_added() {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(BBV)
        .init(true)
        .files([
            ("exp-bbv.function.bench.out.bb.#12345", "something"),
            ("exp-bbv.function.bench.out.bb.#12345.2", "something"),
        ])
        .fx();

    output_path.sanitize().unwrap();

    let mut dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");
    dir_entries.sort();

    assert_eq!(
        dir_entries,
        [
            "exp-bbv.function.bench.t1.bb.out",
            "exp-bbv.function.bench.t2.bb.out",
        ]
    );
}

#[test]
fn test_when_sanitize_callgrind_has_multiple_log_pids_then_pid_is_kept() {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(Callgrind)
        .init(true)
        .files([
            ("callgrind.function.bench.log.#12345", "something"),
            ("callgrind.function.bench.log.#54321", "something"),
        ])
        .fx();

    output_path.sanitize().unwrap();

    let mut dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");
    dir_entries.sort();

    assert_eq!(
        dir_entries,
        [
            "callgrind.function.bench.12345.log",
            "callgrind.function.bench.54321.log",
        ]
    );
}

#[test]
fn test_when_sanitize_perf_has_multiple_parts_then_part_is_moved_before_output_kind() {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(Perf)
        .init(true)
        .files([
            ("perf.function.bench.out.p1", "something"),
            ("perf.function.bench.out.p2", "something"),
        ])
        .fx();

    output_path.sanitize().unwrap();

    let mut dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");
    dir_entries.sort();

    assert_eq!(
        dir_entries,
        ["perf.function.bench.p1.out", "perf.function.bench.p2.out"]
    );
}

#[test]
fn test_when_sanitize_perf_matches_empty_file_then_empty_file_is_removed() {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(Perf)
        .init(true)
        .files([("perf.function.bench.out.p1", "")])
        .fx();

    output_path.sanitize().unwrap();

    let dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");

    assert_eq!(dir_entries, [] as [&str; 0]);
}

#[rstest]
#[case::empty(&[], &[])]
#[case::plain_out(
    &["callgrind.function.bench.out"],
    &[("callgrind.function.bench.out", None)]
)]
#[case::single_modifier(
    &["callgrind.function.bench.cal.out"],
    &[("callgrind.function.bench.cal.out", Some(".cal"))]
)]
#[case::multiple_modifiers(
    &["callgrind.function.bench.cal.overhead.out"],
    &[("callgrind.function.bench.cal.overhead.out", Some(".cal.overhead"))]
)]
#[case::wrong_kind_ignored(&["callgrind.function.bench.log"], &[])]
#[case::wrong_name_ignored(&["callgrind.other.bench.out"], &[])]
#[case::unrelated_file_ignored(&["summary.json"], &[])]
fn test_sanitized_paths_with_modifier(
    #[case] files: &[&str],
    #[case] expected: &[(&str, Option<&str>)],
) {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(Callgrind)
        .init(true)
        .files(files.iter().map(|f| (*f, "something")))
        .fx();

    let mut actual = output_path
        .sanitized_paths_with_modifier()
        .expect("Getting paths with modifiers should succeed")
        .into_iter()
        .map(|(path, modifier)| {
            (
                path.file_name().unwrap().to_string_lossy().to_string(),
                modifier,
            )
        })
        .collect::<Vec<_>>();
    actual.sort();

    let expected = expected
        .iter()
        .map(|(path, modifier)| ((*path).to_owned(), modifier.map(str::to_owned)))
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}
