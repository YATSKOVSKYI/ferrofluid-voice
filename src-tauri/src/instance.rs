//! Acquire ownership before Tauri creates WebViews, hooks or application state.
//! The plugin remains the activation receiver, but its Windows startup race and
//! unbounded SendMessage are bypassed by this early gate.
#[cfg(target_os = "windows")]
mod windows {
    use std::{
        io, ptr, thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE},
        System::{
            DataExchange::COPYDATASTRUCT,
            Threading::{CreateMutexW, ReleaseMutex},
        },
        UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, SendMessageTimeoutW,
            SMTO_ABORTIFHUNG, SMTO_BLOCK, WM_COPYDATA,
        },
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    pub struct Guard(HANDLE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }

    // Must match the application identifier and the plugin's documented IPC endpoint.
    const ID: &str = "com.ferrofluid.voice";
    pub fn enter() -> io::Result<Option<Guard>> {
        let name = wide(&format!("Local\\{ID}-startup-v1"));
        let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let existing = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let class = wide(&format!("{ID}-sic"));
        let title = wide(&format!("{ID}-siw"));
        // Also activate an older installed build using the same app identifier.
        let legacy = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
        if !existing && legacy.is_null() {
            return Ok(Some(Guard(handle)));
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        let outcome = loop {
            let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
            if !hwnd.is_null() {
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    AllowSetForegroundWindow(pid);
                }
                let cwd = std::env::current_dir().unwrap_or_default();
                let args = std::env::args().collect::<Vec<_>>().join("|");
                let data = format!("{}|{args}\0", cwd.to_string_lossy());
                let packet = COPYDATASTRUCT {
                    dwData: 1542,
                    cbData: data.len() as u32,
                    lpData: data.as_ptr() as *mut _,
                };
                let mut response = 0;
                let sent = unsafe {
                    SendMessageTimeoutW(
                        hwnd,
                        WM_COPYDATA,
                        0,
                        &packet as *const _ as isize,
                        SMTO_ABORTIFHUNG | SMTO_BLOCK,
                        2000,
                        &mut response,
                    )
                };
                break if sent != 0 {
                    Ok(None)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Existing Ferrofluid Voice did not respond within 2 seconds",
                    ))
                };
            }
            if Instant::now() >= deadline {
                break Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Existing Ferrofluid Voice did not finish startup within 10 seconds",
                ));
            }
            thread::sleep(Duration::from_millis(50));
        };
        unsafe {
            if !existing {
                ReleaseMutex(handle);
            }
            CloseHandle(handle);
        }
        outcome
    }
}

#[cfg(target_os = "windows")]
pub use windows::enter;

#[cfg(not(target_os = "windows"))]
pub fn enter() -> std::io::Result<Option<()>> {
    Ok(Some(()))
}

pub fn report_failure(error: &std::io::Error) {
    use std::io::Write;
    if let Some(root) = dirs::data_dir() {
        let root = root.join("Ferrofluid Voice");
        let _ = std::fs::create_dir_all(&root);
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("startup.log"))
        {
            let _ = writeln!(log, "{} {error}", chrono::Utc::now().to_rfc3339());
        }
    }
    eprintln!("Ferrofluid Voice: {error}");
}

/// Restore the actual native window even if cached visibility became stale
/// (for example after Explorer or another native window manager hid it).
pub fn restore_window(window: &tauri::WebviewWindow) -> Result<(), tauri::Error> {
    window.show()?;
    window.unminimize()?;
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE};
        ShowWindow(window.hwnd()?.0 as _, SW_RESTORE);
    }
    window.set_focus()
}
