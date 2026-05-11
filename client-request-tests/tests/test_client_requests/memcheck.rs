use std::io::{Write, stderr};

use crate::common;

#[test]
fn test_memcheck_reqs_when_running_native() {
    let mut cmd = common::get_test_bin_command("memcheck-reqs-test");
    cmd.assert().code(0).stdout("").stderr("");
}

#[test]
fn test_memcheck_reqs_when_running_on_valgrind() {
    let mut cmd = common::get_valgrind_wrapper_command();
    cmd.args([
        "1",
        "--tool=memcheck",
        "--valgrind-args=--verbose",
        &format!(
            "--bin={}",
            common::get_test_bin_path("memcheck-reqs-test").display()
        ),
    ]);
    let expected_code = 1;

    match cmd.assert().try_code(expected_code) {
        Ok(assert) => {
            let fixture_string = if cfg!(target_arch = "arm") {
                common::get_fixture_as_string("memcheck-reqs-test.armv7.stderr")
            } else if cfg!(target_os = "freebsd") {
                common::get_fixture_as_string("memcheck-reqs-test.freebsd.stderr")
            } else if cfg!(target_arch = "x86_64") {
                common::get_fixture_as_string("memcheck-reqs-test.x86_64.stderr")
            } else if cfg!(target_arch = "powerpc64") && cfg!(target_endian = "little") {
                common::get_fixture_as_string("memcheck-reqs-test.powerpc64le.stderr")
            } else {
                common::get_fixture_as_string("memcheck-reqs-test.stderr")
            };
            assert
                .stdout("")
                .stderr(predicates::str::diff(fixture_string));
        }
        Err(error) => {
            let assert = error.assert();
            let output = assert.get_output();

            let mut err = stderr();
            writeln!(err, "Unexpected exit code: STDERR:").unwrap();
            err.write_all(&output.stderr).unwrap();
            panic!(
                "Assertion of exit code failed: Actual: {}, Expected: {}",
                &output.status.code().unwrap(),
                expected_code
            )
        }
    }
}
