//! The global allocator of every HoploDex process: the app and its render and
//! import helpers, which are the same executable (`main.rs`).
//!
//! - **Every freed block is zeroed first.** A Rust buffer that held the
//!   passphrase or a record's text (wry's and Tauri's IPC bodies, serde's
//!   values) would otherwise stay readable in freed memory until it is reused,
//!   and with it in a crash's memory. `realloc` is the trait's own (a new
//!   block, a copy, then [`GlobalAlloc::dealloc`] of the old one), so an old
//!   block is wiped too. This reaches only Rust's allocations, not WebKit's,
//!   GLib's or WebView2's.
//! - **On Windows, a failed allocation ends the process with
//!   `TerminateProcess`.** Rust's default handler would `abort()`, whose
//!   fail-fast goes to Windows Error Reporting whatever the error mode is, with
//!   the stack's memory and, at consent level 4, a dump Microsoft can ask for.
//!   The helpers' job objects cap their memory (`preview/helper.rs`,
//!   `import_reader/limits.rs`), so a hostile document reaches this. Elsewhere
//!   a null is returned, as `System` does (`set_alloc_error_hook` is unstable).
//!   A `GlobalAlloc` can't tell a fallible allocation from an infallible one,
//!   so on Windows this ends the process on a fallible one too instead of
//!   returning an error: the import helper's up-front reserve, and std's own
//!   (`std::fs::read`'s `try_reserve_exact`, `read_to_end`'s `try_reserve`).

use std::alloc::{GlobalAlloc, Layout};

/// The allocator `A` with each block zeroed before it goes back to `A`, and
/// on Windows a failed allocation ending the process. `main.rs` installs it
/// around `System`.
pub struct Wiping<A>(pub A);

// SAFETY: a pass-through to `A`, which keeps the layout contract; the only
// additions are the wipe of a block the caller is giving up, and on Windows
// ending the process instead of returning null.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Wiping<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, passed on.
        succeeded(unsafe { self.0.alloc(layout) })
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, passed on.
        succeeded(unsafe { self.0.alloc_zeroed(layout) })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller gives up `ptr`, a block of `layout.size()` bytes
        // from this allocator, so nothing else reads or writes it.
        unsafe {
            wipe(ptr, layout.size());
            self.0.dealloc(ptr, layout);
        }
    }
}

/// Zeroes `len` bytes at `ptr` in a way the optimizer can't drop as a dead
/// store before the free that follows.
///
/// # Safety
///
/// `ptr` must be valid for writes of `len` bytes.
pub unsafe fn wipe(ptr: *mut u8, len: usize) {
    // SAFETY: the caller's contract. A memset rather than `zeroize`'s
    // byte-by-byte volatile writes, which would slow every large free.
    unsafe { std::ptr::write_bytes(ptr, 0, len) };
    // An empty asm block that takes `ptr` may read the memory behind it, so the
    // memset above has to happen (the kernel's `memzero_explicit`).
    // SAFETY: it does nothing.
    unsafe { std::arch::asm!("/* {0} */", in(reg) ptr, options(nostack, preserves_flags)) };
}

/// `ptr`, a block from the allocator: on Windows, a null (the allocation
/// failed) ends the process.
#[inline]
fn succeeded(ptr: *mut u8) -> *mut u8 {
    #[cfg(windows)]
    if ptr.is_null() {
        end_process();
    }
    ptr
}

/// The exit code of a process a failed allocation ended (Windows only).
#[cfg(windows)]
pub const OUT_OF_MEMORY_EXIT_CODE: u32 = windows_sys::Win32::Foundation::STATUS_NO_MEMORY as u32;

/// Ends the process at once, without abort's fail-fast. It allocates nothing.
#[cfg(windows)]
#[cold]
fn end_process() {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
    // SAFETY: ends this process; the pseudo-handle needs no closing. Should it
    // ever return, the null goes back and Rust aborts as before.
    unsafe { TerminateProcess(GetCurrentProcess(), OUT_OF_MEMORY_EXIT_CODE) };
}

#[cfg(test)]
mod tests {
    use std::alloc::System;
    use std::cell::Cell;

    use super::*;

    /// `System`, noting whether each block came back to it all zeros.
    #[derive(Default)]
    struct Recorder {
        freed_zeroed: Cell<Option<bool>>,
    }

    unsafe impl GlobalAlloc for Recorder {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let block = unsafe { std::slice::from_raw_parts(ptr, layout.size()) };
            self.freed_zeroed.set(Some(block.iter().all(|&b| b == 0)));
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    /// A block of `len` bytes from `allocator`, filled with 0xA5.
    fn filled(allocator: &Wiping<Recorder>, len: usize) -> (*mut u8, Layout) {
        let layout = Layout::from_size_align(len, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, len) };
        (ptr, layout)
    }

    #[test]
    fn a_freed_block_goes_back_zeroed() {
        let allocator = Wiping(Recorder::default());
        for len in [1, 40, 1032, 1 << 20] {
            let (ptr, layout) = filled(&allocator, len);
            unsafe { allocator.dealloc(ptr, layout) };
            assert_eq!(allocator.0.freed_zeroed.take(), Some(true), "{len} bytes");
        }
    }

    #[test]
    fn a_reallocation_keeps_the_contents_and_zeroes_the_old_block() {
        let allocator = Wiping(Recorder::default());
        for (from, to) in [(40, 4096), (4096, 40)] {
            let (ptr, layout) = filled(&allocator, from);
            let moved = unsafe { allocator.realloc(ptr, layout, to) };
            assert!(!moved.is_null());
            assert_eq!(allocator.0.freed_zeroed.take(), Some(true), "{from} to {to}");
            let kept = unsafe { std::slice::from_raw_parts(moved, from.min(to)) };
            assert!(kept.iter().all(|&b| b == 0xA5), "{from} to {to}");
            unsafe { allocator.dealloc(moved, Layout::from_size_align(to, 8).unwrap()) };
        }
    }

    #[test]
    fn the_wipe_zeroes_the_whole_block_and_nothing_past_it() {
        let mut bytes = [0xA5u8; 64];
        unsafe { wipe(bytes.as_mut_ptr(), 48) };
        assert!(bytes[..48].iter().all(|&b| b == 0));
        assert!(bytes[48..].iter().all(|&b| b == 0xA5));
    }

    /// An allocator with no memory left.
    struct Exhausted;

    unsafe impl GlobalAlloc for Exhausted {
        unsafe fn alloc(&self, _: Layout) -> *mut u8 {
            std::ptr::null_mut()
        }

        unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
    }

    #[cfg(not(windows))]
    #[test]
    fn a_failed_allocation_returns_null_outside_windows() {
        let layout = Layout::from_size_align(64, 8).unwrap();
        assert!(unsafe { Wiping(Exhausted).alloc(layout) }.is_null());
        assert!(unsafe { Wiping(Exhausted).alloc_zeroed(layout) }.is_null());
    }

    /// Set by the parent of [`a_failed_allocation_ends_the_process_on_windows`].
    #[cfg(windows)]
    const EXHAUSTED_CHILD: &str = "HOPLODEX_EXHAUSTED_ALLOCATOR_CHILD";

    /// A failed allocation ends the process with `TerminateProcess`, not
    /// abort's fail-fast, whose exit code would be `STATUS_STACK_BUFFER_OVERRUN`.
    #[cfg(windows)]
    #[test]
    fn a_failed_allocation_ends_the_process_on_windows() {
        if std::env::var_os(EXHAUSTED_CHILD).is_some() {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let _ = unsafe { Wiping(Exhausted).alloc(layout) };
            std::process::exit(0);
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "allocator::tests::a_failed_allocation_ends_the_process_on_windows"])
            .env(EXHAUSTED_CHILD, "1")
            .status()
            .unwrap();
        assert_eq!(status.code().map(|code| code as u32), Some(OUT_OF_MEMORY_EXIT_CODE));
    }
}
