//! Reads the Windows power-throttling policy of a process, for verifying Efficiency mode.

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Report {
    pub pid: u32,
    pub path: String,
    pub eco_qos: bool,
    pub idle_priority: bool,
    pub priority_class: u32,
    pub control_mask: u32,
    pub state_mask: u32,
}

#[cfg(not(windows))]
pub fn inspect(_pid: u32) -> Result<Report> {
    anyhow::bail!("windows-efficiency reads the Windows process policy and only runs on Windows")
}

#[cfg(windows)]
pub fn inspect(pid: u32) -> Result<Report> {
    native::inspect(pid)
}

#[cfg(windows)]
mod native {
    use std::ffi::c_void;

    use anyhow::{Result, bail};

    use super::Report;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_POWER_THROTTLING: i32 = 4;
    const THROTTLING_EXECUTION_SPEED: u32 = 1;
    const IDLE_PRIORITY_CLASS: u32 = 0x40;
    const POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
    const PATH_CAPACITY: usize = 32_768;

    type Handle = *mut c_void;

    #[repr(C)]
    struct PowerThrottlingState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn GetProcessInformation(
            process: Handle,
            information_class: i32,
            information: *mut c_void,
            size: u32,
        ) -> i32;
        fn GetPriorityClass(process: Handle) -> u32;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    struct OwnedHandle(Handle);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful OpenProcess call and is closed once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn image_path(handle: &OwnedHandle) -> String {
        let mut buffer = vec![0u16; PATH_CAPACITY];
        let mut size = PATH_CAPACITY as u32;
        // SAFETY: the buffer holds `size` UTF-16 units and the handle is open for queries.
        let found =
            unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut size) };
        if found == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..size as usize])
    }

    pub fn inspect(pid: u32) -> Result<Report> {
        // SAFETY: OpenProcess has no memory preconditions; a null result is handled below.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            bail!("Unable to open the requested process for read-only inspection");
        }
        let handle = OwnedHandle(raw);
        let mut state = PowerThrottlingState {
            version: POWER_THROTTLING_CURRENT_VERSION,
            control_mask: 0,
            state_mask: 0,
        };
        // SAFETY: `state` is a live 12-byte PROCESS_POWER_THROTTLING_STATE and the size matches.
        let read = unsafe {
            GetProcessInformation(
                handle.0,
                PROCESS_POWER_THROTTLING,
                std::ptr::from_mut(&mut state).cast::<c_void>(),
                size_of::<PowerThrottlingState>() as u32,
            )
        };
        if read == 0 {
            bail!("Windows did not return process power-throttling state");
        }
        // SAFETY: the handle is open for queries.
        let priority_class = unsafe { GetPriorityClass(handle.0) };
        Ok(Report {
            pid,
            path: image_path(&handle),
            eco_qos: state.control_mask & THROTTLING_EXECUTION_SPEED != 0
                && state.state_mask & THROTTLING_EXECUTION_SPEED != 0,
            idle_priority: priority_class == IDLE_PRIORITY_CLASS,
            priority_class,
            control_mask: state.control_mask,
            state_mask: state.state_mask,
        })
    }
}
