//! The render helper's process: its start and its main loop (research.md
//! §11).
//!
//! The app starts itself again as `hoplodex --render-helper`. `main()` hands
//! the arguments to [`run`] before it sets up anything else, so the helper
//! has no Tauri, no keyring, no session and no logging. `run` confines the
//! process ([`super::confine`]) and only then reads a frame, serves them
//! over stdin and stdout with [`super::tiff`], and exits when stdin ends.
//! Nothing a document says is written anywhere else: stderr is discarded by
//! the parent and the helper writes nothing to it.

use std::io::{self, BufReader, BufWriter, Write};

use super::helper_protocol::{Request, Response, read_request, write_response};
use super::{confine, tiff};

/// What the app starts its helper with.
pub const ARGUMENT: &str = "--render-helper";

/// Runs the helper to its end and exits the process: `args` are the
/// process's arguments after the program name, the first being
/// [`ARGUMENT`]. It never returns to the app's own start-up path.
pub fn run(args: &[String]) -> ! {
    // Nothing a panic says is wanted anywhere, and stderr is discarded.
    std::panic::set_hook(Box::new(|_| {}));
    #[cfg(debug_assertions)]
    if args.get(1).map(String::as_str) == Some("--self-check") {
        std::process::exit(self_check::run(&args[2..]));
    }
    let _ = args;
    if confine::confine().is_err() {
        // Not confined: read no document.
        std::process::exit(3);
    }
    std::process::exit(match serve() {
        Ok(()) => 0,
        Err(_) => 4,
    });
}

/// The main loop: one `Load`, then renders, until the parent closes the pipe.
fn serve() -> io::Result<()> {
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    let mut document: Option<Vec<u8>> = None;
    while let Some(request) = read_request(&mut input)? {
        let response = match request {
            Request::Load(bytes) => {
                document = None;
                match tiff::load(&bytes) {
                    Ok(pages) => {
                        document = Some(bytes);
                        Response::Loaded { pages }
                    }
                    Err(reason) => Response::Failed { reason },
                }
            }
            Request::Render { page, width_px } => match &document {
                Some(bytes) => match tiff::render_page(bytes, page, width_px) {
                    Ok(png) => Response::Page { png },
                    Err(_) => Response::PageFailed,
                },
                None => Response::PageFailed,
            },
        };
        write_response(&mut output, &response)?;
        output.flush()?;
    }
    Ok(())
}

/// `hoplodex --render-helper --self-check <port> <file>`: what the helper
/// could do before and after confining itself, as one JSON line. It is the
/// only way to look inside a confined process from outside
/// (tests/tiff_helper_test.rs), so it is compiled into debug and test builds
/// only: a release binary has no such mode.
#[cfg(debug_assertions)]
mod self_check {
    use std::fs::File;
    use std::net::TcpStream;
    use std::process::{Command, Stdio};

    use serde_json::json;

    use super::confine;

    /// A program that exists on the OS and exits at once: whether the
    /// confined helper can start it is the probe.
    #[cfg(not(windows))]
    const PROGRAM: &str = "/bin/true";
    #[cfg(windows)]
    const PROGRAM: &str = r"C:\Windows\System32\whoami.exe";

    pub fn run(args: &[String]) -> i32 {
        let (Some(port), Some(file)) =
            (args.first().and_then(|p| p.parse::<u16>().ok()), args.get(1))
        else {
            return 2;
        };
        let can_open = || File::open(file).is_ok();
        let can_connect = || TcpStream::connect(("127.0.0.1", port)).is_ok();
        let file_before = can_open();
        let tcp_before = can_connect();
        let Ok(confinement) = confine::confine() else { return 3 };
        let exec_after = Command::new(PROGRAM)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok();
        #[cfg(unix)]
        let rlimit_core = {
            let mut limit = libc::rlimit { rlim_cur: 1, rlim_max: 1 };
            // SAFETY: a plain system call with a pointer to a local.
            let read = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) } == 0;
            read.then(|| json!([limit.rlim_cur, limit.rlim_max]))
        };
        #[cfg(not(unix))]
        let rlimit_core: Option<serde_json::Value> = None;
        #[cfg(target_os = "linux")]
        // SAFETY: a plain system call.
        let dumpable = Some(unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0);
        #[cfg(not(target_os = "linux"))]
        let dumpable: Option<bool> = None;
        println!(
            "{}",
            json!({
                "file_before": file_before,
                "tcp_before": tcp_before,
                "file_after": can_open(),
                "tcp_after": can_connect(),
                "exec_after": exec_after,
                "env_vars": std::env::vars_os().count(),
                "rlimit_core": rlimit_core,
                "dumpable": dumpable,
                "landlock_abi": confinement.landlock_abi,
                "landlock_enforced": confinement.landlock_enforced,
            })
        );
        0
    }
}
