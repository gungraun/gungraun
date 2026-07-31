use std::io::{Write, stderr};

use crate::common::{self, Matcher};

#[test]
fn test_memcheck_reqs_when_running_native() {
    let mut cmd = common::get_test_bin_command("memcheck-reqs-test");
    cmd.assert().code(0).stdout("").stderr("");
}

#[test]
fn test_memcheck_reqs_when_running_on_valgrind() {
    let expected_code = 1;

    let matcher = if cfg!(target_os = "freebsd") {
        Matcher::Exact(common::get_fixture_as_string(
            "memcheck-reqs-test.freebsd.stderr",
        ))
    } else if cfg!(target_os = "illumos") {
        Matcher::Exact(common::get_fixture_as_string(
            "memcheck-reqs-test.illumos.stderr",
        ))
    } else if cfg!(target_os = "macos") {
        Matcher::Contains(vec![
            (
                "Handling new value --leak-check=summary for option --leak-check=summary"
                    .to_owned(),
                1,
            ),
            (
                "Searching for pointers to <__NUMBER__> not-freed blocks
Checked <__FILTER__> bytes"
                    .to_owned(),
                4,
            ),
            (
                "LEAK SUMMARY:
definitely lost: <__FILTER__> bytes in <__FILTER__> blocks
indirectly lost: <__FILTER__> bytes in <__FILTER__> blocks
possibly lost: <__FILTER__> bytes in <__FILTER__> blocks
still reachable: <__FILTER__> bytes in <__FILTER__> blocks
suppressed: <__FILTER__> bytes in <__FILTER__> blocks"
                    .to_owned(),
                4,
            ),
            (
                "HEAP SUMMARY:
in use at exit: <__NUMBER__> bytes in <__NUMBER__> blocks
total heap usage: <__FILTER__>"
                    .to_owned(),
                1,
            ),
            (
                "ERROR SUMMARY: <__NUMBER__> errors from <__NUMBER__> contexts (suppressed: \
                 <__NUMBER__> from <__NUMBER__>)"
                    .to_owned(),
                2,
            ),
        ])
    } else if cfg!(target_arch = "arm") {
        Matcher::Exact(common::get_fixture_as_string(
            "memcheck-reqs-test.armv7.stderr",
        ))
    } else if cfg!(target_arch = "x86_64") {
        Matcher::Exact(common::get_fixture_as_string(
            "memcheck-reqs-test.x86_64.stderr",
        ))
    } else if cfg!(target_arch = "powerpc64") && cfg!(target_endian = "little") {
        Matcher::Exact(common::get_fixture_as_string(
            "memcheck-reqs-test.powerpc64le.stderr",
        ))
    } else {
        Matcher::Exact(common::get_fixture_as_string("memcheck-reqs-test.stderr"))
    };

    let mut cmd = common::get_valgrind_wrapper_command();
    cmd.args([
        "1",
        "--tool=memcheck",
        "--valgrind-args=--verbose --vgdb=no",
        &format!(
            "--bin={}",
            common::get_test_bin_path("memcheck-reqs-test").display()
        ),
    ]);

    match cmd.assert().try_code(expected_code) {
        Ok(assert) => match matcher.try_assert_output(assert) {
            Ok(_) => {}
            Err(error) => {
                panic!("{error}");
            }
        },
        Err(error) => {
            let assert = error.assert();
            let output = assert.get_output();

            let mut err = stderr();
            writeln!(err, "Unexpected exit code: STDERR:").unwrap();
            err.write_all(&output.stderr).unwrap();
            panic!(
                "Assertion of exit code failed: Actual: {}, Expected: {}",
                output.status.code().unwrap(),
                expected_code
            )
        }
    }
}
