//! Windows: how HoploDex starts a confined helper (007 research.md §11,
//! amended 2026-10-07; 008 research.md §8).
//!
//! HoploDex, not the helper, owns the helper's job object: one job per
//! helper, with `KILL_ON_JOB_CLOSE`, `ACTIVE_PROCESS = 1` and a memory
//! limit the caller names (2 GiB for the TIFF helper). The only handle to it
//! lives in the [`JobChild`], and it is not inheritable, so when HoploDex exits in any way (an exit, a crash,
//! `TerminateProcess`) the kernel closes it and kills the helper at once.
//!
//! The helper is put in the job by `CreateProcessW` itself, through a
//! `PROC_THREAD_ATTRIBUTE_JOB_LIST` attribute, so it is a member before it
//! runs a single instruction: there is no window in which it runs unconfined,
//! as there would be if it were assigned to the job after `Command::spawn`
//! had started it. The same attribute list carries
//! `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, so the helper inherits its three
//! standard handles (stdin and stdout pipes, stderr to `NUL`) and nothing
//! else. There is no console window.
//!
//! It also carries `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`: the helper
//! runs in an AppContainer with no capabilities, so from its first instruction
//! it has no network, none of the user's files, no other process of the user's
//! and none of their credentials. The only environment variable given is
//! `LOCALAPPDATA`, which `CreateProcessW` needs to start an AppContainer (it
//! points it and `TEMP` and `TMP` at the container's own folder). There
//! is no fallback: if the container can't start, the helper doesn't run and no
//! document is read. That includes a session with no desktop, such as SSH's
//! session 0, where the helper dies at start with `0xC0000142`.
//!
//! Each helper has a container of its own, so one helper can't open another,
//! which may hold another document or another database's. Its profile (a
//! per-user folder and registry key) is the one place it can write, nothing the
//! helpers run writes there. It is deleted when the [`JobChild`] is dropped, which
//! waits a bounded time and lets the delete finish behind it. If HoploDex ended
//! first or the delete failed, the next start's [`sweep_profiles`] deletes it.

use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INVALID_PARAMETER, FALSE, GetLastError, HANDLE, HANDLE_FLAG_INHERIT, S_OK,
    SetHandleInformation, TRUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Globalization::lstrlenW;
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{FreeSid, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_UILIMIT_DESKTOP,
    JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
    JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES, JOB_OBJECT_UILIMIT_READCLIPBOARD,
    JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS, JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    OpenProcess, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};

/// The helper's exit code when it is ended with [`JobChild::kill`].
const KILLED: u32 = 1;

/// What the name of each helper's AppContainer starts with.
const CONTAINER: &str = "HoploDex.Helper";
/// The longest a drop waits for a profile's delete, which then goes on behind it.
const DELETE_WAIT: Duration = Duration::from_millis(500);

/// A started helper: its process and the job object it is in. Dropping it
/// closes both handles, which ends the helper if it still runs (the job's
/// `KILL_ON_JOB_CLOSE`).
pub struct JobChild {
    process: OwnedHandle,
    /// Held for the helper's life; closing it, here or by HoploDex's exit,
    /// kills the helper. Declared after `process` so the process handle is
    /// closed first.
    _job: OwnedHandle,
    /// Declared after `_job` so the profile goes after the helper.
    container: Container,
    id: u32,
}

impl JobChild {
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The name of its AppContainer, which is its profile folder's in
    /// `%LOCALAPPDATA%\Packages`.
    pub fn container(&self) -> &str {
        &self.container.name
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

/// Starts `exe <mode> <args>` (`mode` being the helper's argument, such as
/// `--render-helper`) in a new AppContainer, and into a new job object of its own
/// with `memory_limit` bytes (as Linux's `RLIMIT_DATA`), and a pipe for its stdin and one for its
/// stdout. `args` are for tests (the self-check); the app passes none. The pipe ends are plain `File`s, which
/// read and write synchronously: `ChildStdin` and `ChildStdout` wait on
/// overlapped I/O, which a `CreatePipe` handle does not do (a read would
/// never return).
pub fn spawn(
    exe: &Path,
    mode: &str,
    memory_limit: usize,
    args: &[&OsStr],
) -> io::Result<(JobChild, File, File)> {
    let job = job(memory_limit)?;

    // The helper's ends are inheritable, ours are not.
    let (stdin_read, stdin_write) = pipe(false)?;
    let (stdout_read, stdout_write) = pipe(true)?;
    let null = OpenOptions::new().write(true).open("NUL")?;
    inheritable(null.as_raw_handle(), true)?;

    let mut command_line = Vec::new();
    for (i, arg) in std::iter::once(exe.as_os_str())
        .chain([OsStr::new(mode)])
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
    let environment = environment(std::env::var_os("LOCALAPPDATA"))?;
    let (container, sid) = container()?;
    let capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: sid.0,
        Capabilities: std::ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };

    let child_ends: [HANDLE; 3] =
        [stdin_read.as_raw_handle(), stdout_write.as_raw_handle(), null.as_raw_handle()];
    let job_handle: HANDLE = job.as_raw_handle();

    let mut attributes = AttributeList::new(3)?;
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
    attributes.set(
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        std::ptr::from_ref(&capabilities).cast(),
        size_of::<SECURITY_CAPABILITIES>(),
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
        JobChild { process, _job: job, container, id: info.dwProcessId },
        File::from(stdin_write),
        File::from(stdout_read),
    ))
}

/// The helper's environment block: `LOCALAPPDATA` alone, which `CreateProcessW`
/// needs for an AppContainer (error 203 without it). A parent without it can't
/// start a helper.
fn environment(local_app_data: Option<std::ffi::OsString>) -> io::Result<Vec<u16>> {
    let value = local_app_data
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
    let mut block: Vec<u16> = OsStr::new("LOCALAPPDATA=").encode_wide().collect();
    block.extend(value.encode_wide());
    block.extend([0, 0]);
    Ok(block)
}

/// A SID from `CreateAppContainerProfile`, freed when dropped.
struct ContainerSid(PSID);

impl Drop for ContainerSid {
    fn drop(&mut self) {
        // SAFETY: the SID came from that call, and is freed once.
        unsafe { FreeSid(self.0) };
    }
}

/// One helper's AppContainer. Dropped, it deletes the profile, waiting
/// [`DELETE_WAIT`] at most: this runs while a database closes or locks, and how
/// long the delete takes depends on what the helper left there.
struct Container {
    name: String,
}

impl Drop for Container {
    fn drop(&mut self) {
        let name = std::mem::take(&mut self.name);
        wait_at_most(DELETE_WAIT, move || delete_profile(&name));
    }
}

/// Runs `work` on a thread of its own and waits for it, up to `limit`. If no
/// thread can start, it doesn't run (the next start's sweep has a profile to find).
fn wait_at_most(limit: Duration, work: impl FnOnce() + Send + 'static) {
    let (done, finished) = mpsc::channel::<()>();
    let started = thread::Builder::new().name("helper-profile".into()).spawn(move || {
        work();
        let _ = done.send(());
    });
    match started {
        Ok(_) => {
            let _ = finished.recv_timeout(limit);
        }
        Err(err) => log::warn!("could not start deleting a helper's profile: {err}"),
    }
}

fn delete_profile(name: &str) {
    // SAFETY: the name is NUL-terminated and outlives the call.
    let deleted = unsafe { DeleteAppContainerProfile(wide(name).as_ptr()) };
    if deleted != 0 {
        log::warn!("could not delete a helper's AppContainer profile: {deleted:#x}");
    }
}

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain([0]).collect()
}

/// Deletes the profiles of helpers whose HoploDex has gone: one that crashed or
/// was killed never reached its [`Container`]'s drop, and a delete can fail or be
/// cut short by an exit. Run once at start-up, off the main thread. A profile
/// whose process id has been reused by something else waits for a later start.
pub fn sweep_profiles() {
    let Some(packages) = packages_folder() else { return };
    sweep_profiles_in(&packages);
}

/// Where the OS keeps AppContainer profiles: the user's own Local AppData, not
/// whatever this process's environment names.
fn packages_folder() -> Option<PathBuf> {
    let mut path = std::ptr::null_mut();
    // SAFETY: a pointer to a local; what it sets is NUL-terminated on success,
    // and is freed once, as it must be even on failure.
    unsafe {
        let found =
            SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut path);
        let folder = (found == S_OK).then(|| {
            let wide = std::slice::from_raw_parts(path, lstrlenW(path) as usize);
            PathBuf::from(OsString::from_wide(wide)).join("Packages")
        });
        CoTaskMemFree(path.cast());
        folder
    }
}

fn sweep_profiles_in(packages: &Path) {
    let Ok(entries) = std::fs::read_dir(packages) else { return };
    let prefix = format!("{}.", CONTAINER.to_lowercase());
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((process, counter)) =
            name.strip_prefix(&prefix).and_then(|rest| rest.split_once('.'))
        else {
            continue;
        };
        if counter.parse::<u32>().is_ok() && process.parse().is_ok_and(|id| !process_is_running(id))
        {
            delete_profile(name);
        }
    }
}

fn process_is_running(id: u32) -> bool {
    // SAFETY: plain system calls; a handle it returns is closed once.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, id);
        if process.is_null() {
            // Access denied is someone's running process. Only no such process isn't.
            return GetLastError() != ERROR_INVALID_PARAMETER;
        }
        CloseHandle(process);
        true
    }
}

/// A new AppContainer with no capabilities for one helper.
fn container() -> io::Result<(Container, ContainerSid)> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name =
        format!("{CONTAINER}.{}.{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    let wide_name = wide(&name);
    let mut sid: PSID = std::ptr::null_mut();
    // SAFETY: NUL-terminated strings that outlive the calls, no capabilities,
    // and a pointer to a local. Deleting first clears one left by a killed
    // HoploDex that had this process id.
    let answer = unsafe {
        DeleteAppContainerProfile(wide_name.as_ptr());
        CreateAppContainerProfile(
            wide_name.as_ptr(),
            wide_name.as_ptr(),
            wide_name.as_ptr(),
            std::ptr::null(),
            0,
            &mut sid,
        )
    };
    if answer != 0 {
        return Err(io::Error::from_raw_os_error(answer));
    }
    Ok((Container { name }, ContainerSid(sid)))
}

/// A job object holding the limits, and no inheritable handle to itself.
fn job(memory_limit: usize) -> io::Result<OwnedHandle> {
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
        limits.JobMemoryLimit = memory_limit;
        if SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == FALSE
        {
            return Err(io::Error::last_os_error());
        }
        let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_DESKTOP
                | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                | JOB_OBJECT_UILIMIT_EXITWINDOWS
                | JOB_OBJECT_UILIMIT_GLOBALATOMS
                | JOB_OBJECT_UILIMIT_HANDLES
                | JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
        };
        if SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectBasicUIRestrictions,
            std::ptr::from_ref(&ui).cast(),
            size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
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

#[cfg(test)]
mod tests {
    use super::{
        CONTAINER, delete_profile, environment, packages_folder, sweep_profiles, wait_at_most, wide,
    };
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Security::{FreeSid, Isolation::CreateAppContainerProfile};

    /// A real profile named as a helper's of process `id`, and its folder.
    fn profile(id: u32) -> (String, std::path::PathBuf) {
        let name = format!("{CONTAINER}.{id}.999");
        // One an interrupted run left.
        delete_profile(&name);
        let mut sid = std::ptr::null_mut();
        let wide = wide(&name);
        // SAFETY: NUL-terminated strings that outlive the call, no capabilities,
        // and a pointer to a local; the SID it returns is freed once.
        unsafe {
            let made = CreateAppContainerProfile(
                wide.as_ptr(),
                wide.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            );
            assert_eq!(made, 0, "the profile wasn't made");
            FreeSid(sid);
        }
        let folder = packages_folder().unwrap().join(name.to_lowercase());
        (name, folder)
    }

    /// A delete that takes long, as a hostile helper can make it, doesn't hold up
    /// the close or lock it runs in, and still finishes.
    #[test]
    fn a_slow_delete_is_not_waited_for_past_the_limit() {
        let (done, finished) = std::sync::mpsc::channel();
        let start = Instant::now();
        wait_at_most(Duration::from_millis(50), move || {
            std::thread::sleep(Duration::from_secs(3));
            done.send(()).unwrap();
        });
        assert!(start.elapsed() < Duration::from_secs(2), "waited {:?}", start.elapsed());
        finished.recv_timeout(Duration::from_secs(30)).expect("the work didn't finish");
    }

    /// A crashed HoploDex's profile goes at the next start; a running one's stays.
    #[test]
    fn the_sweep_deletes_only_the_profiles_of_processes_that_have_gone() {
        // Process ids are multiples of 4, so no process has this one.
        let (_, gone) = profile(4_294_967_291);
        let (running_name, running) = profile(std::process::id());

        sweep_profiles();

        let (gone_there, running_there) = (gone.exists(), running.exists());
        delete_profile(&running_name);
        assert!(!gone_there, "{} is still there", gone.display());
        assert!(running_there, "the sweep deleted a running HoploDex's profile");
    }

    /// Without `LOCALAPPDATA` no AppContainer can start, so no helper is tried.
    #[test]
    fn the_environment_block_is_local_app_data_alone_or_nothing() {
        assert!(environment(None).is_err());
        let block = environment(Some(r"C:\x".into())).unwrap();
        assert_eq!(String::from_utf16(&block).unwrap(), "LOCALAPPDATA=C:\\x\0\0");
    }
}
