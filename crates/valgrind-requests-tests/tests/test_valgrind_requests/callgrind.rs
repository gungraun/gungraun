use crate::common::{self, get_fixture_as_string, get_test_bin_path};

#[test]
fn test_callgrind_reqs_when_running_native() {
    let mut cmd = common::get_test_bin_command("callgrind-reqs-test");
    cmd.assert().code(0).stdout("").stderr("");
}

#[test]
fn test_callgrind_reqs_when_running_on_valgrind() {
    let mut cmd = common::get_valgrind_wrapper_command();
    cmd.args([
        "1",
        "--tool=callgrind",
        "--valgrind-args=--verbose --vgdb=no",
        &format!(
            "--bin={}",
            get_test_bin_path("callgrind-reqs-test").display()
        ),
    ]);
    cmd.assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::diff(get_fixture_as_string(
            "callgrind-reqs-test.stderr",
        )));
}
