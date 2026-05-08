use std::process::Stdio as StdStdio;
use std::time::Duration;

use gungraun::ExitWith;
use gungraun_runner::error::Error;
use gungraun_runner::runner::args::NoCapture;
use gungraun_runner::runner::common::AssistantKind;
use gungraun_runner::runner::tool::run::RunOptions;
use rstest::rstest;
use tempfile::tempdir;

use crate::assert_not_elapsed;
use crate::util::common::{
    BENCH_BIN_FAKE_EXE, ECHO_EXE, EXIT_WITH_EXE, FILE_EXISTS_EXE, TIMEOUT_EXE,
    cleanup_test_process_handler, tool_output_path_dump,
};
use crate::util::fixtures::{
    assistant_f, config_f, force_shutdown_f, module_path_f, process_handler_f, run_options_f,
    setup_child_f, teardown_child_f, test_file_f, tool_command_child_f, tool_command_f,
    tool_config_f, tool_output_path_f,
};

#[test]
fn test_start_assistant_when_setup() {
    let assistant = assistant_f().kind(AssistantKind::Setup).fixture();
    let config = config_f().bench_bin(&BENCH_BIN_FAKE_EXE).fixture();
    let module_path = config.module_path.join("does::not_matter");

    let mut process_handler = process_handler_f().fixture();
    process_handler
        .start_assistant(
            true,
            &assistant,
            &config,
            &module_path,
            None,
            NoCapture::False,
        )
        .expect("Starting the setup should succeed");

    let (id, child) = process_handler
        .setup
        .expect("A setup child should be present");

    assert_eq!(AssistantKind::Setup.id(), id);
    let output = child
        .wait_with_output()
        .expect("An output should be present");

    assert!(output.status.success());
    assert!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .ends_with("bench-bin-fake")
    );
}

#[test]
fn test_start_assistant_when_teardown() {
    let assistant = assistant_f().kind(AssistantKind::Teardown).fixture();
    let config = config_f().bench_bin(&BENCH_BIN_FAKE_EXE).fixture();
    let module_path = config.module_path.join("does::not_matter");

    let mut process_handler = process_handler_f().fixture();
    process_handler
        .start_assistant(
            true,
            &assistant,
            &config,
            &module_path,
            None,
            NoCapture::False,
        )
        .expect("Starting the teardown should succeed");

    let (id, child) = process_handler
        .teardown
        .expect("A teardown child should be present");

    assert_eq!(AssistantKind::Teardown.id(), id);
    let output = child
        .wait_with_output()
        .expect("An output should be present");

    assert!(output.status.success());
    assert!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .ends_with("bench-bin-fake")
    );
}

#[rstest]
#[case::setup(AssistantKind::Setup)]
#[case::teardown(AssistantKind::Teardown)]
fn test_start_assistant_when_force_shutdown_is_true_then_interrupt(#[case] kind: AssistantKind) {
    let assistant = assistant_f().kind(kind).fixture();
    let config = config_f().bench_bin(&BENCH_BIN_FAKE_EXE).fixture();
    let module_path = config.module_path.join("does::not_matter");

    let force_shutdown = force_shutdown_f().yes(true).fixture();
    let mut process_handler = process_handler_f()
        .set_force_shutdown(force_shutdown)
        .fixture();

    let error = process_handler
        .start_assistant(
            true,
            &assistant,
            &config,
            &module_path,
            None,
            NoCapture::False,
        )
        .expect_err("An error should be present")
        .downcast::<Error>()
        .expect("The error should be a gungraun error");

    assert!(process_handler.setup.is_none());
    assert!(process_handler.teardown.is_none());
    assert!(matches!(error, Error::TaskInterrupt));
}

#[rstest]
#[case::with_setup_parallel(true, true)]
#[case::with_setup_not_parallel(false, true)]
#[case::without_setup_parallel(true, true)]
#[case::without_setup_not_parallel(false, true)]
fn test_start_bench_when_force_shutdown_is_false_then_bench_is_started(
    #[case] setup_is_parallel: bool,
    #[case] has_setup: bool,
) {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_output_path = tool_output_path_f()
        .target_dir(test_dir.path())
        .init(true)
        .fixture();

    let mut handler = if has_setup {
        process_handler_f()
            .setup_is_parallel(setup_is_parallel)
            .assistant(setup_child_f().exe(&TIMEOUT_EXE).args(&["1000"]).fixture())
            .fixture()
    } else {
        process_handler_f()
            .setup_is_parallel(setup_is_parallel)
            .fixture()
    };

    handler
        .start_bench(
            tool_command_f().output_path(&tool_output_path).fixture(),
            &tool_config_f().fixture(),
            &ECHO_EXE,
            &["foo".into()],
            &run_options_f()
                .env_clear(!cfg!(target_os = "macos"))
                .fixture(),
            &tool_output_path,
            &module_path_f().fixture(),
            None,
            None,
        )
        .expect("Starting the benchmark should succeed");

    let output = handler
        .bench
        .take()
        .expect("The tool command child process should be present")
        .child
        .take()
        .expect("The benchmark child process should be present")
        .wait_with_output()
        .expect("Waiting for the benchmark process to exit should succeed");

    if !output.status.success() {
        use std::io::stderr;

        let mut writer = stderr();
        tool_output_path_dump(&tool_output_path.to_log_output(), &mut writer).unwrap();
        panic!("Assertion failed: Exit status was failure");
    }

    assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "foo\n");

    cleanup_test_process_handler(handler);
}

#[rstest]
#[case::setup_without_error(0)]
#[case::setup_with_error(200)]
fn test_start_bench_when_setup_is_parallel_then_bench_is_started(#[case] exit_code: i32) {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_output_path = tool_output_path_f()
        .target_dir(test_dir.path())
        .init(true)
        .fixture();

    let mut handler = process_handler_f()
        .setup_is_parallel(true)
        .assistant(
            setup_child_f()
                .exe(&EXIT_WITH_EXE)
                .args(&[&exit_code.to_string()])
                .fixture(),
        )
        .fixture();

    let run_options = run_options_f()
        .env_clear(!cfg!(target_os = "macos"))
        .fixture();

    let result = handler.start_bench(
        tool_command_f()
            .output_path(&tool_output_path)
            .run_options(&run_options)
            .fixture(),
        &tool_config_f().fixture(),
        &ECHO_EXE,
        &[],
        &run_options,
        &tool_output_path,
        &module_path_f().fixture(),
        None,
        None,
    );

    result.expect("Starting the benchmark should succeed");
    let child = handler
        .bench
        .take()
        .expect("The tool command child process should be present")
        .child
        .take()
        .expect("The benchmark child process should be present");

    let output = child
        .wait_with_output()
        .expect("Waiting for the benchmark process to exit should succeed");

    if !output.status.success() {
        use std::io::stderr;

        let mut writer = stderr();
        tool_output_path_dump(&tool_output_path.to_log_output(), &mut writer).unwrap();
        panic!("Assertion failed: Exit status was failure");
    }

    cleanup_test_process_handler(handler);
}

#[rstest]
#[case::with_setup(true)]
#[case::without_setup(false)]
fn test_start_bench_when_force_shutdown_is_true_then_interrupt(#[case] has_setup: bool) {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_output_path = tool_output_path_f()
        .target_dir(test_dir.path())
        .init(true)
        .fixture();

    let force_shutdown = force_shutdown_f().yes(true).fixture();
    let mut handler = if has_setup {
        process_handler_f()
            .setup_is_parallel(false)
            .assistant(setup_child_f().exe(&TIMEOUT_EXE).args(&["10000"]).fixture())
            .set_force_shutdown(force_shutdown)
            .fixture()
    } else {
        process_handler_f()
            .setup_is_parallel(false)
            .set_force_shutdown(force_shutdown)
            .fixture()
    };

    let result = handler.start_bench(
        tool_command_f().output_path(&tool_output_path).fixture(),
        &tool_config_f().fixture(),
        &ECHO_EXE,
        &[],
        &RunOptions::default(),
        &tool_output_path,
        &module_path_f().fixture(),
        None,
        None,
    );

    assert!(handler.bench.is_none());
    let error = result
        .unwrap_err()
        .downcast::<Error>()
        .expect("The error type should be a gungraun error");
    assert!(matches!(error, Error::TaskInterrupt));

    cleanup_test_process_handler(handler);
}

/// Test, if a failed non-parallel setup process is waited for to exit and the benchmark process is
/// not started.
#[test]
fn test_start_bench_when_setup_not_parallel_with_error_then_no_bench_and_setup_error() {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_output_path = tool_output_path_f()
        .target_dir(test_dir.path())
        .init(true)
        .fixture();

    let expected_exit_code = 200;
    let mut handler = process_handler_f()
        .assistant(
            setup_child_f()
                .exe(&EXIT_WITH_EXE)
                .args(&[&expected_exit_code.to_string()])
                .fixture(),
        )
        .fixture();

    let result = handler.start_bench(
        tool_command_f().output_path(&tool_output_path).fixture(),
        &tool_config_f().fixture(),
        &ECHO_EXE,
        &[],
        &RunOptions::default(),
        &tool_output_path,
        &module_path_f().fixture(),
        None,
        None,
    );

    assert!(handler.bench.is_none());
    match result
        .unwrap_err()
        .downcast::<Error>()
        .expect("The error should be the gungraun error")
    {
        Error::ProcessError(_, output, _) => {
            assert_eq!(
                output
                    .status
                    .code()
                    .expect("The exit status code should be present"),
                expected_exit_code
            )
        }
        _ => {
            panic!("The error should be a process error")
        }
    };

    cleanup_test_process_handler(handler);
}

#[test]
#[should_panic = "A benchmark should be started before waiting"]
fn test_wait_or_shutdown_when_no_bench_then_panic() {
    let mut handler = process_handler_f().fixture();
    let _ = handler.wait_or_shutdown();
}

#[rstest]
#[case::with_setup(true)]
#[case::without_setup(false)]
fn test_wait_or_shutdown_when_force_shutdown_is_false(#[case] has_setup: bool) {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_command_child = tool_command_child_f()
        .exe(&ECHO_EXE)
        .args(&["foo"])
        .stdout(StdStdio::piped())
        .log_path(
            tool_output_path_f()
                .target_dir(test_dir.path())
                .init(true)
                .fixture()
                .to_log_output(),
        )
        .fixture();

    let mut handler = if has_setup {
        process_handler_f()
            .assistant(setup_child_f().exe(&TIMEOUT_EXE).args(&["100"]).fixture())
            .bench(tool_command_child)
            .fixture()
    } else {
        process_handler_f().bench(tool_command_child).fixture()
    };

    let output = handler
        .wait_or_shutdown()
        .expect("Waiting for the benchmark process should succeed");

    assert!(handler.bench.is_none());
    assert!(output.status.success());
    assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "foo\n");

    cleanup_test_process_handler(handler);
}

#[rstest]
#[case::with_setup(true)]
#[case::without_setup(false)]
fn test_wait_or_shutdown_when_force_shutdown_is_true_then_interrupt(#[case] has_setup: bool) {
    let test_dir = tempfile::tempdir().unwrap();

    let tool_command_child = tool_command_child_f()
        .exe(&TIMEOUT_EXE)
        .args(&["5000"])
        .log_path(
            tool_output_path_f()
                .target_dir(test_dir.path())
                .init(true)
                .fixture()
                .to_log_output(),
        )
        .fixture();

    let force_shutdown = force_shutdown_f().yes(true).fixture();
    let mut handler = if has_setup {
        process_handler_f()
            .assistant(setup_child_f().exe(&TIMEOUT_EXE).args(&["10000"]).fixture())
            .set_force_shutdown(force_shutdown)
            .bench(tool_command_child)
            .fixture()
    } else {
        process_handler_f()
            .set_force_shutdown(force_shutdown)
            .bench(tool_command_child)
            .fixture()
    };

    let error = assert_not_elapsed!(
        Duration::from_millis(500),
        handler
            .wait_or_shutdown()
            .expect_err("An error should be present")
            .downcast::<Error>()
            .expect("The error should be a gungraun error")
    );

    assert!(handler.bench.is_none());
    assert!(matches!(error, Error::TaskInterrupt));

    cleanup_test_process_handler(handler);
}

#[rstest]
#[case::without_error_in_bench(0)]
#[case::with_error_in_bench(200)]
fn test_wait_or_shutdown_when_error_in_setup_then_setup_error(#[case] exit_code: i32) {
    let test_dir = tempfile::tempdir().unwrap();
    let expected_exit_code = 222;

    let tool_command_child = tool_command_child_f()
        .exe(&EXIT_WITH_EXE)
        .args(&[&exit_code.to_string()])
        .log_path(
            tool_output_path_f()
                .target_dir(test_dir.path())
                .init(true)
                .fixture()
                .to_log_output(),
        )
        .fixture();

    let mut handler = process_handler_f()
        .assistant(
            setup_child_f()
                .exe(&EXIT_WITH_EXE)
                .args(&[&expected_exit_code.to_string()])
                .fixture(),
        )
        .bench(tool_command_child)
        .fixture();

    let result = handler
        .wait_or_shutdown()
        .expect_err("An error should be present")
        .downcast::<Error>()
        .expect("The error should be a gungraun error");

    assert!(handler.bench.is_none());
    match result {
        Error::ProcessError(_, output, _) => {
            assert_eq!(
                output
                    .status
                    .code()
                    .expect("An exit code should be present"),
                expected_exit_code
            );
        }
        _ => {
            panic!("A process error should be present");
        }
    }

    cleanup_test_process_handler(handler);
}

#[rstest]
#[case::exit_with_success(0)]
#[case::exit_with_error(200)]
fn test_wait_or_shutdown_when_exit_with(#[case] exit_with_code: i32) {
    let test_dir = tempfile::tempdir().unwrap();

    let exit_with = ExitWith::Code(exit_with_code);
    let tool_command_child = tool_command_child_f()
        .exe(&EXIT_WITH_EXE)
        .args(&[&exit_with_code.to_string()])
        .exit_with(exit_with)
        .log_path(
            tool_output_path_f()
                .target_dir(test_dir.path())
                .init(true)
                .fixture()
                .to_log_output(),
        )
        .fixture();

    let mut handler = process_handler_f().bench(tool_command_child).fixture();

    let output = handler
        .wait_or_shutdown()
        .expect("Waiting for the benchmark to exit with the expected code should succeed");

    assert!(handler.bench.is_none());
    assert_eq!(
        output
            .status
            .code()
            .expect("An exit code should be present"),
        exit_with_code
    );

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_or_shutdown_when_exit_with_no_match_then_error() {
    let test_dir = tempfile::tempdir().unwrap();
    let actual_exit_code = 0;

    let tool_command_child = tool_command_child_f()
        .exe(&EXIT_WITH_EXE)
        .args(&[&actual_exit_code.to_string()])
        .exit_with(ExitWith::Code(222))
        .log_path(
            tool_output_path_f()
                .target_dir(test_dir.path())
                .init(true)
                .fixture()
                .to_log_output(),
        )
        .fixture();

    let mut handler = process_handler_f().bench(tool_command_child).fixture();

    let error = handler
        .wait_or_shutdown()
        .expect_err(
            "There should be an error if the actual exit code does not match the `ExitWith` code",
        )
        .downcast::<Error>()
        .expect("The error should be a gungraun error");

    assert!(handler.bench.is_none());
    match error {
        Error::ProcessError(_, output, _) => {
            assert_eq!(
                output
                    .status
                    .code()
                    .expect("An exit code should be present"),
                actual_exit_code
            );
        }
        _ => {
            panic!("The error should be a process error");
        }
    }

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_for_setup_when_no_setup() {
    let mut handler = process_handler_f().fixture();
    let result = handler.wait_for_setup();

    assert!(handler.setup.is_none());
    assert!(result.is_none());

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_for_setup() {
    let test_dir = tempdir().unwrap();
    let (test_file_path, _) = test_file_f().dir(test_dir.path()).fixture();

    let mut handler = process_handler_f()
        .assistant(
            setup_child_f()
                .exe(&FILE_EXISTS_EXE)
                .args(&[&test_file_path.display().to_string(), "true"])
                .fixture(),
        )
        .fixture();

    handler
        .wait_for_setup()
        .expect("A result should be present")
        .expect("There should be no errors");

    assert!(handler.setup.is_none());

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_for_setup_when_force_shutdown_is_true_then_interrupt() {
    let force_shutdown = force_shutdown_f().yes(true).fixture();
    let mut handler = process_handler_f()
        .assistant(setup_child_f().exe(&TIMEOUT_EXE).args(&["5000"]).fixture())
        .set_force_shutdown(force_shutdown)
        .fixture();

    let error = assert_not_elapsed!(
        Duration::from_millis(500),
        handler
            .wait_for_setup()
            .expect("A result should be present")
            .expect_err("There should be an error")
            .downcast::<Error>()
            .expect("The error should be a gungraun error")
    );

    assert!(handler.setup.is_none());
    assert!(matches!(error, Error::TaskInterrupt));

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_for_teardown_when_no_teardown() {
    let mut handler = process_handler_f().fixture();
    let result = handler.wait_for_teardown();

    assert!(handler.teardown.is_none());
    assert!(result.is_none());

    cleanup_test_process_handler(handler);
}

#[test]
fn test_wait_for_teardown() {
    let test_dir = tempdir().unwrap();
    let (test_file_path, _) = test_file_f().dir(test_dir.path()).fixture();

    let mut handler = process_handler_f()
        .assistant(
            teardown_child_f()
                .exe(&FILE_EXISTS_EXE)
                .args(&[&test_file_path.display().to_string(), "true"])
                .fixture(),
        )
        .fixture();

    handler
        .wait_for_teardown()
        .expect("A result should be present")
        .expect("There should be no errors");

    assert!(handler.teardown.is_none());

    cleanup_test_process_handler(handler);
}
