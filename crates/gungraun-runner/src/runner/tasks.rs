//! Organize tasks within thread pools and processes

use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};
use std::{iter, thread};

use anyhow::{Context, Result, anyhow, bail};
use crossbeam::deque::{Injector, Steal, Stealer, Worker};
use derive_more::Deref;
use log::debug;
use nix::sys::signal;
use nix::unistd::Pid;
use parking_lot::{Condvar, Mutex};

use super::common::AssistantKind;
use crate::api::BenchRunMode;
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::common::{Assistant, CapturedOutput, Config, ModulePath};
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::{RunOptions, ToolCommand, ToolCommandChild, check_exit};

type Channel<T> = (Sender<(JobId, T)>, Receiver<(JobId, T)>);
type Job<T> = (JobId, JobClosure<T>);
type JobClosure<T> = Box<dyn FnOnce(Arc<AtomicBool>) -> T + Send + 'static>;
type JobId = usize;
type TaskHandle = JoinHandle<Result<()>>;

#[derive(Debug, Clone, Copy)]
enum ProcessState {
    Running,
    Term,
    Kill,
}

/// The wrapper for a [`std::process::Child`] of the setup/teardown or benchmark process
#[derive(Debug, Deref)]
pub struct ProcessChild(pub Child);

/// This struct is used to start and terminate processes related to the execution of a benchmark
///
/// It manages the setup, benchmark execution, and teardown of those processes, providing options
/// for a forced shutdown and handling of parallel setup tasks. The main purpose however is to be
/// able to shutdown processes in a given order and as nicely as possible.
///
/// # Forced Shutdown
///
/// A forced shutdown can be initiated by setting the `force_shutdown` variable. Once this variable
/// is set to `true` it should not be changed back to `false`. This case is not handled properly!
///
/// In order to avoid process zombie processes and if there are processes running then in a first
/// step the `SIGTERM` signal is sent to them. We rely on valgrind to pass this signal to the actual
/// benchmark process. In the case the processes are not shutting down gracefully we call
/// [`std::process::Child::kill`] and end the processes forcefully. In any case we return with a
/// [`Error::TaskInterrupt`].
#[derive(Debug)]
pub struct ProcessHandler {
    /// An optional child process running a benchmark
    pub bench: Option<ToolCommandChild>,
    /// A flag indicating if a forced shutdown is requested.
    pub force_shutdown: Arc<AtomicBool>,
    /// The path to the module in the gungraun benchmark file that this handler is associated with.
    pub module_path: ModulePath,
    /// The time interval to poll for process status updates
    pub poll_interval: Duration,
    /// An optional directory that acts as a sandbox for process execution.
    pub sandbox_dir: Option<PathBuf>,
    /// An optional tuple that holds the setup process
    pub setup: Option<(String, Child)>,
    /// A boolean indicating whether the setup process should be run in parallel to benchmark
    pub setup_is_parallel: bool,
    /// An optional tuple that holds the teardown process
    pub teardown: Option<(String, Child)>,
}

#[derive(Debug)]
struct Task {
    thread: Option<TaskHandle>,
}

/// A work-stealing thread pool that executes jobs and returns results in insertion order.
///
/// This thread pool uses a work-stealing deque implementation for efficient load balancing across
/// worker threads. Jobs are submitted via [`ThreadPool::execute`] and results are retrieved by
/// iterating over the pool, which yields results in the same order jobs were submitted (FIFO
/// ordering).
///
/// The pool supports cooperative cancellation through a shared `force_shutdown` flag that is
/// passed to each job. Long-running jobs can periodically check this flag and terminate early when
/// shutdown is requested.
///
/// # Concurrency Model
///
/// - **Work stealing**: Idle workers steal tasks from busy workers' queues
/// - **Insertion-order results**: Despite parallel execution, results arrive in submission order
/// - **Graceful shutdown**: Workers finish current jobs before exiting
/// - **Force shutdown**: Workers can be interrupted mid-job via the shared flag
///
/// # Thread Safety
///
/// The pool is `Send` and can be safely shared across threads when wrapped in `Arc<Mutex<>>`.
/// Shared state is coordinated with atomics, message passing, and a condition variable for parking
/// idle workers.
///
/// # Examples
///
/// Basic usage with successful and failing jobs:
///
/// ```
/// use anyhow::bail;
/// use gungraun_runner::runner::tasks::ThreadPool;
///
/// let mut pool: ThreadPool<Result<usize, anyhow::Error>> = ThreadPool::new(4)?;
///
/// for i in 0..10 {
///     pool.execute(move |force_shutdown| {
///         // Simulate work that checks for shutdown
///         if force_shutdown.load(std::sync::atomic::Ordering::Acquire) {
///             bail!("Interrupted");
///         }
///         Ok(i * 2)
///     });
/// }
///
/// // Results arrive in insertion order
/// for (i, result) in pool.enumerate() {
///     assert_eq!(result?, i * 2);
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Using with benchmark execution:
///
/// ```
/// # fn run_benchmark(a: usize, _force_shutdown: &Arc<AtomicBool>)
/// # -> Result<usize, anyhow::Error> {
/// #   Ok(a)
/// # }
/// # fn process_summary(_s: usize) {}
/// # let benchmarks = [1, 2];
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicBool, Ordering};
///
/// use gungraun_runner::runner::tasks::ThreadPool;
///
/// let mut pool = ThreadPool::new(4)?;
///
/// for bench in benchmarks {
///     pool.execute(move |force_shutdown| {
///         // Run benchmark, checking force_shutdown periodically
///         run_benchmark(bench, &force_shutdown)
///     });
/// }
///
/// let force_shutdown = pool.clone_force_shutdown();
/// // Collect results in submission order
/// for result in pool {
///     match result {
///         Ok(summary) => process_summary(summary),
///         Err(e) => {
///             // If one thread returns with error we initiate the shutdown process for all other
///             // threads by setting the `force_shutdown` flag to `true`.
///             force_shutdown.store(true, Ordering::Release);
///             return Err(e);
///         }
///     }
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// [`ThreadPool::new`] returns an error if `size` is less than 1.
pub struct ThreadPool<T: Send + 'static> {
    force_shutdown: Arc<AtomicBool>,
    job_queue: Arc<Injector<Job<T>>>,
    next: Option<JobId>,
    num_received: usize,
    result_receiver: Receiver<(JobId, T)>,
    results: HashMap<usize, T>,
    state: Arc<(Mutex<ThreadPoolState>, Condvar)>,
    tasks: Vec<Task>,
    total_jobs: Option<usize>,
}

#[derive(Debug, Default, Clone, Copy)]
struct ThreadPoolState {
    /// Number of jobs that have been submitted but have not finished sending their result.
    pending_jobs: usize,
    /// Number of jobs submitted to the queue that workers have not started yet.
    queued_jobs: usize,
    /// Whether workers may exit once pending work is done.
    shutdown: bool,
}

impl ProcessChild {
    /// Waits for the child process to exit while polling [`std::process::Child::try_wait`].
    ///
    /// The method consumes the wrapped [`std::process::Child`] and repeatedly polls it until the
    /// process exits. It sleeps for `poll_interval` between polls and, when `timeout` is set, sends
    /// SIGTERM after the timeout elapses and the optional `is_ready` predicate returns `true`. This
    /// gates the timeout on readiness. If `force_shutdown` becomes set, it also sends SIGTERM and
    /// returns [`Error::TaskInterrupt`] once the process stops.
    ///
    /// Shutdown follows a three-state machine: `Running` polls the child, `Term` waits up to
    /// 100 poll cycles after SIGTERM, and `Kill` sends SIGKILL via [`std::process::Child::kill`]
    /// if the process still has not exited.
    ///
    /// # Returns
    ///
    /// Returns the child's [`Output`] on normal exit.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Waiting with [`std::process::Child::try_wait`] fails
    /// - Signal delivery fails
    /// - [`std::process::Child::kill`] fails
    /// - The process is interrupted by `force_shutdown` ([`Error::TaskInterrupt`])
    ///
    /// [`Error::TaskInterrupt`]: crate::error::Error::TaskInterrupt
    pub fn wait<F>(
        self,
        force_shutdown: &Arc<AtomicBool>,
        poll_interval: Duration,
        timeout: Option<Duration>,
        is_ready: Option<&F>,
    ) -> Result<Output>
    where
        F: Fn() -> bool,
    {
        let send_sigterm = |id: u32| -> Result<()> {
            let pid_t = i32::try_from(id)?;
            let pid = Pid::from_raw(pid_t);
            signal::kill(pid, signal::SIGTERM)?;
            Ok(())
        };

        let mut run_state = ProcessState::Running;
        // This should be enough time for a proper shutdown of any benchmark process
        let mut ticks = 100;
        let mut child = self.0;
        let mut interrupted = false;
        let start = Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(_)) if interrupted => {
                    break Err(Error::TaskInterrupt.into());
                }
                Ok(Some(_)) => {
                    break Ok(child
                        .wait_with_output()
                        .expect("The output should be present if there is an exit status"));
                }
                Ok(None) => {
                    match run_state {
                        ProcessState::Running if force_shutdown.load(atomic::Ordering::Acquire) => {
                            send_sigterm(child.id())?;

                            run_state = ProcessState::Term;
                            interrupted = true;
                        }
                        ProcessState::Running
                            if timeout.is_some_and(|t| start.elapsed() >= t)
                                && is_ready.is_none_or(|is_ready| is_ready()) =>
                        {
                            send_sigterm(child.id())?;

                            run_state = ProcessState::Term;
                        }
                        ProcessState::Running | ProcessState::Kill => {}
                        ProcessState::Term if ticks > 0 => {
                            ticks -= 1;
                        }
                        ProcessState::Term => {
                            child.kill()?;
                            run_state = ProcessState::Kill;
                        }
                    }
                    sleep(poll_interval);
                }
                Err(error) => {
                    break Err(error)
                        .with_context(|| "Trying to wait for the benchmark process to stop");
                }
            }
        }
    }
}

impl ProcessHandler {
    /// Creates a new instance of a [`ProcessHandler`]
    ///
    /// The `force_shutdown` flag can be used to indicate if a force shutdown is requested.
    /// `setup_is_parallel` indicates whether the setup process should be executed in parallel to
    /// the benchmarking processes or not. If the `sandbox_dir` is set, all processes are going to
    /// be executed within this directory. Each process is waited for to shutdown properly and we
    /// check every `poll_interval` duration if the processes have finished.
    ///
    /// More details are in the [`ProcessHandler`] documentation.
    pub fn new(
        force_shutdown: Arc<AtomicBool>,
        module_path: ModulePath,
        setup_is_parallel: bool,
        poll_interval: Duration,
        sandbox_dir: Option<&Path>,
    ) -> Self {
        Self {
            bench: None,
            force_shutdown,
            module_path,
            setup: None,
            setup_is_parallel,
            teardown: None,
            poll_interval,
            sandbox_dir: sandbox_dir.map(Path::to_path_buf),
        }
    }

    /// Starts the [`Assistant`] process for either setup or teardown.
    ///
    /// `force_parallel` is a flag to indicate if the assistant should run in parallel to the
    /// benchmark process even if not configured in the assistant itself. The optional
    /// [`CapturedOutput`] contains file streams for the terminal output. Configure whether output
    /// should be captured with [`NoCapture`]. Note that the output is always captured if
    /// `captured_output` is present. However, depending on the [`NoCapture`] value the captured
    /// output is printed to stdout in the post processing of the benchmark data.
    pub fn start_assistant(
        &mut self,
        force_parallel: bool,
        assistant: &Assistant,
        config: &Config,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
        nocapture: NoCapture,
    ) -> Result<()> {
        if self.force_shutdown.load(atomic::Ordering::Acquire) {
            return Err(Error::TaskInterrupt.into());
        }

        match assistant.kind() {
            AssistantKind::Setup => {
                let child = assistant.run(
                    config,
                    module_path,
                    captured_output,
                    force_parallel,
                    self.sandbox_dir.as_deref(),
                    nocapture,
                )?;
                self.setup_is_parallel = assistant.is_parallel();
                self.setup = child.map(|c| (assistant.kind().id(), c));
            }
            AssistantKind::Teardown => {
                let child = assistant.run(
                    config,
                    module_path,
                    captured_output,
                    force_parallel,
                    self.sandbox_dir.as_deref(),
                    nocapture,
                )?;
                self.teardown = child.map(|c| (assistant.kind().id(), c));
            }
        }

        Ok(())
    }

    /// Starts the benchmark process with the [`ToolCommand`] for the `executable`
    ///
    /// If the `setup`, started with [`Self::start_assistant`] is not configured to run in parallel,
    /// then this method first waits for the setup to finish before it tries to start the benchmark.
    ///
    /// # Errors
    ///
    /// This method returns with an [`Error::TaskInterrupt`] if either the setup or benchmark
    /// process were asked to shutdown by setting [`Self::force_shutdown`] to `true`.
    ///
    /// Other notable errors are [`Error::LaunchError`] and [`Error::ProcessError`]. These are
    /// returned if either launching the benchmarked binary/library with the [`ToolCommand`] failed
    /// due to an os error or valgrind, the binary/library itself returned with an error.
    pub fn start_bench<'args, F>(
        &mut self,
        command: ToolCommand,
        tool_config: &ToolConfig,
        executable_args: &F,
        run_options: &RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
        tool_runner_dest: Option<&Path>,
    ) -> Result<()>
    where
        F: Fn(&ToolConfig, Option<BenchRunMode>) -> Cow<'args, [OsString]>,
    {
        if !self.setup_is_parallel
            && let Some(Err(error)) = self.wait_for_setup()
        {
            return Err(error);
        }

        if self.force_shutdown.load(atomic::Ordering::Acquire) {
            return Err(Error::TaskInterrupt.into());
        }

        debug!("Spawning {} command", tool_config.tool());

        let child = command.run(
            tool_config,
            executable_args,
            run_options,
            output_path,
            module_path,
            self.setup.as_mut().map(|(_, c)| c),
            captured_output,
            self.sandbox_dir.as_deref(),
            tool_runner_dest,
        )?;

        self.bench = Some(child);

        Ok(())
    }

    /// Waits for the benchmark process to finish or stops waiting when shutdown is requested.
    ///
    /// The method consumes the currently running benchmark child process and waits for completion
    /// while periodically checking the shared `force_shutdown` flag. If shutdown is requested, the
    /// benchmark process is sent SIGTERM, followed by SIGKILL if it doesn't terminate gracefully.
    ///
    /// If `timeout` is set, the benchmark process is stopped with SIGTERM once the timeout has
    /// elapsed and the optional `is_ready` predicate returns `true`.
    ///
    /// After the benchmark process exits, the exit status is validated against the configured
    /// expectations in [`ExitWith`] if present. Finally, if a setup assistant is still running,
    /// this method waits for it to complete and propagates any setup error.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(Output))` when the benchmark process exits and the exit status matches the
    ///   configured expectations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Waiting for the benchmark process fails
    /// - The benchmark exits with a status that does not match the configured expectations
    /// - Setup completion fails
    /// - The process is interrupted by shutdown ([`Error::TaskInterrupt`])
    ///
    /// # Panics
    ///
    /// Panics if called before a benchmark child process has been started.
    ///
    /// [`ExitWith`]: crate::api::ExitWith
    /// [`Error::TaskInterrupt`]: crate::error::Error::TaskInterrupt
    pub fn wait_or_shutdown<F>(
        &mut self,
        timeout: Option<Duration>,
        is_ready: &Option<F>,
    ) -> Result<Output>
    where
        F: Fn() -> bool,
    {
        let mut bench_child = self
            .bench
            .take()
            .expect("A benchmark should be started before waiting");

        debug!("Waiting for the {} child process", bench_child.tool);
        let child = bench_child
            .child
            .take()
            .expect("A child process should be present");

        let result = ProcessChild(child)
            .wait(
                &self.force_shutdown,
                self.poll_interval,
                timeout,
                is_ready.as_ref(),
            )
            .with_context(|| "Trying to wait for the benchmark process to stop")
            .and_then(|output| {
                check_exit(
                    bench_child.tool,
                    &bench_child.executable,
                    output,
                    &bench_child.output_path.to_log_output(),
                    bench_child.exit_with.as_ref(),
                )
            });

        if let Some(Err(error)) = self.wait_for_setup() {
            return Err(error);
        }

        result
    }

    fn wait_for_assistant(&self, child: Child, id: &str) -> Result<()> {
        ProcessChild(child)
            .wait(
                &self.force_shutdown,
                self.poll_interval,
                None,
                None::<&fn() -> bool>,
            )
            .with_context(|| format!("Trying to wait for the {id} process to stop"))
            .and_then(|output| {
                let status = output.status;
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::new_process_error(
                        self.module_path.join(id).to_string(),
                        output,
                        None,
                    )
                    .into())
                }
            })
    }

    /// Waits for the setup assistant process
    ///
    /// This consumes the stored setup child process and returns `None` when no setup process is
    /// active.
    ///
    /// # Errors
    ///
    /// Returns an error when waiting for setup fails or setup exits unsuccessfully.
    pub fn wait_for_setup(&mut self) -> Option<Result<()>> {
        self.setup.take().map(|(id, child)| {
            debug!("Waiting for setup to complete");
            self.wait_for_assistant(child, &id)
        })
    }

    /// Waits for the teardown assistant
    ///
    /// This consumes the stored teardown child process and returns `None` when no teardown process
    /// is active.
    ///
    /// # Errors
    ///
    /// Returns an error when waiting for teardown fails or teardown exits unsuccessfully.
    pub fn wait_for_teardown(&mut self) -> Option<Result<()>> {
        self.teardown.take().map(|(id, child)| {
            debug!("Waiting for teardown to complete");
            self.wait_for_assistant(child, &id)
        })
    }
}

impl Task {
    fn new(thread: TaskHandle) -> Self {
        Self {
            thread: Some(thread),
        }
    }
}

impl<T: Send + 'static> ThreadPool<T> {
    /// Creates a new work-stealing thread pool with insertion-order result delivery.
    ///
    /// The `size` parameter sets the number of worker threads. This thread pool results are
    /// expected to be collected with an iterator [`Iterator::next`]
    ///
    /// # Errors
    ///
    /// Returns an error when `size` is less than 1.
    pub fn new(size: usize) -> Result<Self> {
        if size < 1 {
            bail!("Minimum size for a thread pool is 1 but was: '{size}'");
        }

        let (result_sender, result_receiver): Channel<T> = mpsc::channel();

        let force_shutdown = Arc::new(AtomicBool::new(false));
        let injector = Arc::new(Injector::<Job<T>>::new());

        let mut local_queues = VecDeque::<Worker<Job<T>>>::with_capacity(size);
        let mut stealers = Vec::<Stealer<Job<T>>>::with_capacity(size);
        let state = Arc::new((Mutex::new(ThreadPoolState::default()), Condvar::new()));

        for _ in 0..size {
            let queue = Worker::<Job<T>>::new_fifo();
            stealers.push(queue.stealer());
            local_queues.push_back(queue);
        }

        let mut tasks = Vec::with_capacity(size);
        for _ in 0..size {
            let result_sender = result_sender.clone();
            let global_queue = Arc::clone(&injector);

            // This unwrap is safe since this loop and the one to fill the local queue iterate over
            // the same amount of elements
            let local_queue = local_queues.pop_front().unwrap();
            let local_stealers = stealers.clone();
            let force_shutdown = Arc::clone(&force_shutdown);
            let state = Arc::clone(&state);

            let thread: TaskHandle = thread::spawn(move || {
                loop {
                    if force_shutdown.load(atomic::Ordering::Acquire) {
                        break;
                    }

                    let job = local_queue.pop().or_else(|| {
                        iter::repeat_with(|| {
                            global_queue
                                .steal_batch_and_pop(&local_queue)
                                .or_else(|| local_stealers.iter().map(Stealer::steal).collect())
                        })
                        .find(|s| !s.is_retry())
                        .and_then(Steal::success)
                    });

                    if let Some((id, job)) = job {
                        {
                            let (lock, _) = &*state;
                            let mut thread_state = lock.lock();

                            debug_assert!(thread_state.queued_jobs > 0);
                            thread_state.queued_jobs -= 1;
                        }

                        let force_shutdown = Arc::clone(&force_shutdown);
                        let result = job(force_shutdown);

                        let send_result = result_sender
                            .send((id, result))
                            .map_err(|error| anyhow!("{error}"));

                        let (lock, condvar) = &*state;
                        let mut thread_state = lock.lock();

                        debug_assert!(thread_state.pending_jobs > 0);
                        // Decrement and notify before propagating a send error so shutdown can
                        // still observe that this job finished.
                        thread_state.pending_jobs -= 1;
                        condvar.notify_all();

                        send_result?;
                    } else {
                        let (lock, condvar) = &*state;
                        let mut thread_state = lock.lock();
                        if thread_state.shutdown && thread_state.pending_jobs == 0 {
                            break;
                        }
                        if thread_state.queued_jobs > 0 {
                            continue;
                        }

                        condvar.wait(&mut thread_state);
                    }
                }

                Ok(())
            });

            tasks.push(Task::new(thread));
        }

        Ok(Self {
            tasks,
            result_receiver,
            force_shutdown,
            job_queue: injector,
            results: HashMap::new(),
            next: None,
            total_jobs: None,
            num_received: 0,
            state,
        })
    }

    /// Returns a clone of the shared force-shutdown flag.
    pub fn clone_force_shutdown(&self) -> Arc<AtomicBool> {
        self.force_shutdown.clone()
    }

    /// Enqueues a job for execution in the thread pool.
    ///
    /// The job receives the shared force-shutdown flag so long-running tasks can cooperatively
    /// terminate early.
    pub fn execute<F>(&mut self, job: F)
    where
        F: FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
    {
        // Mark the job as pending before making it visible in the queue. Otherwise a worker could
        // steal and finish it before the counter is incremented, leaving shutdown waiting forever.
        let (lock, condvar) = &*self.state;
        let mut state = lock.lock();
        assert!(
            !state.shutdown,
            "cannot submit a job after thread pool shutdown has started"
        );

        state.pending_jobs += 1;
        state.queued_jobs += 1;

        let num_jobs = self.total_jobs.get_or_insert(0);
        self.job_queue.push((*num_jobs, Box::new(job)));
        *num_jobs += 1;

        // Notify after pushing so a woken worker can observe the queued job.
        condvar.notify_one();
    }

    /// Gracefully shuts down all worker threads and waits for them to finish.
    pub fn shutdown(&mut self) {
        // Drop the state lock before joining. Workers need this lock to observe shutdown,
        // decrement pending jobs, and exit.
        {
            let (lock, condvar) = &*self.state;
            let mut state = lock.lock();
            state.shutdown = true;
            condvar.notify_all();
        }

        for task in &mut self.tasks {
            if let Some(thread) = task.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl<T: Send + 'static> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl<T: Send + 'static> Iterator for ThreadPool<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.next == self.total_jobs {
                break None;
            }
            if self.total_jobs.is_some_and(|c| c == self.num_received) {
                if let Some(next) = self.next.as_mut() {
                    let result = self.results.remove(next);
                    *next += 1;
                    break result;
                }

                break None;
            }
            match self.result_receiver.recv() {
                Ok((index, result)) => {
                    let next = self.next.get_or_insert(0);
                    self.num_received += 1;

                    if index == *next {
                        *next += 1;
                        break Some(result);
                    }
                    if let Some(r) = self.results.remove(next) {
                        self.results.insert(index, result);
                        *next += 1;
                        break Some(r);
                    }

                    self.results.insert(index, result);
                }
                _ => {
                    break None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    use rstest::rstest;

    use super::*;
    use crate::api::LibraryBenchmarkConfig;
    use crate::fixtures::metadata_f;
    use crate::runner::common::ModulePath;
    use crate::runner::lib_bench::{self, LibBench};

    const DEFAULT_TARGET: &str = "x86_64-unknown-linux-gnu";

    #[test]
    fn test_process_child_wait_with_timeout_waits_for_readiness() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("perf.out");

        let delayed_output_path = output_path.clone();

        let start = Instant::now();
        let output_thread = thread::spawn(move || {
            sleep(Duration::from_millis(500));
            std::fs::File::create(delayed_output_path).unwrap();
        });

        let child = Command::new("sleep").arg("1").spawn().unwrap();

        let output = ProcessChild(child)
            .wait(
                &Arc::new(AtomicBool::new(false)),
                Duration::from_millis(10),
                Some(Duration::from_millis(50)),
                Some(&|| output_path.exists()),
            )
            .unwrap();

        output_thread.join().unwrap();

        assert!(start.elapsed() >= Duration::from_millis(500));
        assert_eq!(output.status.signal(), Some(signal::SIGTERM as i32));
    }

    #[test]
    #[should_panic(expected = "cannot submit a job after thread pool shutdown has started")]
    fn test_thread_pool_execute_after_shutdown_panics() {
        let mut pool = ThreadPool::<()>::new(1).unwrap();
        pool.shutdown();
        pool.execute(|_| {});
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    fn test_thread_pool_execute_after_workers_are_idle() {
        let mut pool = ThreadPool::<usize>::new(4).unwrap();

        sleep(Duration::from_millis(50));

        pool.execute(|_| 42);

        assert_eq!(pool.next(), Some(42));
    }

    #[rstest]
    #[case::size_one_jobs_zero(1, 0)]
    #[case::equal_one(1, 1)]
    #[case::size_one_jobs_two(1, 2)]
    #[case::size_one_jobs_three(1, 3)]
    #[case::size_one_jobs_twenty(1, 20)]
    #[case::size_two_jobs_1(2, 1)]
    #[case::equal_two(2, 2)]
    #[case::size_two_jobs_3(2, 3)]
    #[case::size_two_jobs_4(2, 4)]
    #[case::size_two_jobs_20(2, 20)]
    #[case::size_19_jobs_20(19, 20)]
    #[case::equal_twenty(20, 20)]
    #[case::size_21_jobs_20(21, 20)]
    #[timeout(Duration::from_secs(1))]
    fn test_thread_pool_execute_and_next(#[case] size: usize, #[case] jobs: usize) {
        let mut pool = ThreadPool::new(size).unwrap();
        for i in 0..jobs {
            pool.execute(move |_| {
                // Simulating some work
                if i % 2 == 0 {
                    Ok(i) // Successful job
                } else {
                    Err(format!("Failed job {i}")) // Simulated failure
                }
            });
        }

        let mut expected = 0;
        for result in pool {
            match result {
                Ok(num) => {
                    assert_eq!(num, expected);
                }
                Err(error) => assert_eq!(error, format!("Failed job {expected}")),
            }

            expected += 1;
        }

        assert_eq!(expected, jobs);
    }

    #[test]
    fn test_thread_pool_next_when_no_execute() {
        let mut pool = ThreadPool::<usize>::new(4).unwrap();
        assert_eq!(pool.tasks.len(), 4);
        assert_eq!(pool.next(), None);
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    fn test_thread_pool_runs_jobs_in_parallel_after_idle_wait() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let running = Arc::new(AtomicUsize::new(0));
        let max_running = Arc::new(AtomicUsize::new(0));
        let mut pool = ThreadPool::<()>::new(2).unwrap();

        sleep(Duration::from_millis(50));

        for _ in 0..2 {
            let running = Arc::clone(&running);
            let max_running = Arc::clone(&max_running);

            pool.execute(move |_| {
                let current = running.fetch_add(1, Ordering::SeqCst) + 1;
                max_running.fetch_max(current, Ordering::SeqCst);
                sleep(Duration::from_millis(100));
                running.fetch_sub(1, Ordering::SeqCst);
            });
        }

        for () in &mut pool {}

        assert_eq!(max_running.load(Ordering::SeqCst), 2);
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    fn test_thread_pool_shutdown() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = Arc::new(AtomicUsize::new(0));

        let mut pool: ThreadPool<()> = ThreadPool::new(4).unwrap();
        for _ in 0..4 {
            let counter_clone = counter.clone();
            pool.execute(move |_| {
                counter_clone.fetch_add(1, Ordering::Relaxed);
            });
        }

        pool.shutdown();

        assert_eq!(counter.load(Ordering::Relaxed), 4);
        assert!(pool.tasks.iter().all(|t| t.thread.is_none()));
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    fn test_thread_pool_shutdown_waits_for_pending_jobs() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = Arc::new(AtomicUsize::new(0));
        let mut pool = ThreadPool::<()>::new(4).unwrap();

        for _ in 0..16 {
            let counter = Arc::clone(&counter);

            pool.execute(move |_| {
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }

        pool.shutdown();

        assert_eq!(counter.load(Ordering::Relaxed), 16);
        assert!(pool.tasks.iter().all(|task| task.thread.is_none()));
    }

    #[rstest]
    #[timeout(Duration::from_secs(2))]
    fn test_thread_pool_when_job_is_running_then_idle_workers_wait() {
        let (started_sender, started_receiver) = mpsc::channel();
        let (finish_sender, finish_receiver) = mpsc::channel();
        let mut pool = ThreadPool::<usize>::new(4).unwrap();

        pool.execute(move |_| {
            started_sender.send(()).unwrap();
            finish_receiver.recv().unwrap();
            1
        });

        started_receiver.recv().unwrap();
        sleep(Duration::from_millis(50));
        finish_sender.send(()).unwrap();

        assert_eq!(pool.next(), Some(1));
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    fn test_thread_pool_when_many_jobs_after_idle_then_all_complete() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        for _ in 0..100 {
            let counter = Arc::new(AtomicUsize::new(0));
            let mut pool = ThreadPool::<()>::new(8).unwrap();

            sleep(Duration::from_micros(100));

            for _ in 0..128 {
                let counter = Arc::clone(&counter);
                pool.execute(move |_| {
                    counter.fetch_add(1, Ordering::Relaxed);
                });
            }

            for () in &mut pool {}

            assert_eq!(counter.load(Ordering::Relaxed), 128);
        }
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    fn test_thread_pool_when_repeatedly_submitting_after_idle_then_jobs_complete() {
        for _ in 0..1_000 {
            let mut pool = ThreadPool::<usize>::new(1).unwrap();

            sleep(Duration::from_micros(100));

            pool.execute(|_| 1);

            assert_eq!(pool.next(), Some(1));
        }
    }

    #[test]
    fn test_thread_pool_when_size_is_zero() {
        assert!(ThreadPool::<usize>::new(0).is_err());
    }

    #[rstest]
    #[timeout(Duration::from_secs(2))]
    fn test_thread_pool_when_shutdown_with_blocked_workers_then_queued_jobs_complete() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = Arc::new(AtomicUsize::new(0));
        let (finish_sender, finish_receiver) = mpsc::channel();
        let finish_receiver = Arc::new(Mutex::new(finish_receiver));
        let mut pool = ThreadPool::<()>::new(2).unwrap();

        for _ in 0..8 {
            let counter = Arc::clone(&counter);
            let finish_receiver = Arc::clone(&finish_receiver);

            pool.execute(move |_| {
                finish_receiver.lock().recv().unwrap();
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }

        for _ in 0..8 {
            finish_sender.send(()).unwrap();
        }

        pool.shutdown();

        assert_eq!(counter.load(Ordering::Relaxed), 8);
    }

    #[rstest]
    #[timeout(Duration::from_secs(2))]
    fn test_thread_pool_when_submitting_after_idle_rounds_then_workers_wake() {
        let mut pool = ThreadPool::<usize>::new(2).unwrap();

        for i in 0..20 {
            sleep(Duration::from_millis(5));

            pool.execute(move |_| i);

            assert_eq!(pool.next(), Some(i));
        }
    }

    #[test]
    fn test_thread_pool_with_lib_bench() {
        let meta = metadata_f()
            .raw_command_line_args([])
            .target(DEFAULT_TARGET)
            .fx();
        let bench = lib_bench::LibBench::new(
            None,
            None,
            None,
            ModulePath::new("hello::world"),
            "function".to_owned(),
            &meta,
            LibraryBenchmarkConfig::default(),
            0,
            0,
            None,
            crate::api::Tool::Callgrind,
            "group".to_owned(),
        )
        .unwrap()
        .unwrap();

        let mut thread_pool = ThreadPool::<LibBench>::new(4).unwrap();
        thread_pool.execute(move |_| bench);

        let next = thread_pool.next();
        assert!(next.is_some());
    }
}
