//! Reproduce the process and output lifetimes behind the perf sampling shutdown hang, without
//! running perf. A surviving benchmark harness can keep captured output open after perf exits,
//! leaving the runner waiting for end of file (EOF).
//!
//! # Arguments and process roles
//!
//! The integration test runs `perf-parent-fixture supervisor <armed|unarmed>`. The same binary
//! runs all three roles in separate processes:
//!
//! - `supervisor` starts `parent`, waits for workload readiness, and kills only `parent`. It acts
//!   as a Linux child subreaper so it adopts the orphaned workload and can reap it.
//! - `parent` stands in for perf: it starts `workload` and waits for it, passing along its
//!   redirected standard input and output.
//! - `workload` stands in for the benchmark harness. Only `armed` calls the real perf harness
//!   initialization. Both modes report readiness and then block on standard input.
//!
//! # Streams and observations
//!
//! The supervisor uses two socket pairs. The capture socket receives the workload's standard
//! output: a five-byte readiness message (one byte indicating whether `SIGTERM` is registered,
//! then the four-byte native-endian workload PID). The supervisor waits for the whole message
//! before killing the parent, so this fixture does not exercise death before registration.
//! The lifeline socket feeds the workload's standard input. The supervisor keeps it open during
//! observation so the unarmed workload stays blocked and keeps the capture socket open.
//!
//! The supervisor reports `<registered> <eof|timeout> <signal>` on its own standard output,
//! separate from the capture socket. The tests expect `1 eof SIGTERM` in the armed case and
//! `0 timeout SIGKILL` in the control case. `SIGKILL` is sent by the supervisor after the read
//! times out, not by the kernel as a parent-death signal.

// spell-checker:ignore errno EINTR getpid pdeathsig prctl subreaper waitpid WNOHANG

#[cfg(target_os = "linux")]
mod linux {
    //! Linux implementation of the three fixture roles.

    use std::io::{ErrorKind, Read, Write};
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail, ensure};
    use nix::errno::Errno;
    use nix::sys::prctl::{get_pdeathsig, set_child_subreaper};
    use nix::sys::signal::{Signal, kill};
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::{Pid, getpid};

    /// Time budget for each capture read or reap phase, not for the entire fixture.
    const DEADLINE: Duration = Duration::from_secs(3);

    /// Own the handles needed for best-effort cleanup, including on early return.
    struct Fixture {
        /// Supervisor's end of the socket feeding workload input; closing it releases the read.
        lifeline: Option<UnixStream>,
        /// Direct child, cleared once its wait status has been collected.
        parent: Option<Child>,
    }

    impl Fixture {
        /// Kill and reap the direct parent without signaling the workload.
        ///
        /// Call once, after reading readiness and while `parent` is still present.
        fn stop_parent(&mut self) -> Result<()> {
            let parent = self.parent.as_mut().unwrap();
            parent.kill()?;
            let pid = Pid::from_raw(i32::try_from(parent.id())?);
            let status = reap(Some(pid), Instant::now() + DEADLINE)?;
            self.parent = None;
            ensure!(
                matches!(status, WaitStatus::Signaled(_, Signal::SIGKILL, _)),
                "unexpected parent exit: {status:?}"
            );
            Ok(())
        }
    }

    impl Drop for Fixture {
        /// Kill any remaining parent, close the lifeline, and try to reap remaining children.
        ///
        /// All reap attempts share one deadline, and cleanup errors are ignored.
        fn drop(&mut self) {
            if let Some(parent) = &mut self.parent {
                let _ = parent.kill();
            }
            drop(self.lifeline.take());
            let deadline = Instant::now() + DEADLINE;
            while reap(None, deadline).is_ok() {}
        }
    }

    /// Poll for a child status without blocking in `waitpid`.
    ///
    /// `None` selects any child, including the adopted workload. Retry if no status is ready
    /// or the wait is interrupted; return an error if the deadline is reached or the wait fails.
    fn reap(pid: Option<Pid>, deadline: Instant) -> Result<WaitStatus> {
        loop {
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(Errno::EINTR) => {
                    ensure!(Instant::now() < deadline, "timed out waiting for {pid:?}");
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return Ok(result?),
            }
        }
    }

    /// Validate the role and mode arguments, then run the requested role in this process.
    pub fn run() -> Result<()> {
        let mut args = std::env::args().skip(1);
        let role = args.next().context("missing fixture role")?;
        let mode = args.next().context("missing fixture mode")?;
        let armed = match mode.as_str() {
            "armed" => true,
            "unarmed" => false,
            _ => bail!("unknown fixture mode {mode}"),
        };
        match role.as_str() {
            "supervisor" => supervise(&mode),
            "parent" => {
                let status = subprocess("workload", &mode)?.status()?;
                ensure!(status.success(), "workload exited early: {status}");
                Ok(())
            }
            "workload" => workload(armed),
            _ => bail!("unknown fixture role {role}"),
        }
    }

    /// Prepare a command for another role of this binary, inheriting standard streams by default.
    fn subprocess(role: &str, mode: &str) -> Result<Command> {
        let mut command = Command::new(std::env::current_exe()?);
        command.args([role, mode]);
        Ok(command)
    }

    /// Observe output and workload exit after killing its direct parent.
    ///
    /// Reap the parent before reading for EOF so its copy of the capture socket cannot mask
    /// workload exit.
    ///
    /// A timed-out capture read is an observation, not an error. Kill the workload in that case
    /// so it can be reaped, then report the observed signal rather than an expected one.
    fn supervise(mode: &str) -> Result<()> {
        set_child_subreaper(true)?;

        let (mut capture, writer) = UnixStream::pair()?;
        capture.set_read_timeout(Some(DEADLINE))?;

        let (lifeline, reader) = UnixStream::pair()?;
        reader.set_read_timeout(Some(Duration::from_secs(10)))?;

        let parent = subprocess("parent", mode)?
            .stdin(Stdio::from(OwnedFd::from(reader)))
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .spawn()?;
        let mut fixture = Fixture {
            lifeline: Some(lifeline),
            parent: Some(parent),
        };

        let mut ready = [0; 5];
        capture
            .read_exact(&mut ready)
            .context("workload readiness")?;

        let registered = ready[0];
        let pid = Pid::from_raw(i32::from_ne_bytes(ready[1..].try_into()?));
        ensure!(
            pid.as_raw() > 1 && registered <= 1,
            "invalid readiness: {ready:?}"
        );
        fixture.stop_parent()?;

        let mut marker = [0];
        let output = match capture.read(&mut marker) {
            Ok(0) => "eof",
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                kill(pid, Signal::SIGKILL)?;
                "timeout"
            }
            result => bail!("unexpected workload output: {result:?}, byte {marker:?}"),
        };
        let status = reap(Some(pid), Instant::now() + DEADLINE)?;
        let WaitStatus::Signaled(_, signal, _) = status else {
            bail!("unexpected workload exit: {status:?}");
        };
        println!("{registered} {output} {signal}");
        Ok(())
    }

    /// Optionally run perf harness initialization, then check whether `SIGTERM` is registered.
    ///
    /// Block on the lifeline after flushing readiness. The input read has a fallback timeout;
    /// if it returns before the workload is killed, `finished` marks that it resumed execution
    /// rather than closing the capture socket silently.
    fn workload(armed: bool) -> Result<()> {
        if armed {
            gungraun::__internal::perf::init();
        }
        let mut ready = [u8::from(get_pdeathsig()? == Some(Signal::SIGTERM)); 5];
        ready[1..].copy_from_slice(&getpid().as_raw().to_ne_bytes());
        std::io::stdout().write_all(&ready)?;
        std::io::stdout().flush()?;
        let _ = std::io::stdin().read(&mut [0]);
        std::io::stdout().write_all(b"finished")?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
/// Run the selected fixture role, returning an error if setup or observation fails.
fn main() -> anyhow::Result<()> {
    linux::run()
}

#[cfg(not(target_os = "linux"))]
/// Keep the helper buildable on platforms where the Linux-only tests do not run.
fn main() {}
