//! The loopback tripwire that proves the PDF surface reaches no network
//! (research.md §6).
//!
//! The surface's proxy points at this listener, a port HoploDex holds on
//! `127.0.0.1`: nothing else can bind it, so a request that arrives is one
//! the surface's other layers (the content filter, WebRTC off, navigation
//! rules) failed to stop. It accepts and closes each connection without
//! reading a byte, counts them for tests, and says once per run that a
//! request was refused, without its address, which could come from the
//! document.

use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

/// The listener, bound at the first PDF preview and held for the run.
pub struct Tripwire {
    addr: SocketAddr,
    connections: Arc<AtomicUsize>,
}

impl Tripwire {
    /// Binds `127.0.0.1:0` and starts answering. The thread and the listener
    /// live until the process exits.
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&connections);
        let logged = Arc::new(AtomicBool::new(false));
        thread::Builder::new().name("preview-tripwire".into()).spawn(move || {
            for stream in listener.incoming() {
                // Closed unread: dropping the stream closes it.
                if stream.is_err() {
                    continue;
                }
                counter.fetch_add(1, Ordering::SeqCst);
                if !logged.swap(true, Ordering::SeqCst) {
                    log::warn!(
                        "the PDF preview tried to reach the network; the request was refused"
                    );
                }
            }
        })?;
        Ok(Self { addr, connections })
    }

    /// The address for the surface's `proxy_url`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// How many connections have been accepted and closed.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

/// The run's tripwire, bound by the first call. `None` when it can't be
/// bound, which a caller treats as the surface being unavailable.
pub fn shared() -> Option<&'static Tripwire> {
    static TRIPWIRE: OnceLock<Option<Tripwire>> = OnceLock::new();
    TRIPWIRE
        .get_or_init(|| match Tripwire::bind() {
            Ok(tripwire) => Some(tripwire),
            Err(err) => {
                log::error!("could not bind the preview tripwire: {err}");
                None
            }
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    fn wait_for(tripwire: &Tripwire, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while tripwire.connections() < count && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn it_listens_on_loopback_counts_each_connection_and_closes_it_unread() {
        let tripwire = Tripwire::bind().unwrap();
        assert!(tripwire.addr().ip().is_loopback());
        assert_eq!(tripwire.connections(), 0);

        for round in 1..=2 {
            let mut stream = TcpStream::connect(tripwire.addr()).unwrap();
            let _ = stream.write_all(b"GET http://example.com/ HTTP/1.1\r\n\r\n");
            let mut reply = Vec::new();
            // Closed without a byte of reply (a reset counts as closed).
            let _ = stream.read_to_end(&mut reply);
            assert!(reply.is_empty(), "the tripwire answers nothing");
            wait_for(&tripwire, round);
            assert_eq!(tripwire.connections(), round);
        }
    }

    #[test]
    fn the_shared_tripwire_is_bound_once() {
        let first = shared().expect("binds").addr();
        assert_eq!(shared().unwrap().addr(), first);
    }
}
