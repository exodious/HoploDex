//! The app's side of the TIFF render helper (007 research.md §11):
//! [`HelperHandle`] starts `hoplodex --render-helper`, loads a document into
//! it, asks it for pages, restarts it when it dies (at most twice per
//! preview), and kills and waits for it when dropped. Starting, the time
//! limits and the kill are the shared core's (`services::helper::handle`).
//! The helper answers with pixels, which are checked and encoded here: the
//! main web view only ever gets a PNG the app made (007 research.md §11).

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zeroize::Zeroizing;

use super::PageSize;
use super::helper::{ARGUMENT, MEMORY_LIMIT};
use super::helper_protocol::{Pixels, Request, Response, read_response, write_load, write_request};
use super::tiff::{MAX_PIXELS, MAX_WIDTH_PX, encode_png};
use crate::services::helper::handle::{HelperProcess, lock};

/// How often a preview's helper is started again after it dies.
pub const MAX_RESTARTS: u8 = 2;

/// How long the helper may take (007 research.md §11): past either, the parent
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

/// The TIFF requests, on a helper process.
fn load(
    process: &Arc<HelperProcess>,
    bytes: &[u8],
    limit: Duration,
) -> Result<Vec<PageSize>, HelperError> {
    let response = process
        .exchange(limit, |stdin, stdout| {
            write_load(stdin, bytes).and_then(|()| read_response(stdout))
        })
        .ok_or(HelperError::Lost)?;
    match response {
        Response::Loaded { pages } => Ok(pages),
        Response::Failed { reason } => Err(HelperError::Refused(reason)),
        _ => {
            process.kill();
            Err(HelperError::Lost)
        }
    }
}

/// Whether a page's pixels are what was asked for: no wider than the width
/// requested and the page limits (`tiff::output_size`), and exactly as many
/// samples as they say. The helper may be hostile, and this is all the app
/// reads of them before encoding them.
fn fits_the_request(pixels: &Pixels, width_px: u32) -> bool {
    let area = u64::from(pixels.width) * u64::from(pixels.height);
    (1..=width_px.clamp(1, MAX_WIDTH_PX)).contains(&pixels.width)
        && pixels.height >= 1
        && area <= MAX_PIXELS
        && matches!(pixels.channels, 1 | 3)
        && pixels.samples.len() as u64 == area * u64::from(pixels.channels)
}

fn render(
    process: &Arc<HelperProcess>,
    page: u32,
    width_px: u32,
    limit: Duration,
) -> Result<Vec<u8>, HelperError> {
    let response = process
        .exchange(limit, |stdin, stdout| {
            write_request(stdin, &Request::Render { page, width_px })
                .and_then(|()| read_response(stdout))
        })
        .ok_or(HelperError::Lost)?;
    match response {
        Response::Page(pixels) if fits_the_request(&pixels, width_px) => {
            encode_png(&pixels.samples, pixels.channels, pixels.width, pixels.height)
                .map_err(|_| HelperError::PageFailed)
        }
        Response::PageFailed => Err(HelperError::PageFailed),
        _ => {
            process.kill();
            Err(HelperError::Lost)
        }
    }
}

/// Starts a TIFF helper process.
fn spawn_process(exe: &Path) -> Result<Arc<HelperProcess>, HelperError> {
    HelperProcess::spawn(exe, ARGUMENT, MEMORY_LIMIT).map_err(HelperError::Spawn)
}

struct State {
    process: Arc<HelperProcess>,
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
        lock(&self.state).process.pid()
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

    fn current(&self) -> Arc<HelperProcess> {
        Arc::clone(&lock(&self.state).process)
    }

    fn load(&self, bytes: &[u8]) -> Result<Vec<PageSize>, HelperError> {
        if self.is_closed() {
            return Err(HelperError::Closed);
        }
        match load(&self.current(), bytes, self.limits.load) {
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
            match render(&process, page, width_px, self.limits.render) {
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

    fn recover(&self, failed: &Arc<HelperProcess>) -> Recovery {
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
        let Ok(replacement) = spawn_process(&self.exe) else { return Recovery::Exhausted };
        if load(&replacement, &document, self.limits.load).is_err() {
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
    /// console window on Windows (007 research.md §11). Tests add arguments
    /// to it and spawn it themselves. On Windows the app does not run this
    /// command: it starts the helper into its job object with
    /// `helper::job::spawn`, so a helper started from this command is in no
    /// job of HoploDex's and refuses to serve.
    pub fn command(exe: &Path) -> Command {
        crate::services::helper::handle::command(exe, ARGUMENT)
    }

    /// Starts the helper. Nothing is loaded yet.
    pub fn spawn(exe: &Path, limits: HelperLimits) -> Result<Self, HelperError> {
        let process = spawn_process(exe)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(width: u32, height: u32, channels: u8, samples: usize) -> Pixels {
        Pixels { width, height, channels, samples: vec![0; samples] }
    }

    #[test]
    fn pixels_of_the_size_asked_for_fit() {
        assert!(fits_the_request(&pixels(100, 50, 1, 5000), 100));
        assert!(fits_the_request(&pixels(100, 50, 3, 15_000), 100));
        assert!(fits_the_request(&pixels(50, 50, 1, 2500), 100), "a page narrower than asked");
        assert!(fits_the_request(&pixels(4096, 1, 1, 4096), 10_000), "the widest page");
    }

    #[test]
    fn pixels_of_any_other_shape_do_not() {
        assert!(!fits_the_request(&pixels(101, 50, 1, 5050), 100), "wider than asked");
        assert!(!fits_the_request(&pixels(4097, 1, 1, 4097), 10_000), "wider than any page");
        assert!(!fits_the_request(&pixels(0, 50, 1, 0), 100), "no width");
        assert!(!fits_the_request(&pixels(100, 0, 1, 0), 100), "no height");
        assert!(!fits_the_request(&pixels(100, 50, 2, 10_000), 100), "two channels");
        assert!(!fits_the_request(&pixels(100, 50, 4, 20_000), 100), "four channels");
        assert!(!fits_the_request(&pixels(100, 50, 0, 0), 100), "no channels");
        assert!(!fits_the_request(&pixels(100, 50, 1, 4999), 100), "a sample short");
        assert!(!fits_the_request(&pixels(100, 50, 1, 5001), 100), "a sample over");
        let tall = (MAX_PIXELS + 1) as u32;
        assert!(!fits_the_request(&pixels(1, tall, 1, 0), 100), "over the pixel limit");
        assert!(!fits_the_request(&pixels(u32::MAX, u32::MAX, 3, 0), u32::MAX), "no overflow");
    }
}
