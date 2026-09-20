pub fn requested(value: Option<&str>) -> bool {
    !value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "disabled"
        )
    })
}

#[cfg(windows)]
pub fn apply_from_env(name: &str) {
    if requested(std::env::var(name).ok().as_deref())
        && let Err(error) = windows::enable()
    {
        tracing::warn!(%error, "Windows efficiency mode could not be enabled");
    }
}

#[cfg(not(windows))]
pub fn apply_from_env(_name: &str) {}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;

    const PROCESS_POWER_THROTTLING: i32 = 4;
    const EXECUTION_SPEED: u32 = 1;
    const IDLE_PRIORITY_CLASS: u32 = 0x40;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct PowerState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessInformation(
            process: *mut c_void,
            class: i32,
            data: *mut c_void,
            size: u32,
        ) -> i32;
        fn SetProcessInformation(
            process: *mut c_void,
            class: i32,
            data: *const c_void,
            size: u32,
        ) -> i32;
        fn GetPriorityClass(process: *mut c_void) -> u32;
        fn SetPriorityClass(process: *mut c_void, class: u32) -> i32;
    }

    pub fn enable() -> io::Result<()> {
        let process = unsafe { GetCurrentProcess() };
        let size = std::mem::size_of::<PowerState>() as u32;
        let mut previous = PowerState {
            version: 1,
            control_mask: 0,
            state_mask: 0,
        };
        if unsafe {
            GetProcessInformation(
                process,
                PROCESS_POWER_THROTTLING,
                std::ptr::from_mut(&mut previous).cast(),
                size,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let old_priority = unsafe { GetPriorityClass(process) };
        if old_priority == 0 {
            return Err(io::Error::last_os_error());
        }
        let enabled = PowerState {
            control_mask: previous.control_mask | EXECUTION_SPEED,
            state_mask: previous.state_mask | EXECUTION_SPEED,
            ..previous
        };
        if unsafe {
            SetProcessInformation(
                process,
                PROCESS_POWER_THROTTLING,
                std::ptr::from_ref(&enabled).cast(),
                size,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe { SetPriorityClass(process, IDLE_PRIORITY_CLASS) } == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                SetProcessInformation(
                    process,
                    PROCESS_POWER_THROTTLING,
                    std::ptr::from_ref(&previous).cast(),
                    size,
                );
            }
            return Err(error);
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn windows_power_state_matches_the_sdk_layout() {
            assert_eq!(std::mem::size_of::<PowerState>(), 12);
            assert_eq!(PROCESS_POWER_THROTTLING, 4);
            assert_eq!(EXECUTION_SPEED, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn efficiency_is_enabled_by_default_with_explicit_opt_out() {
        assert!(requested(None));
        for value in ["1", "true", "on"] {
            assert!(requested(Some(value)));
        }
        for value in ["0", "false", " OFF ", "disabled"] {
            assert!(!requested(Some(value)));
        }
    }
}
