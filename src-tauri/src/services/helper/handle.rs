//! The app's side of a confined helper, whatever its mode (008 research.md §8;
//! 007 research.md §11): [`HelperProcess`] starts `hoplodex <mode argument>`, runs
//! one request and its answer under a time limit by killing the helper when
//! it passes, and kills and waits for it when dropped. What is sent and read,
//! and when to start another, is the mode's (`preview::helper_handle` for
//! TIFF).

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::Child;
#[cfg(not(windows))]
use std::process::{ChildStdin, ChildStdout};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::Duration;

use super::confine;

/// A started helper: a `Child`, or on Windows the process together with the
/// job object HoploDex made for it (`job`). Both have `id`, `kill` and
/// `wait`.
#[cfg(not(windows))]
type Proc = Child;
#[cfg(windows)]
type Proc = super::job::JobChild;

/// The helper's pipes: a `Child`'s, or on Windows plain files (see
/// `job::spawn`).
#[cfg(not(windows))]
pub type Stdin = ChildStdin;
#[cfg(not(windows))]
pub type Stdout = ChildStdout;
#[cfg(windows)]
pub type Stdin = std::fs::File;
#[cfg(windows)]
pub type Stdout = std::fs::File;

/// A helper just started, with its pipes.
type Started = (Proc, Stdin, Stdout);

/// The command the app runs for a helper: `mode_argument` (such as
/// `--render-helper`), a cleared environment, its stdin and stdout piped,
/// stderr discarded, and no console window on Windows (007 research.md
/// §11). Tests add arguments to it and spawn it themselves. On Windows the
/// app does not run this command: it starts the helper into its job object
/// with `job::spawn`, so a helper started from this command is in no job of
/// HoploDex's and refuses to serve.
pub fn command(exe: &Path, mode_argument: &str) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(mode_argument)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Starts the helper the app's way (007 research.md §11). On Windows that
/// is into its own job object of `memory_limit` bytes, by `CreateProcessW`,
/// before it runs any code; elsewhere the helper sets its own limit
/// (`confine::confine`).
fn start(exe: &Path, mode_argument: &str, memory_limit: usize) -> io::Result<Started> {
    #[cfg(windows)]
    {
        super::job::spawn(exe, mode_argument, memory_limit, &[])
    }
    #[cfg(not(windows))]
    {
        let _ = memory_limit;
        let mut child = command(exe, mode_argument).spawn()?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("no pipes"));
        };
        Ok((child, stdin, stdout))
    }
}

/// One helper process and its pipes. Dropping it kills the process and waits
/// for it.
pub struct HelperProcess {
    pid: u32,
    child: Mutex<Proc>,
    io: Mutex<Io>,
}

struct Io {
    stdin: Stdin,
    stdout: BufReader<Stdout>,
}

/// Locks, taking the guard from a poisoned mutex too: the state behind these
/// locks is only ever a process handle or a counter.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What the spawner thread is asked for: the program, the helper's mode
/// argument, its memory limit, and where to send the result.
type SpawnRequest = (PathBuf, &'static str, usize, Sender<io::Result<Started>>);

/// Helpers are started from one thread that lives as long as the process.
/// `PR_SET_PDEATHSIG` fires when the *thread* that started a child ends, not
/// the process, and the thread a command runs on can be a pool thread that
/// ends when it has been idle.
fn spawner() -> &'static Sender<SpawnRequest> {
    static SPAWNER: OnceLock<Sender<SpawnRequest>> = OnceLock::new();
    SPAWNER.get_or_init(|| {
        let (sender, requests) = mpsc::channel::<SpawnRequest>();
        let started = thread::Builder::new().name("helper-spawner".into()).spawn(move || {
            for (exe, mode_argument, memory_limit, reply) in requests {
                let _ = reply.send(start(&exe, mode_argument, memory_limit));
            }
        });
        if let Err(err) = started {
            log::error!("could not start the helper spawner: {err}");
        }
        sender
    })
}

impl HelperProcess {
    /// Starts `exe mode_argument` confined, with `memory_limit` bytes (which
    /// the helper must call `confine::confine` with too).
    pub fn spawn(
        exe: &Path,
        mode_argument: &'static str,
        memory_limit: usize,
    ) -> io::Result<Arc<Self>> {
        confine::log_support_once();
        let (reply, answer) = mpsc::channel();
        spawner()
            .send((exe.to_owned(), mode_argument, memory_limit, reply))
            .map_err(|_| io::Error::other("no spawner"))?;
        let (child, stdin, stdout) =
            answer.recv().map_err(|_| io::Error::other("no spawner"))??;
        Ok(Arc::new(Self {
            pid: child.id(),
            child: Mutex::new(child),
            io: Mutex::new(Io { stdin, stdout: BufReader::new(stdout) }),
        }))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn kill(&self) {
        let _ = lock(&self.child).kill();
    }

    /// Kills it if it still runs, and waits, so no zombie is left.
    pub fn reap(&self) {
        let mut child = lock(&self.child);
        let _ = child.kill();
        let _ = child.wait();
    }

    /// One request and its answer through `exchange`, which writes to the
    /// helper's stdin and reads from its stdout, killing the helper when
    /// `limit` passes. `None` for any failure of the pipe (or `exchange`
    /// answering `Ok(None)`: the helper closed its end), the helper having
    /// been killed.
    pub fn exchange<R>(
        self: &Arc<Self>,
        limit: Duration,
        exchange: impl FnOnce(&mut Stdin, &mut BufReader<Stdout>) -> io::Result<Option<R>>,
    ) -> Option<R> {
        let mut io = lock(&self.io);
        let _watchdog = Watchdog::start(self, limit);
        let io = &mut *io;
        match exchange(&mut io.stdin, &mut io.stdout) {
            Ok(Some(answer)) => Some(answer),
            _ => {
                self.kill();
                None
            }
        }
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        let child = self.child.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Kills the process when the time limit passes before the guard is dropped.
/// The kill ends a blocked write or read on the pipe, which then fails.
struct Watchdog {
    _done: Sender<()>,
}

impl Watchdog {
    fn start(process: &Arc<HelperProcess>, limit: Duration) -> Self {
        let (done, finished) = mpsc::channel::<()>();
        let process = Arc::clone(process);
        let started = thread::Builder::new().name("helper-watchdog".into()).spawn(move || {
            if finished.recv_timeout(limit) == Err(RecvTimeoutError::Timeout) {
                process.kill();
            }
        });
        if let Err(err) = started {
            log::error!("could not start a helper watchdog: {err}");
        }
        Self { _done: done }
    }
}
