use std::io::{Write, stderr};

use version_compare::Cmp;

use crate::common;

#[test]
fn test_client_request_print_macros_when_running_native() {
    let mut cmd = common::get_test_bin_command("print-macros-test");
    cmd.assert().code(0).stdout("").stderr("");
}

#[test]
fn test_client_request_print_macros_when_running_on_valgrind() {
    let mut cmd = common::get_valgrind_wrapper_command();
    cmd.args([
        "1",
        "--tool=callgrind",
        "--valgrind-args=--verbose --vgdb=no",
        &format!(
            "--bin={}",
            common::get_test_bin_path("print-macros-test").display()
        ),
    ]);

    let sandbox = common::get_sandbox();
    cmd.current_dir(&sandbox);

    let expected_code = 1;
    match cmd.assert().try_code(expected_code) {
        Ok(assert) => {
            let fixture_string = if common::compare_rust_version(Cmp::Ge, "1.79.0")
                && cfg!(target_arch = "arm")
            {
                common::get_fixture("print-macros-test", Some("armv7"), Some("1.79.0"), "stderr")
            } else {
                common::get_fixture("print-macros-test", None, None, "stderr")
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
                output.status.code().unwrap(),
                expected_code
            )
        }
    }
    drop(sandbox);
}
