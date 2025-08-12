use std::io::{stderr, Write};

use crate::common;

#[test]
fn test_valgrind_reqs_when_running_native() {
    let mut cmd = common::get_test_bin_command("valgrind-reqs-test");
    cmd.assert().code(0).stdout("").stderr("");
}

#[test]
fn test_valgrind_reqs_when_running_on_valgrind() {
    let mut cmd = common::get_valgrind_wrapper_command();
    cmd.args([
        "1",
        "--tool=memcheck",
        "--valgrind-args=--verbose",
        &format!(
            "--bin={}",
            common::get_test_bin_path("valgrind-reqs-test").display()
        ),
    ]);

    let expected_code = 1;
    match cmd.assert().try_code(expected_code) {
        Ok(assert) => {
            let fixture_string = if cfg!(target_os = "freebsd") {
                common::get_fixture_as_string("valgrind-reqs-test.freebsd.stderr")
            } else if cfg!(target_arch = "x86_64") {
                common::get_fixture_as_string("valgrind-reqs-test.x86_64.stderr")
            } else {
                common::get_fixture_as_string("valgrind-reqs-test.stderr")
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
