//! TODO: DOCS

#[cfg(all(target_os = "linux", feature = "perf"))]
mod imp {
    /// Writes a message to the perf side-channel log used by the runner.
    ///
    /// When the `perf` feature is enabled on Linux, the message is written to the inherited perf
    /// log file descriptor.
    #[macro_export]
    macro_rules! perf_log {
        ($($arg:tt)*) => {{
            $crate::perf::log(&format!("{}", format_args!($($arg)*)));
        }};
    }

    /// Enables process-global perf measurement and returns an opaque token for
    /// [`perf_disable!`](crate::perf_disable!).
    ///
    /// This macro controls a single process-global perf control channel. It is not thread-safe,
    /// must not be nested, and must be paired with exactly one matching `perf_disable!` call.
    ///
    /// # Panics
    ///
    /// When multiple `perf_enable!` calls are nested
    #[macro_export]
    macro_rules! perf_enable {
        () => {{
            // SAFETY: Benchmarks have to use a single process-global perf control channel. This
            // block initializes the shared control state once, converts the stored mutable
            // reference into a raw pointer token for pairing with `perf_disable!`, and performs
            // process-global nested-section validation before sending the control command.
            unsafe {
                let __gungraun_control_slot = &raw mut $crate::perf::PERF_CONTROL;

                if (*__gungraun_control_slot).is_none() {
                    // SAFETY: `PERF_CONTROL` is initialized at most once, so each raw fd is
                    // converted into a `File` exactly once and therefore has a single owner.
                    let __gungraun_control = <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(
                        $crate::__internal::PERF_CTL_FD_WRITE,
                    );
                    // SAFETY: See comment above for the control fd.
                    let __gungraun_ack = <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(
                        $crate::__internal::PERF_ACK_FD_READ,
                    );

                    *__gungraun_control_slot = Some($crate::perf::PerfControl {
                        ack: __gungraun_ack,
                        control: __gungraun_control,
                        enabled: false,
                    });
                }

                let Some(__gungraun_control) = (*__gungraun_control_slot).as_mut() else {
                    unreachable!("gungraun: perf control initialized above");
                };
                let __gungraun_control =
                    std::ptr::from_mut::<$crate::perf::PerfControl>(__gungraun_control);

                assert!(
                    !(*__gungraun_control).enabled,
                    "gungraun: nested perf sections are unsupported"
                );

                if let Err(error) =
                    std::io::Write::write_all(&mut (*__gungraun_control).control, b"enable\n")
                {
                    panic!("gungraun: failed writing to control file: {error}");
                }

                let _ = std::io::Read::read_exact(&mut (*__gungraun_control).ack, &mut [0_u8; 1]);
                (*__gungraun_control).enabled = true;

                __gungraun_control
            }
        }};
    }

    /// Disables the active process-global perf measurement started by
    /// [`perf_enable!`](crate::perf_enable!).
    ///
    /// The token must come from the matching `perf_enable!` call. This macro is not thread-safe and
    /// panics if called without an active perf section.
    #[macro_export]
    macro_rules! perf_disable {
        ($control_token:expr) => {{
            let __gungraun_control = $control_token;

            // SAFETY: The token comes from `perf_enable!`, which returns a raw pointer to the
            // process-global `PerfControl`.
            unsafe {
                if let Err(error) =
                    std::io::Write::write_all(&mut (*__gungraun_control).control, b"disable\n")
                {
                    panic!("gungraun: failed writing to control file: {error}");
                }

                let _ = std::io::Read::read_exact(&mut (*__gungraun_control).ack, &mut [0_u8; 1]);
                assert!(
                    (*__gungraun_control).enabled,
                    "gungraun: perf_disable! called without a matching perf_enable!"
                );
                (*__gungraun_control).enabled = false;
            }
        }};
    }

    use std::fs::File;
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::sync::{Mutex, OnceLock};

    use crate::__internal::PERF_LOG_FD;

    /// Process-global perf control state for the inherited control and acknowledgement fds.
    pub struct PerfControl {
        /// File descriptor used to receive acknowledgement bytes from perf.
        pub ack: File,
        /// File descriptor used to send enable and disable commands to perf.
        pub control: File,
        /// Tracks whether a perf section is currently active for this process.
        pub enabled: bool,
    }

    /// Lazily initialized process-global perf control state.
    pub static mut PERF_CONTROL: Option<PerfControl> = None;
    /// Synchronizes writes to the inherited perf log file descriptor.
    static PERF_LOG_LOCK: OnceLock<Mutex<Option<File>>> = OnceLock::new();

    /// Writes a single message line to the perf log file.
    #[inline]
    pub fn log(message: &str) {
        let log_file = PERF_LOG_LOCK.get_or_init(open_log);

        let Ok(mut guard) = log_file.lock() else {
            eprintln!("gungraun: log lock poisoned");
            return;
        };

        if let Some(log_file) = guard.as_mut() {
            if let Err(error) = writeln!(log_file, "{message}") {
                eprintln!(
                    "gungraun: failed writing to perf log file: {error}. Falling back to stderr"
                );
                eprintln!("{message}");

                *guard = None;
            }
        } else {
            eprintln!("{message}");
        }
    }

    fn open_log() -> Mutex<Option<File>> {
        // SAFETY: The perf log fd is provided by the runner and converted into a `File` exactly
        // once when the log lock is first initialized, giving this process unique ownership.
        let log_file = unsafe { File::from_raw_fd(PERF_LOG_FD) };
        Mutex::new(Some(log_file))
    }
}

#[cfg(not(all(target_os = "linux", feature = "perf")))]
#[allow(unused)]
mod imp {
    /// No-op logging macro when perf feature is disabled.
    #[macro_export]
    macro_rules! perf_log {
        ($($arg:tt)*) => {};
    }

    /// No-op enable macro when perf feature is disabled.
    #[macro_export]
    macro_rules! perf_enable {
        () => {
            std::ptr::null_mut::<$crate::perf::PerfControl>()
        };
    }

    /// No-op disable macro when perf feature is disabled.
    #[macro_export]
    macro_rules! perf_disable {
        ($control_token:expr) => {};
    }

    pub static mut PERF_CONTROL: Option<PerfControl> = None;

    pub struct PerfControl;

    /// Writes a single message line to the perf log file.
    #[inline(always)]
    pub fn log(_message: &str) {}
}

pub use imp::log;
#[cfg(all(target_os = "linux", feature = "perf"))]
#[doc(hidden)]
pub use imp::{PERF_CONTROL, PerfControl};

#[cfg(all(target_os = "linux", feature = "perf"))]
#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write as _;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::net::UnixStream;

    use serial_test::serial;

    use super::PERF_CONTROL;
    use super::imp::PerfControl;

    struct ResetPerfControl;

    impl Drop for ResetPerfControl {
        fn drop(&mut self) {
            // SAFETY: These tests run serially when mutating `PERF_CONTROL`, and the guard exists
            // solely to restore the global to its empty state during unwinding.
            unsafe {
                PERF_CONTROL = None;
            }
        }
    }

    struct PerfControlPeers {
        _control_reader: UnixStream,
        ack_writer: UnixStream,
    }

    fn perf_control(enabled: bool) -> (PerfControl, PerfControlPeers) {
        let (ack_reader, ack_writer) = UnixStream::pair().unwrap();
        let (control_reader, control_writer) = UnixStream::pair().unwrap();

        // SAFETY: Each raw fd is consumed exactly once here to give the test sole ownership of the
        // backing stream endpoints as `File`s.
        let ack = unsafe { File::from_raw_fd(ack_reader.into_raw_fd()) };
        // SAFETY: Each raw fd is consumed exactly once here to give the test sole ownership of the
        // backing stream endpoints as `File`s.
        let control = unsafe { File::from_raw_fd(control_writer.into_raw_fd()) };

        (
            PerfControl {
                ack,
                control,
                enabled,
            },
            PerfControlPeers {
                _control_reader: control_reader,
                ack_writer,
            },
        )
    }

    #[test]
    #[serial]
    #[should_panic(expected = "nested perf sections are unsupported")]
    fn perf_enable_when_nested_then_panics() {
        let _reset = ResetPerfControl;

        // SAFETY: This test runs serially and temporarily installs a fresh process-global control
        // object to exercise the nested-enable panic path.
        unsafe {
            PERF_CONTROL = Some(perf_control(true).0);
        }

        let _ = crate::perf_enable!();
    }

    #[test]
    #[should_panic(expected = "perf_disable! called without a matching perf_enable!")]
    fn perf_disable_when_no_matching_enable_then_panics() {
        let (mut control, mut peers) = perf_control(false);
        peers.ack_writer.write_all(&[0_u8]).unwrap();
        let token = std::ptr::from_mut::<PerfControl>(&mut control);
        crate::perf_disable!(token);
    }
}
