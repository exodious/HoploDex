//! Windows: how HoploDex starts the render helper (research.md §11, amended
//! 2026-10-07).
//!
//! HoploDex, not the helper, owns the helper's job object: one job per
//! helper, with `KILL_ON_JOB_CLOSE`, `ACTIVE_PROCESS = 1` and a 2 GiB memory
//! limit. The only handle to it lives in the [`JobChild`], and it is not
//! inheritable, so when HoploDex exits in any way (an exit, a crash,
//! `TerminateProcess`) the kernel closes it and kills the helper at once.
//!
//! The helper is put in the job by `CreateProcessW` itself, through a
//! `PROC_THREAD_ATTRIBUTE_JOB_LIST` attribute, so it is a member before it
//! runs a single instruction: there is no window in which it runs unconfined,
//! as there would be if it were assigned to the job after `Command::spawn`
//! had started it. The same attribute list carries
//! `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, so the helper inherits its three
//! standard handles (stdin and stdout pipes, stderr to `NUL`) and nothing
//! else. The environment is empty, and there is no console window.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::Path;

use windows_sys::Win32::Foundation::{
    FALSE, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation, TRUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject,
};

use super::helper::ARGUMENT;

/// The job's memory limit: 2 GiB, as Linux's `RLIMIT_DATA` (research.md §11).
pub const MEMORY_LIMIT: usize = 2 * 1024 * 1024 * 1024;

/// The helper's exit code when it is ended with [`JobChild::kill`].
const KILLED: u32 = 1;

/// A started helper: its process and the job object it is in. Dropping it
/// closes both handles, which ends the helper if it still runs (the job's
/// `KILL_ON_JOB_CLOSE`).
pub struct JobChild {
    process: OwnedHandle,
    /// Held for the helper's life; closing it, here or by HoploDex's exit,
    /// kills the helper. Declared after `process` so the process handle is
    /// closed first.
    _job: OwnedHandle,
    id: u32,
}

impl JobChild {
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Ends the helper at once. Already gone is not an error here, as in
    /// `std::process::Child::kill`.
    pub fn kill(&mut self) -> io::Result<()> {
        // SAFETY: the process handle is open as long as `self` is.
        if unsafe { TerminateProcess(self.process.as_raw_handle(), KILLED) } == FALSE {
            let err = io::Error::last_os_error();
            // ERROR_ACCESS_DENIED is what a process that has already exited
            // answers.
            return if self.has_exited() { Ok(()) } else { Err(err) };
        }
        Ok(())
    }

    /// Waits for the helper to end and returns its exit code.
    pub fn wait(&mut self) -> io::Result<u32> {
        // SAFETY: the process handle is open as long as `self` is.
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), INFINITE) } != WAIT_OBJECT_0 {
            return Err(io::Error::last_os_error());
        }
        let mut code = 0;
        // SAFETY: a pointer to a local.
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == FALSE {
            return Err(io::Error::last_os_error());
        }
        Ok(code)
    }

    fn has_exited(&self) -> bool {
        // SAFETY: the process handle is open as long as `self` is.
        unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) == WAIT_OBJECT_0 }
    }
}

/// Starts `exe --render-helper <args>` into a new job object of its own, with
/// a pipe for its stdin and one for its stdout. `args` are for tests (the
/// self-check); the app passes none. The pipe ends are plain `File`s, which
/// read and write synchronously: `ChildStdin` and `ChildStdout` wait on
/// overlapped I/O, which a `CreatePipe` handle does not do (a read would
/// never return).
pub fn spawn(exe: &Path, args: &[&OsStr]) -> io::Result<(JobChild, File, File)> {
    let job = job()?;

    // The helper's ends are inheritable, ours are not.
    let (stdin_read, stdin_write) = pipe(false)?;
    let (stdout_read, stdout_write) = pipe(true)?;
    let null = OpenOptions::new().write(true).open("NUL")?;
    inheritable(null.as_raw_handle(), true)?;

    let mut command_line = Vec::new();
    for (i, arg) in std::iter::once(exe.as_os_str())
        .chain([OsStr::new(ARGUMENT)])
        .chain(args.iter().copied())
        .enumerate()
    {
        if i > 0 {
            command_line.push(u16::from(b' '));
        }
        quote(arg, &mut command_line);
    }
    command_line.push(0);
    let application: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    // An empty environment block is two NULs.
    let environment = [0u16; 2];

    let child_ends: [HANDLE; 3] =
        [stdin_read.as_raw_handle(), stdout_write.as_raw_handle(), null.as_raw_handle()];
    let job_handle: HANDLE = job.as_raw_handle();

    let mut attributes = AttributeList::new(2)?;
    attributes.set(
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
        child_ends.as_ptr().cast(),
        size_of::<[HANDLE; 3]>(),
    )?;
    attributes.set(
        PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
        std::ptr::from_ref(&job_handle).cast(),
        size_of::<HANDLE>(),
    )?;

    // SAFETY: a zeroed structure of plain data, then the fields that matter.
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = child_ends[0];
    startup.StartupInfo.hStdOutput = child_ends[1];
    startup.StartupInfo.hStdError = child_ends[2];
    startup.lpAttributeList = attributes.as_ptr();

    // SAFETY: a zeroed structure the call fills in.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the buffers outlive the call, the command line is writable and
    // NUL-terminated, the startup structure is an `STARTUPINFOEXW` as
    // `EXTENDED_STARTUPINFO_PRESENT` says, and every handle in the attribute
    // list is open and inheritable.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            TRUE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            std::ptr::null(),
            std::ptr::from_ref(&startup.StartupInfo),
            &mut info,
        )
    };
    if created == FALSE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both handles are new and ours.
    let (process, thread) = unsafe {
        (OwnedHandle::from_raw_handle(info.hProcess), OwnedHandle::from_raw_handle(info.hThread))
    };
    drop(thread);
    // Not ours to keep: the helper has its own copies of these.
    drop((stdin_read, stdout_write, null));
    drop(attributes);

    Ok((
        JobChild { process, _job: job, id: info.dwProcessId },
        File::from(stdin_write),
        File::from(stdout_read),
    ))
}

/// A job object holding the limits, and no inheritable handle to itself.
fn job() -> io::Result<OwnedHandle> {
    // SAFETY: plain system calls; the structure is zeroed and `size_of` is
    // its own size.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = OwnedHandle::from_raw_handle(job);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.JobMemoryLimit = MEMORY_LIMIT;
        if SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == FALSE
        {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }
}

/// An anonymous pipe, read end then write end. With `child_writes` the write
/// end is the helper's (its stdout) and the read end ours; otherwise the
/// read end is the helper's (its stdin). Only the helper's end can be
/// inherited.
fn pipe(child_writes: bool) -> io::Result<(OwnedHandle, OwnedHandle)> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: TRUE,
    };
    let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: two pointers to locals and one to the attributes.
    if unsafe { CreatePipe(&mut read, &mut write, &raw const attributes, 0) } == FALSE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both handles are new and ours.
    let (read, write) =
        unsafe { (OwnedHandle::from_raw_handle(read), OwnedHandle::from_raw_handle(write)) };
    inheritable(if child_writes { read.as_raw_handle() } else { write.as_raw_handle() }, false)?;
    Ok((read, write))
}

fn inheritable(handle: RawHandle, on: bool) -> io::Result<()> {
    // SAFETY: the caller's handle is open.
    let set = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, u32::from(on)) };
    if set == FALSE { Err(io::Error::last_os_error()) } else { Ok(()) }
}

/// A `PROC_THREAD_ATTRIBUTE_LIST`.
struct AttributeList {
    /// `u64`s, for the list's alignment.
    buffer: Vec<u64>,
}

impl AttributeList {
    fn new(count: u32) -> io::Result<Self> {
        let mut size = 0usize;
        // SAFETY: the first call only reports the size it needs, and fails.
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), count, 0, &mut size) };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut list = Self { buffer: vec![0u64; size.div_ceil(size_of::<u64>())] };
        // SAFETY: the buffer is at least `size` bytes and aligned.
        if unsafe { InitializeProcThreadAttributeList(list.as_ptr(), count, 0, &mut size) } == FALSE
        {
            list.buffer.clear();
            return Err(io::Error::last_os_error());
        }
        Ok(list)
    }

    fn as_ptr(&self) -> *mut core::ffi::c_void {
        self.buffer.as_ptr().cast_mut().cast()
    }

    /// The value must stay valid until the process is created.
    fn set(
        &mut self,
        attribute: usize,
        value: *const core::ffi::c_void,
        size: usize,
    ) -> io::Result<()> {
        // SAFETY: the list is initialized; the caller keeps `value` alive.
        let updated = unsafe {
            UpdateProcThreadAttribute(
                self.as_ptr(),
                0,
                attribute,
                value,
                size,
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if updated == FALSE { Err(io::Error::last_os_error()) } else { Ok(()) }
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            // SAFETY: the list was initialized by `new`.
            unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
        }
    }
}

/// Appends `arg` as one argument of a command line, quoted the way
/// `CommandLineToArgvW` (and the C runtime) reads it back.
fn quote(arg: &OsStr, out: &mut Vec<u16>) {
    let wide: Vec<u16> = arg.encode_wide().collect();
    let plain =
        !wide.is_empty() && !wide.iter().any(|c| matches!(*c, 0x20 | 0x09 | 0x0a | 0x0b | 0x22));
    if plain {
        out.extend(wide);
        return;
    }
    out.push(u16::from(b'"'));
    let mut backslashes = 0usize;
    for c in wide {
        if c == u16::from(b'\\') {
            backslashes += 1;
            continue;
        }
        let extra = if c == u16::from(b'"') { backslashes + 1 } else { 0 };
        out.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes + extra));
        backslashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2));
    out.push(u16::from(b'"'));
}
