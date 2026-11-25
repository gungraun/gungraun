//! Macros and process-global control state for the `perf` feature.
//!
//! When the `perf` feature is enabled on Linux, the [`perf_enable!`], [`perf_disable!`] and
//! [`perf_log!`] macros allow benchmarks to communicate with the runner's `perf` measurement
//! infrastructure. The macros are safe to call from any thread, but only one measurement section
//! may be active per process. Nested or overlapping sections panic instead of causing undefined
//! behavior.
//!
//! On non-Linux platforms or when the feature is disabled, the macros compile to no-ops.
//!
//! [`perf_enable!`]: crate::perf_enable
//! [`perf_disable!`]: crate::perf_disable
//! [`perf_log!`]: crate::perf_log

#![expect(clippy::inline_always)]

#[cfg(all(target_os = "linux", feature = "perf"))]
mod imp {
    /// Disables the active process-global perf measurement started by
    /// [`perf_enable!`](crate::perf_enable!).
    ///
    /// The token must come from the matching `perf_enable!` call. Only one measurement section may
    /// be active per process; nested or overlapping sections panic instead of causing undefined
    /// behavior.
    ///
    /// # Panics
    ///
    /// - When writing to the control file fails.
    /// - When the acknowledgement frame from perf is missing or malformed.
    #[macro_export]
    macro_rules! perf_disable {
        ($control_token:expr) => {{
            let mut token = $control_token;
            if let ::core::result::Result::Err(error) = ::std::io::Write::write_all(
                &mut $crate::perf::PerfControl::control_file(&token),
                b"disable\n",
            ) {
                $crate::perf::PerfControl::control_write_failed(&error);
            }
            $crate::perf::PerfControl::read_ack(&mut token);
            $crate::perf::PerfControl::finish(token);
        }};
    }

    /// Enables process-global perf measurement and returns an opaque token for
    /// [`perf_disable!`](crate::perf_disable!).
    ///
    /// This macro controls a single process-global perf control channel and is safe to call from
    /// any thread. Only one measurement section may be active per process; nested or overlapping
    /// sections panic instead of causing undefined behavior. The returned opaque, `#[must_use]`
    /// token must be passed to exactly one matching `perf_disable!` call.
    ///
    /// # Panics
    ///
    /// - When another perf section is active in any thread (nested or overlapping sections are
    ///   unsupported).
    /// - When the perf control state was poisoned by a previous panic.
    /// - When writing to the control file fails.
    /// - When the acknowledgement frame from perf is missing or malformed.
    #[macro_export]
    macro_rules! perf_enable {
        () => {
            $crate::perf_enable!($crate::perf::acquire_control())
        };
        // The unit tests use the same sequence with an independently acquired guard.
        ($control_token:expr) => {{
            let mut token = $control_token;
            if let ::core::result::Result::Err(error) = ::std::io::Write::write_all(
                &mut $crate::perf::PerfControl::control_file(&token),
                b"enable\n",
            ) {
                $crate::perf::PerfControl::control_write_failed(&error);
            }
            $crate::perf::PerfControl::read_ack(&mut token);
            $crate::perf::PerfControl::mark_enabled(&mut token);
            token
        }};
    }

    /// Writes a message to the perf side-channel log used by the runner.
    ///
    /// When the `perf` feature is enabled on Linux, the message is written to the inherited
    /// perf log file descriptor.
    #[macro_export]
    macro_rules! perf_log {
        ($($arg:tt)*) => {{
            $crate::perf::log(&format!("{}", format_args!($($arg)*)));
        }};
    }

    use std::fmt::{self, Debug};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    use std::sync::{Mutex, MutexGuard, OnceLock, TryLockError};

    use crate::__internal::{PERF_ACK_FD_READ, PERF_CTL_FD_WRITE, PERF_LOG_FD};

    /// Lazily initialized process-global perf control state.
    static PERF_CONTROL: OnceLock<Mutex<PerfControlState>> = OnceLock::new();
    /// Synchronizes writes to the inherited perf log file descriptor.
    static PERF_LOG_LOCK: OnceLock<Mutex<Option<File>>> = OnceLock::new();

    /// Holds exclusive access to the active perf section until consumed by `perf_disable!`.
    #[must_use = "the perf section stays enabled until perf_disable! consumes this token"]
    pub struct PerfControl<'a>(MutexGuard<'a, PerfControlState>);

    /// Process-global perf control state for the inherited control and acknowledgement fds.
    pub(super) struct PerfControlState {
        /// File descriptor used to receive acknowledgement bytes from perf.
        ack: File,
        /// File descriptor used to send enable and disable commands to perf.
        control: File,
        /// Tracks whether a perf section is currently active for this process.
        enabled: bool,
    }

    impl PerfControl<'_> {
        #[doc(hidden)]
        #[inline(always)]
        pub fn control_file(&self) -> &File {
            &self.0.control
        }

        #[doc(hidden)]
        #[cold]
        pub fn control_write_failed(error: &std::io::Error) -> ! {
            panic!("gungraun: failed writing to control file: {error}");
        }

        #[doc(hidden)]
        #[inline]
        pub fn finish(mut self) {
            assert!(
                self.0.enabled,
                "gungraun: perf_disable! called without a matching perf_enable!"
            );
            self.0.enabled = false;
        }

        #[doc(hidden)]
        #[inline(always)]
        pub fn mark_enabled(&mut self) {
            self.0.enabled = true;
        }

        #[doc(hidden)]
        #[inline(always)]
        pub fn read_ack(&mut self) {
            read_ack(&mut self.0.ack);
        }
    }

    impl Debug for PerfControl<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PerfControl").finish_non_exhaustive()
        }
    }

    impl PerfControlState {
        pub(super) fn new(control: File, ack: File) -> Self {
            Self {
                ack,
                control,
                enabled: false,
            }
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn acquire_control() -> PerfControl<'static> {
        try_acquire(PERF_CONTROL.get_or_init(open_control))
    }

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

    fn open_control() -> Mutex<PerfControlState> {
        // SAFETY: The ack fd is provided by the runner and converted into a `File` exactly
        // once when the control state is initialized, giving this process unique ownership.
        let ack = unsafe { File::from_raw_fd(PERF_ACK_FD_READ) };
        // SAFETY: The same unique ownership guarantee applies to the control fd.
        let control = unsafe { File::from_raw_fd(PERF_CTL_FD_WRITE) };
        Mutex::new(PerfControlState::new(control, ack))
    }

    fn open_log() -> Mutex<Option<File>> {
        // SAFETY: The perf log fd is provided by the runner and converted into a `File` exactly
        // once when the log lock is first initialized, giving this process unique ownership.
        let log_file = unsafe { File::from_raw_fd(PERF_LOG_FD) };
        Mutex::new(Some(log_file))
    }

    /// Reads and validates one perf control acknowledgement frame.
    ///
    /// The Linux `perf` control protocol documents acknowledgements as the line `ack\n`. In
    /// practice, the kernel implementation in `tools/perf/util/evlist.c` writes the acknowledgement
    /// using `sizeof("ack\n")`, which includes the C string literal's null terminator. This means
    /// the wire format is actually `ack\n\0` (five bytes).
    ///
    /// This reader skips any leading NUL padding, collects bytes until the line terminator, and
    /// validates that the frame is exactly `ack`. Accepting zero or more leading NULs keeps the
    /// reader aligned whether `perf` emits the trailing NUL, fixes the bug, or adds other padding.
    #[doc(hidden)]
    pub fn read_ack(ack: &mut File) {
        const PREFIX: &str = "gungraun: invalid acknowledgement from perf control channel";

        let mut frame = [0_u8; 8];
        let mut len = 0;
        loop {
            let mut byte = [0_u8; 1];
            if let Err(error) = ack.read_exact(&mut byte) {
                panic!("{PREFIX}: {error}");
            }
            let [byte] = byte;
            if byte == 0 && len == 0 {
                continue;
            }
            if byte == b'\n' {
                assert!(
                    &frame[..len] == b"ack",
                    "{PREFIX}: expected acknowledgement frame `ack\\n`"
                );
                return;
            }
            assert!(
                len < frame.len(),
                "{PREFIX}: frame exceeded 8 bytes; first 8 bytes are '{frame:02x?}'"
            );
            assert!(
                b"ack".get(len) == Some(&byte),
                "{PREFIX}: unexpected byte {byte:#04x} at offset {len}"
            );
            frame[len] = byte;
            len += 1;
        }
    }

    pub(super) fn try_acquire(mutex: &Mutex<PerfControlState>) -> PerfControl<'_> {
        let guard = match mutex.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => {
                panic!("gungraun: nested or overlapping perf sections are unsupported");
            }
            Err(TryLockError::Poisoned(_)) => {
                panic!("gungraun: perf control state poisoned by a previous panic");
            }
        };
        assert!(
            !guard.enabled,
            "gungraun: nested or overlapping perf sections are unsupported"
        );
        PerfControl(guard)
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
            $crate::perf::PerfControl::stub()
        };
    }

    /// No-op disable macro when perf feature is disabled.
    #[macro_export]
    macro_rules! perf_disable {
        ($control_token:expr) => {{ $crate::perf::PerfControl::finish($control_token) }};
    }

    use std::fmt::{self, Debug};

    /// No-op token when perf is disabled.
    #[must_use = "the perf section stays enabled until perf_disable! consumes this token"]
    pub struct PerfControl(());

    impl Debug for PerfControl {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PerfControl").finish_non_exhaustive()
        }
    }

    impl PerfControl {
        #[doc(hidden)]
        #[inline]
        pub fn finish(self) {}

        #[doc(hidden)]
        #[inline]
        pub fn stub() -> Self {
            Self(())
        }
    }

    /// Writes a single message line to the perf log file.
    #[inline(always)]
    pub fn log(_message: &str) {}
}

#[doc(hidden)]
pub use imp::PerfControl;
#[cfg(all(target_os = "linux", feature = "perf"))]
#[doc(hidden)]
pub use imp::acquire_control;
pub use imp::log;
#[cfg(all(target_os = "linux", feature = "perf"))]
#[doc(hidden)]
pub use imp::read_ack;

#[cfg(all(target_os = "linux", feature = "perf"))]
#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::fs::File;
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Mutex, mpsc};
    use std::thread::{self, JoinHandle};

    use serial_test::serial;

    use super::PerfControl;
    use super::imp::{PerfControlState, read_ack, try_acquire};

    struct MockControl {
        state: Option<Mutex<PerfControlState>>,
        worker: Option<JoinHandle<Vec<Vec<u8>>>>,
    }

    impl MockControl {
        fn new() -> Self {
            let (control, reader) = UnixStream::pair().unwrap();
            let (ack, mut writer) = UnixStream::pair().unwrap();
            let state = Mutex::new(PerfControlState::new(
                File::from(OwnedFd::from(control)),
                File::from(OwnedFd::from(ack)),
            ));
            let worker = thread::spawn(move || {
                let mut reader = BufReader::new(reader);
                let mut commands = Vec::new();
                let mut command = Vec::new();
                while reader.read_until(b'\n', &mut command).unwrap() != 0 {
                    assert!(command == b"enable\n" || command == b"disable\n");
                    // The kernel writes `sizeof("ack\n")` bytes, including the NUL terminator.
                    writer.write_all(b"ack\n\0").unwrap();
                    commands.push(std::mem::take(&mut command));
                }
                commands
            });
            Self {
                state: Some(state),
                worker: Some(worker),
            }
        }

        fn enable(&self) -> PerfControl<'_> {
            crate::perf_enable!(try_acquire(self.state.as_ref().unwrap()))
        }

        fn finish(mut self) -> Vec<Vec<u8>> {
            self.join().unwrap()
        }

        fn join(&mut self) -> Option<Vec<Vec<u8>>> {
            // Close the control endpoint so the worker sees EOF before joining it.
            drop(self.state.take());
            self.worker.take().map(|worker| worker.join().unwrap())
        }
    }

    impl Drop for MockControl {
        fn drop(&mut self) {
            self.join();
        }
    }

    fn assert_panic_message(panic: &(dyn Any + Send), expected: &str) {
        let message = match panic.downcast_ref::<&str>() {
            Some(message) => *message,
            None => panic.downcast_ref::<String>().unwrap().as_str(),
        };
        assert!(message.contains(expected), "unexpected panic: {message}");
    }

    #[test]
    #[should_panic(expected = "perf_disable! called without a matching perf_enable!")]
    fn perf_disable_when_no_matching_enable_then_panics() {
        let mock = MockControl::new();
        crate::perf_disable!(try_acquire(mock.state.as_ref().unwrap()));
    }

    #[test]
    fn perf_enable_when_nested_then_panics() {
        let mock = MockControl::new();
        let token = mock.enable();

        let panic = catch_unwind(AssertUnwindSafe(|| mock.enable())).unwrap_err();

        crate::perf_disable!(token);
        drop(mock);
        assert_panic_message(
            panic.as_ref(),
            "gungraun: nested or overlapping perf sections are unsupported",
        );
    }

    #[test]
    fn perf_enable_when_overlapping_threads_then_panics() {
        // Regression for #778: the second thread must fail while the first owns its section.
        let mock = MockControl::new();
        let panic = thread::scope(|scope| {
            let (ready_tx, ready_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let mock = &mock;
            let holder = scope.spawn(move || {
                let token = mock.enable();
                ready_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                crate::perf_disable!(token);
            });
            ready_rx.recv().unwrap();

            let result = catch_unwind(AssertUnwindSafe(|| mock.enable()));
            release_tx.send(()).unwrap();
            holder.join().unwrap();
            result.unwrap_err()
        });

        drop(mock);
        assert_panic_message(
            panic.as_ref(),
            "gungraun: nested or overlapping perf sections are unsupported",
        );
    }

    #[test]
    fn perf_enable_when_poisoned_then_panics() {
        let mock = MockControl::new();
        let holder_panic = thread::scope(|scope| {
            let holder = scope.spawn(|| {
                let _token = mock.enable();
                panic!("test: panic while holding perf token");
            });
            holder.join().unwrap_err()
        });
        assert_panic_message(
            holder_panic.as_ref(),
            "test: panic while holding perf token",
        );

        let panic = catch_unwind(AssertUnwindSafe(|| mock.enable())).unwrap_err();

        drop(mock);
        assert_panic_message(
            panic.as_ref(),
            "gungraun: perf control state poisoned by a previous panic",
        );
    }

    #[test]
    fn perf_enable_when_token_dropped_then_panics() {
        let mock = MockControl::new();
        drop(mock.enable());

        let panic = catch_unwind(AssertUnwindSafe(|| mock.enable())).unwrap_err();

        drop(mock);
        assert_panic_message(
            panic.as_ref(),
            "gungraun: nested or overlapping perf sections are unsupported",
        );
    }

    #[test]
    fn perf_sections_send_enable_and_disable_in_order() {
        let mock = MockControl::new();
        for _ in 0..2 {
            crate::perf_disable!(mock.enable());
        }
        assert_eq!(
            mock.finish(),
            [
                b"enable\n".as_slice(),
                b"disable\n",
                b"enable\n",
                b"disable\n"
            ]
        );
    }

    #[test]
    #[should_panic(expected = "gungraun: invalid acknowledgement from perf control channel")]
    fn read_ack_when_eof_then_panics() {
        let (peer, ack) = UnixStream::pair().unwrap();
        drop(peer);
        read_ack(&mut File::from(OwnedFd::from(ack)));
    }

    #[test]
    #[should_panic(expected = "gungraun: invalid acknowledgement from perf control channel")]
    fn read_ack_when_malformed_then_panics() {
        let (mut peer, ack) = UnixStream::pair().unwrap();
        peer.write_all(b"bad\n").unwrap();
        drop(peer);
        read_ack(&mut File::from(OwnedFd::from(ack)));
    }

    #[test]
    fn read_ack_when_nul_padding_then_accepts_next_frame() {
        let (mut peer, ack) = UnixStream::pair().unwrap();
        peer.write_all(b"ack\n\0\0ack\n").unwrap();
        drop(peer);
        let mut ack = File::from(OwnedFd::from(ack));
        read_ack(&mut ack);
        read_ack(&mut ack);
        assert_eq!(ack.read(&mut [0_u8; 1]).unwrap(), 0);
    }

    #[test]
    fn read_ack_when_valid_then_consumes_complete_frame() {
        let (mut peer, ack) = UnixStream::pair().unwrap();
        peer.write_all(b"ack\n\0").unwrap();
        drop(peer);
        let mut ack = File::from(OwnedFd::from(ack));
        read_ack(&mut ack);
        let mut residue = [0_u8; 1];
        ack.read_exact(&mut residue).unwrap();
        assert_eq!(&residue, b"\0");
    }

    #[test]
    #[serial]
    fn perf_enable_disable_repeated_then_succeeds() {
        let mock = MockControl::new();
        for _ in 0..8 {
            crate::perf_disable!(mock.enable());
        }
        let expected: Vec<Vec<u8>> = (0..8)
            .flat_map(|_| [b"enable\n".to_vec(), b"disable\n".to_vec()])
            .collect();
        assert_eq!(mock.finish(), expected);
    }
}
