//! Single-instance lock with takeover, and child processes that cannot outlive the daemon.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
#[cfg(all(unix, not(target_os = "linux")))]
use std::process::Stdio;
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use crate::config;
use crate::util::write_json_pretty_atomic;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const TAKEOVER_SOFT_TIMEOUT: Duration = Duration::from_secs(4);
const TAKEOVER_HARD_TIMEOUT: Duration = Duration::from_secs(3);
const TAKEOVER_RETRY_INTERVAL: Duration = Duration::from_millis(120);

pub enum AcquireState {
    Acquired(InstanceGuard),
    AlreadyRunning { pid: Option<u32> },
}

pub struct AcquireWithTakeover {
    pub guard: InstanceGuard,
    pub takeover_pid: Option<u32>,
}

pub enum RunningState {
    NotRunning,
    Running { pid: Option<u32> },
}

pub struct InstanceGuard {
    file: File,
    meta_path: PathBuf,
    pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstanceMetadata {
    pid: u32,
    exe_path: Option<String>,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        remove_instance_metadata_if_owned(self.pid, &self.meta_path);
    }
}

pub fn acquire_or_takeover_single_instance() -> Result<AcquireWithTakeover> {
    match acquire_single_instance()? {
        AcquireState::Acquired(guard) => Ok(AcquireWithTakeover {
            guard,
            takeover_pid: None,
        }),
        AcquireState::AlreadyRunning { pid } => {
            let Some(existing_pid) = pid else {
                bail!(
                    "codex-discord-presence is already running, but PID metadata is unavailable for takeover"
                );
            };
            if existing_pid == std::process::id() {
                bail!(
                    "codex-discord-presence lock appears to be owned by current process; close the existing process and retry"
                );
            }
            let exe_path = read_instance_metadata(&config::instance_meta_path())
                .ok()
                .flatten()
                .filter(|metadata| metadata.pid == existing_pid)
                .and_then(|metadata| metadata.exe_path);
            if process_exists(existing_pid)
                && !takeover_target_matches(existing_pid, exe_path.as_deref())
            {
                bail!(
                    "codex-discord-presence lock is held, but PID {existing_pid} does not match the recorded executable; refusing to terminate it"
                );
            }

            terminate_process(existing_pid, false)?;
            if let Some(guard) = wait_for_lock(TAKEOVER_SOFT_TIMEOUT)? {
                return Ok(AcquireWithTakeover {
                    guard,
                    takeover_pid: Some(existing_pid),
                });
            }

            terminate_process(existing_pid, true)?;
            if let Some(guard) = wait_for_lock(TAKEOVER_HARD_TIMEOUT)? {
                return Ok(AcquireWithTakeover {
                    guard,
                    takeover_pid: Some(existing_pid),
                });
            }

            bail!(
                "failed to acquire single-instance lock after takeover attempt (PID {existing_pid})"
            );
        }
    }
}

pub fn acquire_single_instance() -> Result<AcquireState> {
    let lock_path = config::lock_path();
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create lock directory {}", parent.display()))?;
    }

    let meta_path = config::instance_meta_path();
    let mut file = open_lock_file(&lock_path)?;
    match file.try_lock() {
        Ok(()) => {
            write_pid_compat(&mut file)?;
            let metadata = InstanceMetadata {
                pid: std::process::id(),
                exe_path: env::current_exe()
                    .ok()
                    .map(|path| path.display().to_string()),
            };
            write_instance_metadata(&meta_path, &metadata)?;
            Ok(AcquireState::Acquired(InstanceGuard {
                file,
                meta_path,
                pid: metadata.pid,
            }))
        }
        Err(_) => {
            let pid = read_instance_metadata(&meta_path)
                .ok()
                .flatten()
                .map(|m| m.pid);
            Ok(AcquireState::AlreadyRunning { pid })
        }
    }
}

pub fn inspect_running_instance() -> Result<RunningState> {
    let lock_path = config::lock_path();
    if !lock_path.exists() {
        return Ok(RunningState::NotRunning);
    }

    let meta_path = config::instance_meta_path();
    let file = open_lock_file(&lock_path)?;
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            let _ = fs::remove_file(&meta_path);
            Ok(RunningState::NotRunning)
        }
        Err(_) => {
            let pid = read_instance_metadata(&meta_path)
                .ok()
                .flatten()
                .map(|m| m.pid);
            Ok(RunningState::Running { pid })
        }
    }
}

fn wait_for_lock(timeout: Duration) -> Result<Option<InstanceGuard>> {
    let deadline = Instant::now() + timeout;
    loop {
        match acquire_single_instance()? {
            AcquireState::Acquired(guard) => return Ok(Some(guard)),
            AcquireState::AlreadyRunning { .. } => {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                thread::sleep(TAKEOVER_RETRY_INTERVAL);
            }
        }
    }
}

fn takeover_target_matches(pid: u32, exe_path: Option<&str>) -> bool {
    let Some(expected) = exe_path.map(Path::new) else {
        return false;
    };
    process_image_path(pid).is_some_and(|actual| same_executable(&actual, expected))
}

fn same_executable(actual: &Path, expected: &Path) -> bool {
    let normalize = |path: &Path| {
        let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let text = resolved.display().to_string();
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text).to_string();
        let text = text.strip_suffix(" (deleted)").unwrap_or(&text).to_string();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    normalize(actual) == normalize(expected)
}

#[cfg(windows)]
fn terminate_process(pid: u32, _force: bool) -> Result<()> {
    if !process_exists(pid) {
        return Ok(());
    }
    if let Err(error) = win32::terminate(pid)
        && process_exists(pid)
    {
        bail!("failed to terminate running instance PID {pid}: {error}");
    }
    Ok(())
}

#[cfg(unix)]
fn terminate_process(pid: u32, force: bool) -> Result<()> {
    if !process_exists(pid) {
        return Ok(());
    }
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    if let Err(error) = signal_process(pid, signal)
        && process_exists(pid)
    {
        bail!("failed to terminate running instance PID {pid}: {error}");
    }
    Ok(())
}

// Pids 0 and below address whole process groups in kill(2), so they never name one process.
#[cfg(unix)]
fn signal_process(pid: u32, signal: libc::c_int) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: kill takes two integers and touches no memory of this process.
    if unsafe { libc::kill(pid, signal) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    win32::process_exists(pid)
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    match signal_process(pid, 0) {
        Ok(()) => true,
        Err(error) => error.raw_os_error() == Some(libc::EPERM),
    }
}

#[cfg(windows)]
fn process_image_path(pid: u32) -> Option<PathBuf> {
    win32::image_path(pid)
}

#[cfg(target_os = "linux")]
fn process_image_path(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_image_path(pid: u32) -> Option<PathBuf> {
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("comm=")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !path.is_empty()).then(|| PathBuf::from(path))
}

pub struct ChildProcessGuard {
    child: Child,
    #[cfg(windows)]
    _job: Option<win32::KillOnCloseJob>,
}

impl ChildProcessGuard {
    /// Spawns `command` so the child cannot outlive this process.
    ///
    /// On Linux the kernel sends the child SIGTERM when the spawning thread exits,
    /// so call this from a thread that lives as long as the process. Windows uses a
    /// kill-on-close job object. Other Unix systems rely on `Drop` alone.
    pub fn spawn(command: &mut Command) -> std::io::Result<Self> {
        die_with_parent(command);
        command.spawn().map(Self::new)
    }

    pub fn new(child: Child) -> Self {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let job = win32::KillOnCloseJob::assign(child.as_raw_handle())
                .inspect_err(|error| {
                    tracing::warn!(%error, "codex child could not be placed in a job object");
                })
                .ok();
            Self { child, _job: job }
        }
        #[cfg(unix)]
        {
            Self { child }
        }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}

impl Drop for ChildProcessGuard {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

// The release profile uses panic = "abort", so a crash never runs `Drop`; the kernel has to reap the child.
#[cfg(target_os = "linux")]
fn die_with_parent(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    let parent = std::process::id() as libc::pid_t;
    // SAFETY: the hook runs between fork and exec, where only async-signal-safe calls are
    // allowed. prctl and getppid are, and the hook allocates nothing.
    unsafe {
        command.pre_exec(move || {
            let signal = libc::SIGTERM as libc::c_ulong;
            let unused: libc::c_ulong = 0;
            if libc::prctl(libc::PR_SET_PDEATHSIG, signal, unused, unused, unused) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            // A parent that exited before the prctl call above never delivers the signal.
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn die_with_parent(_command: &mut Command) {}

#[cfg(windows)]
mod win32 {
    use std::ffi::{OsString, c_void};
    use std::io;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::RawHandle;
    use std::path::PathBuf;

    type Handle = *mut c_void;

    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const MAX_IMAGE_PATH: usize = 32_768;

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    pub(super) struct ExtendedLimitInformation {
        basic: BasicLimitInformation,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
        fn GetExitCodeProcess(process: Handle, exit_code: *mut u32) -> i32;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            class: i32,
            info: *const c_void,
            length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    }

    struct OwnedHandle(Handle);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn open(pid: u32, access: u32) -> io::Result<OwnedHandle> {
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(OwnedHandle(handle))
        }
    }

    pub(super) fn process_exists(pid: u32) -> bool {
        let process = match open(pid, PROCESS_QUERY_LIMITED_INFORMATION) {
            Ok(process) => process,
            Err(error) => return error.raw_os_error() == Some(ERROR_ACCESS_DENIED),
        };
        let mut exit_code = 0u32;
        unsafe { GetExitCodeProcess(process.0, &mut exit_code) != 0 && exit_code == STILL_ACTIVE }
    }

    pub(super) fn image_path(pid: u32) -> Option<PathBuf> {
        let process = open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?;
        let mut buffer = vec![0u16; MAX_IMAGE_PATH];
        let mut size = MAX_IMAGE_PATH as u32;
        if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut size) } == 0
        {
            return None;
        }
        buffer.truncate(size as usize);
        Some(PathBuf::from(OsString::from_wide(&buffer)))
    }

    pub(super) fn terminate(pid: u32) -> io::Result<()> {
        let process = open(pid, PROCESS_TERMINATE)?;
        if unsafe { TerminateProcess(process.0, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) struct KillOnCloseJob {
        _handle: OwnedHandle,
    }

    impl KillOnCloseJob {
        pub(super) fn assign(process: RawHandle) -> io::Result<Self> {
            let job = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = OwnedHandle(job);
            let mut limits = ExtendedLimitInformation::default();
            limits.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if unsafe {
                SetInformationJobObject(
                    job.0,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    std::ptr::from_ref(&limits).cast(),
                    std::mem::size_of::<ExtendedLimitInformation>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if unsafe { AssignProcessToJobObject(job.0, process.cast()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { _handle: job })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn job_limit_layout_matches_the_sdk() {
            let expected = if cfg!(target_pointer_width = "64") {
                144
            } else {
                112
            };
            assert_eq!(std::mem::size_of::<ExtendedLimitInformation>(), expected);
        }
    }
}

fn open_lock_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open lock file {}", path.display()))
}

fn write_pid_compat(file: &mut File) -> Result<()> {
    let pid = std::process::id();
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(pid.to_string().as_bytes())?;
    file.flush()?;
    Ok(())
}

fn write_instance_metadata(path: &Path, metadata: &InstanceMetadata) -> Result<()> {
    write_json_pretty_atomic(path, metadata)
        .with_context(|| format!("failed to write instance metadata {}", path.display()))?;
    Ok(())
}

fn read_instance_metadata(path: &Path) -> Result<Option<InstanceMetadata>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read instance metadata {}", path.display()))?;
    let parsed: InstanceMetadata = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    Ok(Some(parsed))
}

fn remove_instance_metadata_if_owned(expected_pid: u32, path: &Path) {
    let Ok(Some(metadata)) = read_instance_metadata(path) else {
        return;
    };
    if metadata.pid == expected_pid {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tempfile::TempDir;

    fn long_running_command() -> std::process::Command {
        if cfg!(windows) {
            let mut command = crate::util::silent_command("ping");
            command.args(["-n", "30", "127.0.0.1"]);
            command
        } else {
            let mut command = crate::util::silent_command("sleep");
            command.arg("30");
            command
        }
    }

    #[test]
    fn takeover_target_requires_matching_image_path() {
        let pid = std::process::id();
        let exe = env::current_exe().expect("current exe");
        assert!(takeover_target_matches(
            pid,
            Some(&exe.display().to_string())
        ));
        assert!(!takeover_target_matches(
            pid,
            Some(
                &exe.with_file_name("unrelated-binary.exe")
                    .display()
                    .to_string()
            )
        ));
        assert!(!takeover_target_matches(pid, None));
    }

    #[test]
    fn native_process_queries_track_a_child_lifecycle() {
        let mut child = long_running_command()
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");
        let pid = child.id();
        assert!(process_exists(pid));
        assert!(process_image_path(pid).is_some());
        terminate_process(pid, true).expect("terminate");
        child.wait().expect("reap");
        assert!(!process_exists(pid));
    }

    #[test]
    fn child_guard_kills_and_reaps_on_drop() {
        let mut command = long_running_command();
        command.stdout(Stdio::null());
        let guard = ChildProcessGuard::spawn(&mut command).expect("spawn");
        let pid = guard.id();
        assert!(process_exists(pid));
        drop(guard);
        assert!(!process_exists(pid));
    }

    #[test]
    fn liveness_probe_sees_this_process_and_not_a_reaped_child() {
        assert!(process_exists(std::process::id()));

        let mut child = long_running_command()
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");
        let pid = child.id();
        child.kill().expect("kill");
        child.wait().expect("reap");
        assert!(!process_exists(pid), "pid {pid} was reaped");
    }

    #[cfg(unix)]
    #[test]
    fn liveness_probe_never_addresses_a_process_group() {
        assert!(!process_exists(0));
        assert!(!process_exists(u32::MAX));
        assert!(terminate_process(0, true).is_ok());
    }

    #[cfg(target_os = "linux")]
    const PDEATHSIG_HELPER_ENV: &str = "CDRP_TEST_PDEATHSIG_HELPER";
    #[cfg(target_os = "linux")]
    const PDEATHSIG_HELPER_TEST: &str =
        "process_guard::tests::pdeathsig_helper_spawns_a_child_and_exits";
    #[cfg(target_os = "linux")]
    const PDEATHSIG_CHILD_PREFIX: &str = "pdeathsig-child-pid=";

    #[cfg(target_os = "linux")]
    #[test]
    fn pdeathsig_helper_spawns_a_child_and_exits() {
        if env::var_os(PDEATHSIG_HELPER_ENV).is_none() {
            return;
        }
        let mut command = long_running_command();
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let guard = ChildProcessGuard::spawn(&mut command).expect("spawn");
        println!("{PDEATHSIG_CHILD_PREFIX}{}", guard.id());
        std::process::exit(0);
    }

    #[cfg(target_os = "linux")]
    fn process_is_gone(pid: u32) -> bool {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => stat
                .rsplit_once(')')
                .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn guarded_child_dies_with_its_parent_when_drop_never_runs() {
        let output = Command::new(env::current_exe().expect("current exe"))
            .args(["--exact", PDEATHSIG_HELPER_TEST, "--nocapture"])
            .env(PDEATHSIG_HELPER_ENV, "1")
            .output()
            .expect("run helper process");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "helper failed: {stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let pid: u32 = stdout
            .lines()
            .find_map(|line| line.strip_prefix(PDEATHSIG_CHILD_PREFIX))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or_else(|| panic!("helper printed no child pid: {stdout}"));

        let deadline = Instant::now() + Duration::from_secs(5);
        while !process_is_gone(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        let still_running = !process_is_gone(pid);
        if still_running {
            let _ = signal_process(pid, libc::SIGKILL);
        }
        assert!(!still_running, "child {pid} outlived its parent");
    }

    #[test]
    fn roundtrip_instance_metadata() {
        let tmp = TempDir::new().expect("temp dir");
        let path = tmp.path().join("instance.json");

        let original = InstanceMetadata {
            pid: 1234,
            exe_path: Some("codex-discord-presence".to_string()),
        };

        write_instance_metadata(&path, &original).expect("write metadata");
        let loaded = read_instance_metadata(&path)
            .expect("read metadata")
            .expect("metadata value");

        assert_eq!(loaded.pid, 1234);
        assert_eq!(loaded.exe_path.as_deref(), Some("codex-discord-presence"));
    }

    #[test]
    fn inspect_reports_running_while_lock_is_held() {
        let _mutex = crate::config::test_env_lock();
        let tmp = TempDir::new().expect("temp dir");
        unsafe {
            env::set_var("CODEX_PRESENCE_HOME", tmp.path());
        }

        let state = acquire_single_instance().expect("acquire instance");
        let guard = match state {
            AcquireState::Acquired(guard) => guard,
            AcquireState::AlreadyRunning { .. } => panic!("expected acquired lock"),
        };

        match inspect_running_instance().expect("inspect running") {
            RunningState::Running { .. } => {}
            RunningState::NotRunning => panic!("expected running state while lock is held"),
        }

        drop(guard);

        match inspect_running_instance().expect("inspect stopped") {
            RunningState::NotRunning => {}
            RunningState::Running { .. } => panic!("expected not running after lock release"),
        }

        unsafe {
            env::remove_var("CODEX_PRESENCE_HOME");
        }
    }
}
