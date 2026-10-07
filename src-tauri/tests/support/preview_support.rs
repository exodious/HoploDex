//! Shared by the feature 007 preview tests (`pdf_preview_test`,
//! `tiff_preview_test`, `tiff_helper_test` and `lock_test`): a recording
//! `PreviewSurface`, the `PreviewEnv` the `ops` take, a session with a
//! firearm to attach documents to, and process helpers. A test crate pulls it
//! in with
//! `#[path = "support/preview_support.rs"] mod preview_support;`
//! beside `mod support;`. It is not a part of `support/mod.rs` so that the
//! other test crates keep compiling.
//!
//! Assumed API (written ahead of T048-T054, T061; the shapes are in each
//! item's doc comment):
//! - `services::preview::{PreviewSurface, SurfaceFactory, SurfaceSpec,
//!   Preview, PreviewContent, HelperHandle, HelperLimits}`;
//! - `commands::preview::PreviewEnv` and `commands::preview::ops`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::preview::{PreviewEnv, ops as preview};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::preview::availability::PdfAvailabilityState;
use hoplodex_lib::services::preview::surface::Rect;
use hoplodex_lib::services::preview::{
    PreviewContent, PreviewSurface, SurfaceFactory, SurfaceSpec,
};
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::Value;
use tempfile::TempDir;

use crate::support::{self, TestEvents, passphrase, test_session};

// ------------------------------------------------------------- the surface

#[derive(Default)]
struct Calls {
    urls: Vec<String>,
    bounds: Vec<(Rect, bool)>,
    focus: usize,
    closed: bool,
}

/// What one recorded surface was asked to do. Shared by the
/// [`RecordingSurface`] the session holds and the test that looks at it, so a
/// surface dropped by the session is still reported as closed.
pub struct SurfaceLog {
    secret: String,
    calls: Mutex<Calls>,
}

impl SurfaceLog {
    /// The surface secret the session generated for the frame script
    /// (`SurfaceSpec::secret`): `GET /hooked/<secret>` is the Linux hook.
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Every URL the surface was built on or navigated to, in order.
    pub fn urls(&self) -> Vec<String> {
        self.calls.lock().unwrap().urls.clone()
    }

    /// The URL it shows now.
    pub fn url(&self) -> String {
        self.urls().pop().expect("the surface was never given a URL")
    }

    /// Every `set_bounds(rect, visible)` call, in order.
    pub fn bounds_calls(&self) -> Vec<(Rect, bool)> {
        self.calls.lock().unwrap().bounds.clone()
    }

    /// Whether any call asked to show it.
    pub fn ever_shown(&self) -> bool {
        self.bounds_calls().iter().any(|(_, visible)| *visible)
    }

    pub fn focus_calls(&self) -> usize {
        self.calls.lock().unwrap().focus
    }

    /// `close()` was called, or the session dropped the surface.
    pub fn is_closed(&self) -> bool {
        self.calls.lock().unwrap().closed
    }
}

/// The `PreviewSurface` the tests give the session in place of a web view.
struct RecordingSurface(Arc<SurfaceLog>);

impl PreviewSurface for RecordingSurface {
    fn navigate(&self, url: &str) {
        self.0.calls.lock().unwrap().urls.push(url.to_owned());
    }

    fn set_bounds(&self, rect: Rect, visible: bool) {
        self.0.calls.lock().unwrap().bounds.push((rect, visible));
    }

    fn focus(&self) {
        self.0.calls.lock().unwrap().focus += 1;
    }

    fn close(&self) {
        self.0.calls.lock().unwrap().closed = true;
    }
}

impl Drop for RecordingSurface {
    fn drop(&mut self) {
        self.0.calls.lock().unwrap().closed = true;
    }
}

/// Builds recording surfaces, and keeps a log of each.
#[derive(Default)]
pub struct Surfaces {
    built: Mutex<Vec<Arc<SurfaceLog>>>,
}

impl Surfaces {
    /// Every surface built so far.
    pub fn built(&self) -> Vec<Arc<SurfaceLog>> {
        self.built.lock().unwrap().clone()
    }

    /// The surface built, when exactly one has been.
    pub fn only(&self) -> Arc<SurfaceLog> {
        let built = self.built();
        assert_eq!(built.len(), 1, "expected exactly one surface to have been built");
        Arc::clone(&built[0])
    }

    /// The most recently built surface.
    pub fn latest(&self) -> Arc<SurfaceLog> {
        self.built().pop().expect("no surface was built")
    }
}

impl SurfaceFactory for Surfaces {
    fn build(&self, spec: SurfaceSpec) -> Result<Arc<dyn PreviewSurface>, String> {
        let log = Arc::new(SurfaceLog {
            secret: spec.secret,
            calls: Mutex::new(Calls { urls: vec![spec.url], ..Calls::default() }),
        });
        self.built.lock().unwrap().push(Arc::clone(&log));
        Ok(Arc::new(RecordingSurface(log)))
    }
}

// ------------------------------------------------------------ the preview env

/// The `PreviewEnv` the `ops` take, with the recording surfaces and the
/// availability it was built on. `env.helper_exe` is the real helper binary.
pub struct TestPreview {
    pub env: PreviewEnv,
    pub surfaces: Arc<Surfaces>,
    pub availability: Arc<PdfAvailabilityState>,
}

impl TestPreview {
    pub fn new() -> Self {
        let surfaces = Arc::new(Surfaces::default());
        let availability = Arc::new(PdfAvailabilityState::default());
        let env = PreviewEnv::new(
            PathBuf::from(env!("CARGO_BIN_EXE_hoplodex")),
            surfaces.clone(),
            availability.clone(),
        );
        Self { env, surfaces, availability }
    }
}

// --------------------------------------------------------------- a session

/// An open database with one firearm, a recording surface factory and the
/// real helper, in throwaway folders.
pub struct Fixture {
    pub config: TempDir,
    pub dir: TempDir,
    pub session: Arc<Session>,
    pub events: Arc<TestEvents>,
    pub machine: Arc<MachineSettings>,
    pub preview: TestPreview,
    pub firearm: i64,
}

impl Fixture {
    pub fn new() -> Self {
        let config = TempDir::new().unwrap();
        let dir = TempDir::new().unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        let machine = Arc::new(MachineSettings::load(config.path()).unwrap());
        lifecycle::create(&session, &machine, &dir.path().join("Mine.hoplodex"), &passphrase())
            .unwrap();
        let session = Arc::new(session);
        let firearm = new_firearm(&session);
        Self { config, dir, session, events, machine, preview: TestPreview::new(), firearm }
    }

    /// Attaches `bytes` to the firearm as a document named `name`.
    pub fn add(&self, name: &str, bytes: &[u8]) -> i64 {
        add_document(&self.session, self.firearm, name, bytes)
    }

    /// `open_preview`, with its answer as the JSON the viewer would get.
    pub fn open(&self, document_id: i64) -> Result<Value, hoplodex_lib::commands::CommandError> {
        preview::open_preview(&self.session, &self.preview.env, document_id)
            .map(|info| serde_json::to_value(info).unwrap())
    }

    pub fn open_ok(&self, document_id: i64) -> Value {
        self.open(document_id).unwrap_or_else(|e| panic!("open_preview failed: {e:?}"))
    }

    /// `previewId` of an `open_preview` answer.
    pub fn id_of(info: &Value) -> u64 {
        info["previewId"].as_u64().unwrap_or_else(|| panic!("no previewId in {info}"))
    }

    pub fn close(&self, preview_id: u64) {
        preview::close_preview(&self.session, preview_id).unwrap();
    }

    /// The PID of the open TIFF preview's helper.
    pub fn helper_pid(&self) -> Option<u32> {
        tiff_pid(&self.session)
    }
}

/// A new firearm in the open database.
pub fn new_firearm(session: &Session) -> i64 {
    let serial = hoplodex_lib::db::random_hex(4).unwrap();
    session
        .write(|conn| {
            firearms::create_firearm(conn, &support::firearm("Glock", "19", &serial), false, None)
                .map(|created| created.id)
        })
        .unwrap()
}

/// Attaches a document to `firearm` through the real `add_document`.
pub fn add_document(session: &Session, firearm: i64, name: &str, bytes: &[u8]) -> i64 {
    session
        .write(|conn| {
            document_ops::add_document(conn, RecordRef::Firearm(firearm), bytes, name)
                .map(|document| document.id)
        })
        .unwrap_or_else(|e| panic!("attaching {name}: {e:?}"))
}

/// A row as an older build or a bug could have left it: a type and bytes the
/// attach rules would refuse, stored as given.
pub fn insert_raw_document(
    session: &Session,
    firearm: i64,
    name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> i64 {
    session
        .write(|conn| {
            conn.execute(
                "INSERT INTO document_attachments
                    (firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at)
                 VALUES (?1, NULL, ?2, ?3, ?4, datetime('now'))",
                rusqlite::params![firearm, bytes, name, mime_type],
            )
            .unwrap();
            Ok(conn.last_insert_rowid())
        })
        .unwrap()
}

/// The PID of the helper of the open TIFF preview, if there is one. The one
/// place that reaches into `Preview`: `PreviewContent::Tiff { helper, .. }`
/// and `HelperHandle::pid()`.
pub fn tiff_pid(session: &Session) -> Option<u32> {
    session
        .inspect(|open| {
            Ok(match open.preview.as_ref().map(|p| &p.content) {
                Some(PreviewContent::Tiff { helper, .. }) => Some(helper.pid()),
                _ => None,
            })
        })
        .unwrap()
}

// ------------------------------------------------------------------ PNG

/// The width and height of a PNG, checking its signature.
pub fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.len() > 24, "{} bytes is not a PNG", png.len());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    assert_eq!(&png[12..16], b"IHDR");
    (
        u32::from_be_bytes(png[16..20].try_into().unwrap()),
        u32::from_be_bytes(png[20..24].try_into().unwrap()),
    )
}

// ------------------------------------------------------------- processes

/// Whether the process is gone. A process that has exited but not been
/// waited for (a zombie) counts as gone: it runs nothing.
pub fn process_is_gone(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => {
                // "pid (comm) S ...": the state follows the last ')'.
                let state = stat.rsplit(')').next().unwrap_or("").trim_start().chars().next();
                matches!(state, Some('Z') | Some('X'))
            }
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // SAFETY: signal 0 only checks that the process exists.
        let found = unsafe { libc::kill(pid as i32, 0) } == 0;
        !found
    }
    #[cfg(windows)]
    {
        let listing = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .expect("tasklist");
        !String::from_utf8_lossy(&listing.stdout).contains(&pid.to_string())
    }
}

/// Waits up to `within` for the process to be gone.
pub fn wait_until_gone(pid: u32, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if process_is_gone(pid) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub fn assert_gone(pid: u32, what: &str) {
    assert!(wait_until_gone(pid, Duration::from_secs(3)), "helper {pid} still runs {what}");
}

/// The PIDs of this process's render helpers: through `/proc` on Linux, a
/// snapshot of the processes on Windows (a child named `hoplodex.exe`, the
/// helper being the app's own executable), empty elsewhere.
pub fn helper_children() -> Vec<u32> {
    #[cfg(target_os = "linux")]
    {
        let me = std::process::id();
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir("/proc") else { return found };
        for entry in entries.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
            let after = stat.rsplit(')').next().unwrap_or("");
            let mut fields = after.split_whitespace();
            let (state, ppid) = (fields.next(), fields.next().and_then(|p| p.parse::<u32>().ok()));
            if ppid != Some(me) || state == Some("Z") {
                continue;
            }
            let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else { continue };
            if String::from_utf8_lossy(&cmdline).contains("--render-helper") {
                found.push(pid);
            }
        }
        found
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        };

        let me = std::process::id();
        let mut found = Vec::new();
        // SAFETY: plain system calls; the snapshot is closed, and the
        // structure is zeroed and sized.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return found;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry);
            while more != 0 {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(0);
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if entry.th32ParentProcessID == me && name.eq_ignore_ascii_case("hoplodex.exe") {
                    found.push(entry.th32ProcessID);
                }
                more = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
        }
        found
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Vec::new()
    }
}

/// Waits up to `within` for a helper child of this process to exist.
pub fn wait_for_helper_child(within: Duration) -> Option<u32> {
    let deadline = Instant::now() + within;
    loop {
        if let Some(pid) = helper_children().into_iter().next() {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Waits up to `within` for `condition`.
pub fn wait_for(within: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Whether any file under `dir` holds `needle`.
pub fn any_file_holds(dir: &Path, needle: &[u8]) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return None };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if let Some(found) = any_file_holds(&path, needle) {
                return Some(found);
            }
        } else if kind.is_file()
            && let Ok(bytes) = fs::read(&path)
            && bytes.windows(needle.len()).any(|window| window == needle)
        {
            return Some(path);
        }
    }
    None
}
