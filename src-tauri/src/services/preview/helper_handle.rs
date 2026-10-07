//! The app's side of the render helper (research.md §11): [`HelperHandle`]
//! starts `hoplodex --render-helper`, loads a document into it, asks it for
//! pages, enforces the time limits by killing it, restarts it when it dies
//! (at most twice per preview), and kills and waits for it when dropped.

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::Child;
#[cfg(not(windows))]
use std::process::{ChildStdin, ChildStdout};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::Duration;

use zeroize::Zeroizing;

use super::PageSize;
use super::confine;
use super::helper::ARGUMENT;
use super::helper_protocol::{Request, Response, read_response, write_load, write_request};

/// How often a preview's helper is started again after it dies.
pub const MAX_RESTARTS: u8 = 2;

/// How long the helper may take (research.md §11): past either, the parent
/// kills it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperLimits {
    /// For the `Load`, which includes writing the document down the pipe.
    pub load: Duration,
    /// For each `Render`.
    pub render: Duration,
}

impl Default for HelperLimits {
    fn default() -> Self {
        Self { load: Duration::from_secs(20), render: Duration::from_secs(10) }
    }
}

#[derive(Debug)]
pub enum HelperError {
    /// The helper couldn't be started.
    Spawn(io::Error),
    /// The helper read the document and refused it (a damaged first
    /// directory). Its reason is for a developer only.
    Refused(String),
    /// This page couldn't be rendered; the helper is fine.
    PageFailed,
    /// The helper died or passed its time limit, and was killed. In `load`,
    /// the document wasn't loaded; in `render`, this is only seen when the
    /// restart didn't happen (it was closed meanwhile).
    Lost,
    /// The helper died more often than it is restarted.
    Exhausted,
    /// The helper was shut down on purpose, by the preview closing.
    Closed,
}

/// A started helper: a `Child`, or on Windows the process together with the
/// job object HoploDex made for it (`helper_job`). Both have `id`, `kill`
/// and `wait`.
#[cfg(not(windows))]
type Proc = Child;
#[cfg(windows)]
type Proc = super::helper_job::JobChild;

/// The helper's pipes: a `Child`'s, or on Windows plain files (see
/// `helper_job::spawn`).
#[cfg(not(windows))]
type Stdin = ChildStdin;
#[cfg(not(windows))]
type Stdout = ChildStdout;
#[cfg(windows)]
type Stdin = std::fs::File;
#[cfg(windows)]
type Stdout = std::fs::File;

/// A helper just started, with its pipes.
type Started = (Proc, Stdin, Stdout);

/// Starts the helper the app's way (research.md §11). On Windows that is into
/// its own job object, by `CreateProcessW`, before it runs any code.
fn start(exe: &Path) -> io::Result<Started> {
    #[cfg(windows)]
    {
        super::helper_job::spawn(exe, &[])
    }
    #[cfg(not(windows))]
    {
        let mut child = HelperHandle::command(exe).spawn()?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("no pipes"));
        };
        Ok((child, stdin, stdout))
    }
}

/// One helper process and its pipes.
struct Process {
    pid: u32,
    child: Mutex<Proc>,
    io: Mutex<Io>,
}

struct Io {
    stdin: Stdin,
    stdout: BufReader<Stdout>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What the spawner thread is asked for.
type SpawnRequest = (PathBuf, Sender<io::Result<Started>>);

/// Helpers are started from one thread that lives as long as the process.
/// `PR_SET_PDEATHSIG` fires when the *thread* that started a child ends, not
/// the process, and the thread a command runs on can be a pool thread that
/// ends when it has been idle.
fn spawner() -> &'static Sender<SpawnRequest> {
    static SPAWNER: OnceLock<Sender<SpawnRequest>> = OnceLock::new();
    SPAWNER.get_or_init(|| {
        let (sender, requests) = mpsc::channel::<SpawnRequest>();
        let started =
            thread::Builder::new().name("preview-helper-spawner".into()).spawn(move || {
                for (exe, reply) in requests {
                    let _ = reply.send(start(&exe));
                }
            });
        if let Err(err) = started {
            log::error!("could not start the helper spawner: {err}");
        }
        sender
    })
}

impl Process {
    fn spawn(exe: &Path) -> Result<Arc<Self>, HelperError> {
        confine::log_support_once();
        let (reply, answer) = mpsc::channel();
        spawner()
            .send((exe.to_owned(), reply))
            .map_err(|_| HelperError::Spawn(io::Error::other("no spawner")))?;
        let (child, stdin, stdout) = answer
            .recv()
            .map_err(|_| HelperError::Spawn(io::Error::other("no spawner")))?
            .map_err(HelperError::Spawn)?;
        Ok(Arc::new(Self {
            pid: child.id(),
            child: Mutex::new(child),
            io: Mutex::new(Io { stdin, stdout: BufReader::new(stdout) }),
        }))
    }

    fn kill(&self) {
        let _ = lock(&self.child).kill();
    }

    /// Kills it if it still runs, and waits, so no zombie is left.
    fn reap(&self) {
        let mut child = lock(&self.child);
        let _ = child.kill();
        let _ = child.wait();
    }

    /// One request and its answer, killing the helper when `limit` passes.
    /// `Lost` for any failure of the pipe, the helper having been killed.
    fn exchange(
        self: &Arc<Self>,
        limit: Duration,
        send: impl FnOnce(&mut Stdin) -> io::Result<()>,
    ) -> Result<Response, HelperError> {
        let mut io = lock(&self.io);
        let _watchdog = Watchdog::start(self, limit);
        let answer = send(&mut io.stdin).and_then(|()| read_response(&mut io.stdout));
        match answer {
            Ok(Some(response)) => Ok(response),
            _ => {
                self.kill();
                Err(HelperError::Lost)
            }
        }
    }

    fn load(self: &Arc<Self>, bytes: &[u8], limit: Duration) -> Result<Vec<PageSize>, HelperError> {
        match self.exchange(limit, |stdin| write_load(stdin, bytes))? {
            Response::Loaded { pages } => Ok(pages),
            Response::Failed { reason } => Err(HelperError::Refused(reason)),
            _ => {
                self.kill();
                Err(HelperError::Lost)
            }
        }
    }

    fn render(
        self: &Arc<Self>,
        page: u32,
        width_px: u32,
        limit: Duration,
    ) -> Result<Vec<u8>, HelperError> {
        let response = self
            .exchange(limit, |stdin| write_request(stdin, &Request::Render { page, width_px }))?;
        match response {
            Response::Page { png } => Ok(png),
            Response::PageFailed => Err(HelperError::PageFailed),
            _ => {
                self.kill();
                Err(HelperError::Lost)
            }
        }
    }
}

impl Drop for Process {
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
    fn start(process: &Arc<Process>, limit: Duration) -> Self {
        let (done, finished) = mpsc::channel::<()>();
        let process = Arc::clone(process);
        let started =
            thread::Builder::new().name("preview-helper-watchdog".into()).spawn(move || {
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

struct State {
    process: Arc<Process>,
    restarts: u8,
}

/// What a preview's helper is, for as long as the preview lasts: shared
/// between the [`HelperHandle`] the preview owns and the renders in flight,
/// which don't hold the session's lock.
pub struct HelperShared {
    exe: PathBuf,
    limits: HelperLimits,
    closed: AtomicBool,
    state: Mutex<State>,
    /// What a restarted helper loads again.
    document: Mutex<Option<Arc<Zeroizing<Vec<u8>>>>>,
    /// One restart at a time.
    restarting: Mutex<()>,
}

enum Recovery {
    /// A new helper has the document loaded.
    Restarted,
    /// Another request already replaced the failed one.
    Superseded,
    Exhausted,
    Closed,
}

impl HelperShared {
    pub fn pid(&self) -> u32 {
        lock(&self.state).process.pid
    }

    pub fn restarts(&self) -> u8 {
        lock(&self.state).restarts
    }

    /// Ends the helper at once and for good: whatever is blocked on it
    /// fails, and nothing starts it again.
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        lock(&self.state).process.kill();
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn current(&self) -> Arc<Process> {
        Arc::clone(&lock(&self.state).process)
    }

    fn load(&self, bytes: &[u8]) -> Result<Vec<PageSize>, HelperError> {
        if self.is_closed() {
            return Err(HelperError::Closed);
        }
        match self.current().load(bytes, self.limits.load) {
            Ok(pages) => {
                *lock(&self.document) = Some(Arc::new(Zeroizing::new(bytes.to_vec())));
                Ok(pages)
            }
            Err(_) if self.is_closed() => Err(HelperError::Closed),
            Err(err) => Err(err),
        }
    }

    /// Renders a page. A helper that dies (or passes its limit) on it fails
    /// that page and is replaced for the others, up to [`MAX_RESTARTS`]
    /// times; the one after that is `Exhausted`.
    pub fn render(&self, page: u32, width_px: u32) -> Result<Vec<u8>, HelperError> {
        for _ in 0..3 {
            if self.is_closed() {
                return Err(HelperError::Closed);
            }
            let process = self.current();
            match process.render(page, width_px, self.limits.render) {
                Err(HelperError::Lost) => match self.recover(&process) {
                    Recovery::Restarted => return Err(HelperError::PageFailed),
                    Recovery::Superseded => continue,
                    Recovery::Exhausted => return Err(HelperError::Exhausted),
                    Recovery::Closed => return Err(HelperError::Closed),
                },
                other => return other,
            }
        }
        Err(HelperError::PageFailed)
    }

    fn recover(&self, failed: &Arc<Process>) -> Recovery {
        let _one_at_a_time = lock(&self.restarting);
        if self.is_closed() {
            return Recovery::Closed;
        }
        {
            let state = lock(&self.state);
            if !Arc::ptr_eq(&state.process, failed) {
                return Recovery::Superseded;
            }
            if state.restarts >= MAX_RESTARTS {
                return Recovery::Exhausted;
            }
        }
        failed.kill();
        let Some(document) = lock(&self.document).clone() else { return Recovery::Exhausted };
        let Ok(replacement) = Process::spawn(&self.exe) else { return Recovery::Exhausted };
        if replacement.load(&document, self.limits.load).is_err() {
            replacement.reap();
            return Recovery::Exhausted;
        }
        let old = {
            let mut state = lock(&self.state);
            if self.is_closed() {
                drop(state);
                replacement.reap();
                return Recovery::Closed;
            }
            state.restarts += 1;
            std::mem::replace(&mut state.process, replacement)
        };
        old.reap();
        Recovery::Restarted
    }
}

/// The render helper of one TIFF preview. Dropping it kills the process and
/// waits for it.
pub struct HelperHandle {
    shared: Arc<HelperShared>,
}

impl HelperHandle {
    /// The command the app runs for the helper: `--render-helper`, a cleared
    /// environment, its stdin and stdout piped, stderr discarded, and no
    /// console window on Windows (research.md §11). Tests add arguments to
    /// it and spawn it themselves. On Windows the app does not run this
    /// command: it starts the helper into its job object with
    /// `helper_job::spawn`, so a helper started from this command is in no
    /// job of HoploDex's and refuses to serve.
    pub fn command(exe: &Path) -> Command {
        let mut command = Command::new(exe);
        command
            .arg(ARGUMENT)
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

    /// Starts the helper. Nothing is loaded yet.
    pub fn spawn(exe: &Path, limits: HelperLimits) -> Result<Self, HelperError> {
        let process = Process::spawn(exe)?;
        Ok(Self {
            shared: Arc::new(HelperShared {
                exe: exe.to_owned(),
                limits,
                closed: AtomicBool::new(false),
                state: Mutex::new(State { process, restarts: 0 }),
                document: Mutex::new(None),
                restarting: Mutex::new(()),
            }),
        })
    }

    /// Sends the document and returns every page's size. The helper keeps a
    /// copy of the bytes, in memory only, to load a restarted helper with.
    pub fn load(&mut self, bytes: &[u8]) -> Result<Vec<PageSize>, HelperError> {
        self.shared.load(bytes)
    }

    /// One page as PNG bytes about `width_px` wide.
    pub fn render(&mut self, page: u32, width_px: u32) -> Result<Vec<u8>, HelperError> {
        self.shared.render(page, width_px)
    }

    /// The running helper's process id (a new one after a restart).
    pub fn pid(&self) -> u32 {
        self.shared.pid()
    }

    pub fn restarts(&self) -> u8 {
        self.shared.restarts()
    }

    /// A handle that renders and shuts the helper down without owning it, so
    /// a render can run while the session's lock is free and `close_preview`
    /// can end a `Load` still under way.
    pub fn share(&self) -> Arc<HelperShared> {
        Arc::clone(&self.shared)
    }
}

impl Drop for HelperHandle {
    fn drop(&mut self) {
        self.shared.shutdown();
        // Wait for it, so the process is gone when the preview is.
        self.shared.current().reap();
    }
}
