//! Organize tasks within thread pools and processes

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};
use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use std::{iter, thread};

use anyhow::{anyhow, Context, Result};
use crossbeam::deque::{Injector, Steal, Stealer, Worker};
use log::debug;
use nix::sys::signal;
use nix::unistd::Pid;

use super::common::AssistantKind;
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::common::{Assistant, CapturedOutput, Config, ModulePath};
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::{check_exit, RunOptions, ToolCommand, ToolCommandChild};

type Channel<T> = (Sender<(JobId, T)>, Receiver<(JobId, T)>);
type Job<T> = (JobId, JobClosure<T>);
type JobClosure<T> = Box<dyn FnOnce(Arc<AtomicBool>) -> T + Send + 'static>;
type JobId = usize;
type TaskHandle = JoinHandle<Result<()>>;

/// The wrapper for a [`std::process::Child`] of the setup/teardown or benchmark process
#[derive(Debug)]
struct ProcessChild(Child);

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

#[derive(Debug, Clone, Copy)]
enum ProcessState {
    Running,
    Term,
    Kill,
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
/// All internal state is protected by atomic operations and message passing channels.
///
/// # Examples
///
/// Basic usage with successful and failing jobs:
///
/// ```
/// use anyhow::anyhow;
/// use gungraun_runner::runner::tasks::ThreadPool;
///
/// let mut pool: ThreadPool<Result<usize, anyhow::Error>> = ThreadPool::new(4)?;
///
/// for i in 0..10 {
///     pool.execute(move |force_shutdown| {
///         // Simulate work that checks for shutdown
///         if force_shutdown.load(std::sync::atomic::Ordering::Acquire) {
///             return Err(anyhow!("Interrupted"));
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
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::Arc;
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
pub struct ThreadPool<T> {
    force_shutdown: Arc<AtomicBool>,
    graceful_shutdown: Arc<AtomicBool>,
    job_queue: Arc<Injector<Job<T>>>,
    next: Option<JobId>,
    num_received: usize,
    result_receiver: Receiver<(JobId, T)>,
    results: HashMap<usize, T>,
    tasks: Vec<Task>,
    total_jobs: Option<usize>,
}

impl ProcessChild {
    fn wait(self, force_shutdown: &Arc<AtomicBool>, poll_interval: Duration) -> Result<Output> {
        let mut run_state = ProcessState::Running;
        // This should be enough time for a proper shutdown of any benchmark process
        let mut ticks = 100;
        let mut child = self.0;
        let mut interrupted = false;

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
                            let pid_t = i32::try_from(child.id())?;
                            let pid = Pid::from_raw(pid_t);
                            signal::kill(pid, signal::SIGTERM)?;

                            run_state = ProcessState::Term;
                            interrupted = true;
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
    pub fn start_bench(
        &mut self,
        command: ToolCommand,
        tool_config: ToolConfig,
        executable: &Path,
        executable_args: &[OsString],
        run_options: RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
    ) -> Result<()> {
        if !self.setup_is_parallel {
            if let Some(Err(error)) = self.wait_for_setup() {
                return Err(error);
            }
        }

        if self.force_shutdown.load(atomic::Ordering::Acquire) {
            return Err(Error::TaskInterrupt.into());
        }

        let child = command.run(
            tool_config,
            executable,
            executable_args,
            run_options,
            output_path,
            module_path,
            self.setup.as_mut().map(|(_, c)| c),
            captured_output,
            self.sandbox_dir.as_deref(),
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
    pub fn wait_or_shutdown(&mut self) -> Result<Output> {
        let mut bench_child = self
            .bench
            .take()
            .expect("A benchmark should be started before waiting");

        let result = ProcessChild(
            bench_child
                .child
                .take()
                .expect("A child process should be present"),
        )
        .wait(&self.force_shutdown, self.poll_interval)
        .with_context(|| "Trying to wait for the benchmark process to stop")
        .and_then(|output| {
            check_exit(
                bench_child.tool,
                &bench_child.executable,
                output,
                &bench_child.log_path,
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
            .wait(&self.force_shutdown, self.poll_interval)
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
            return Err(anyhow!(
                "Minimum size for a thread pool is 1 but was: '{size}'"
            ));
        }

        let (result_sender, result_receiver): Channel<T> = mpsc::channel();

        let force_shutdown = Arc::new(AtomicBool::new(false));
        let graceful_shutdown = Arc::new(AtomicBool::new(false));
        let injector = Arc::new(Injector::<Job<T>>::new());

        let mut local_queues = VecDeque::<Worker<Job<T>>>::with_capacity(size);
        let mut stealers = Vec::<Stealer<Job<T>>>::with_capacity(size);

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
            let graceful_shutdown = Arc::clone(&graceful_shutdown);
            let force_shutdown = Arc::clone(&force_shutdown);

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
                        let force_shutdown = Arc::clone(&force_shutdown);

                        let result = job(force_shutdown);
                        result_sender
                            .send((id, result))
                            .map_err(|error| anyhow!("{error}"))?;
                    } else if graceful_shutdown.load(atomic::Ordering::Acquire) {
                        break;
                    } else {
                        std::hint::spin_loop();
                    }
                }

                Ok(())
            });

            tasks.push(Task::new(thread));
        }

        Ok(Self {
            tasks,
            result_receiver,
            graceful_shutdown,
            force_shutdown,
            job_queue: injector,
            results: HashMap::new(),
            next: None,
            total_jobs: None,
            num_received: 0,
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
        let num_jobs = self.total_jobs.get_or_insert(0);
        self.job_queue.push((*num_jobs, Box::new(job)));
        *num_jobs += 1;
    }

    /// Gracefully shuts down all worker threads and waits for them to finish.
    pub fn shutdown(&mut self) {
        self.graceful_shutdown
            .store(true, atomic::Ordering::Release);
        for task in &mut self.tasks {
            if let Some(thread) = task.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl<T> Iterator for ThreadPool<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.next == self.total_jobs {
                break None;
            } else if self.total_jobs.is_some_and(|c| c == self.num_received) {
                if let Some(next) = self.next.as_mut() {
                    let result = self.results.remove(next);
                    *next += 1;
                    break result;
                }

                break None;
            } else if let Ok((index, result)) = self.result_receiver.recv() {
                let next = self.next.get_or_insert(0);
                self.num_received += 1;

                #[allow(clippy::else_if_without_else)]
                if index == *next {
                    *next += 1;
                    break Some(result);
                } else if let Some(r) = self.results.remove(next) {
                    self.results.insert(index, result);
                    *next += 1;
                    break Some(r);
                }

                self.results.insert(index, result);
            } else {
                break None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::rstest;

    use super::*;
    use crate::api::LibraryBenchmarkConfig;
    use crate::runner::common::ModulePath;
    use crate::runner::lib_bench::{self, LibBench};
    use crate::runner::meta::Metadata;

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

    #[test]
    fn test_thread_pool_when_size_is_zero() {
        assert!(ThreadPool::<usize>::new(0).is_err());
    }

    #[test]
    fn test_thread_pool_with_lib_bench() {
        let meta = Metadata::new(
            &[],
            "benchmark-tests",
            &PathBuf::from("test_lib_bench_intro.rs"),
        )
        .unwrap();
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
            crate::api::ValgrindTool::Callgrind,
        )
        .unwrap()
        .unwrap();

        let mut thread_pool = ThreadPool::<LibBench>::new(4).unwrap();
        thread_pool.execute(move |_| bench);

        let next = thread_pool.next();
        assert!(next.is_some());
    }
}
