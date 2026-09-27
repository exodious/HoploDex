//! Reading an open database's file, ciphertext and all, through SQLite's own
//! file handle (research.md §3, "Reading the open file").
//!
//! On Linux and macOS, closing any file descriptor on a file drops every
//! POSIX lock the process holds on it, SQLite's exclusive lock included.
//! Reading an open database with `std::fs` would therefore release it to
//! another copy of HoploDex the moment the reader closed. SQLite's unix VFS
//! knows this and never closes a descriptor on a locked file, so every read
//! of an open database goes through the descriptor SQLite already holds. On
//! Windows the same path avoids the lock bytes a second handle can't read.

use std::ffi::{c_int, c_void};
use std::io;
use std::marker::PhantomData;

use rusqlite::{ffi, Connection};

/// The main database file of an open connection, read at an offset through
/// the connection's own VFS handle. SQLCipher encrypts above the VFS, so the
/// bytes are exactly what is on disk.
pub struct RawFile<'conn> {
    file: *mut ffi::sqlite3_file,
    _conn: PhantomData<&'conn Connection>,
}

impl<'conn> RawFile<'conn> {
    /// The main database file of `conn`, which must not be used from another
    /// thread while this is alive (the session's mutex sees to that).
    pub fn of(conn: &'conn Connection) -> io::Result<Self> {
        let mut file: *mut ffi::sqlite3_file = std::ptr::null_mut();
        // SAFETY: `conn.handle()` is a live connection for `'conn`, and
        // SQLITE_FCNTL_FILE_POINTER writes one `sqlite3_file*` to the
        // pointer it is given.
        let rc = unsafe {
            ffi::sqlite3_file_control(
                conn.handle(),
                c"main".as_ptr(),
                ffi::SQLITE_FCNTL_FILE_POINTER,
                (&mut file as *mut *mut ffi::sqlite3_file).cast::<c_void>(),
            )
        };
        // SAFETY: a non-null `file` from SQLite points at its open file
        // object, whose `pMethods` is null only while the file is closed.
        if rc != ffi::SQLITE_OK || file.is_null() || unsafe { (*file).pMethods.is_null() } {
            return Err(io::Error::other("the database file is not open"));
        }
        Ok(Self { file, _conn: PhantomData })
    }

    fn methods(&self) -> &ffi::sqlite3_io_methods {
        // SAFETY: checked non-null in `of`, and SQLite keeps it for as long
        // as the connection holds the file open, which outlives `'conn`.
        unsafe { &*(*self.file).pMethods }
    }

    /// The file's length in bytes.
    pub fn size(&self) -> io::Result<u64> {
        let size_of = self.methods().xFileSize.ok_or_else(|| io::Error::other("no xFileSize"))?;
        let mut size: ffi::sqlite3_int64 = 0;
        // SAFETY: `self.file` is the open file these methods belong to.
        let rc = unsafe { size_of(self.file, &mut size) };
        if rc != ffi::SQLITE_OK {
            return Err(io::Error::other(format!("could not size the database file ({rc})")));
        }
        u64::try_from(size).map_err(io::Error::other)
    }

    /// Fills `buf` from `offset`. The caller keeps `offset + buf.len()`
    /// within [`size`](Self::size): SQLite zero-fills a short read.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let read = self.methods().xRead.ok_or_else(|| io::Error::other("no xRead"))?;
        let amount = c_int::try_from(buf.len()).map_err(io::Error::other)?;
        let offset = ffi::sqlite3_int64::try_from(offset).map_err(io::Error::other)?;
        // SAFETY: `buf` is valid for `amount` bytes, and `self.file` is the
        // open file these methods belong to.
        let rc = unsafe { read(self.file, buf.as_mut_ptr().cast::<c_void>(), amount, offset) };
        match rc {
            ffi::SQLITE_OK => Ok(()),
            ffi::SQLITE_IOERR_SHORT_READ => {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the database file is shorter"))
            }
            _ => Err(io::Error::other(format!("could not read the database file ({rc})"))),
        }
    }
}
