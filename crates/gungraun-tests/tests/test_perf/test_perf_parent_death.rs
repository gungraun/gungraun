//! Regression tests for a perf sampling shutdown hang.
//!
//! At the sampling cutoff, the runner stops perf. A library benchmark harness still waiting for
//! a perf control acknowledgment could outlive perf and keep the captured output pipes open.
//! The runner had reaped perf but hung waiting for EOF on those pipes.
//!
//! The fixture stands in for perf: it kills the workload's direct parent after the workload
//! signals readiness, then observes the output stream and exit status. The armed workload uses
//! the real perf harness initialization to register a parent-death signal; the unarmed workload
//! controls for the fixture itself closing the stream. These tests do not run perf or cover
//! parent death before signal registration.

#![cfg(target_os = "linux")]
// spell-checker:ignore killpg

mod helper {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    struct Supervisor {
        child: Child,
        completed: bool,
    }

    impl Drop for Supervisor {
        fn drop(&mut self) {
            if !self.completed {
                // Last resort: the supervisor and both descendants share this private process
                // group.
                let group = Pid::from_raw(i32::try_from(self.child.id()).unwrap());
                let _ = killpg(group, Signal::SIGKILL);
                let _ = self.child.wait();
            }
        }
    }

    /// Return the fixture's registration flag, output stream outcome, and workload exit signal.
    pub fn observe(mode: &str) -> String {
        let (mut observations, writer) = UnixStream::pair().unwrap();

        observations
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        let mut supervisor = Supervisor {
            child: Command::new(env!("CARGO_BIN_EXE_perf-parent-fixture"))
                .args(["supervisor", mode])
                .process_group(0)
                .stdout(Stdio::from(OwnedFd::from(writer)))
                .spawn()
                .unwrap(),
            completed: false,
        };

        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = supervisor.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "{mode} fixture timed out");
            std::thread::sleep(Duration::from_millis(10));
        };

        assert!(status.success(), "{mode} fixture failed: {status}");
        let mut observation = String::new();
        observations.read_to_string(&mut observation).unwrap();
        supervisor.completed = true;
        observation
    }
}

/// Parent death terminates the armed workload with SIGTERM and releases its output stream.
#[test]
fn armed_workload_exits_when_parent_dies() {
    assert_eq!(helper::observe("armed"), "1 eof SIGTERM\n");
}

/// Without registration, the workload keeps its output stream open until fixture cleanup.
#[test]
fn unarmed_workload_survives_parent_death() {
    assert_eq!(helper::observe("unarmed"), "0 timeout SIGKILL\n");
}
