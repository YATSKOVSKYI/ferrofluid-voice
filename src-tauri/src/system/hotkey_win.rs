use super::{get_cached_settings, trigger_start_recording, trigger_stop_recording};
use crate::commands::{GLOBAL_APP_HANDLE, IS_RECORDING, IS_RECORDING_HOTKEY, IS_TRANSCRIBING};
use crate::system::hold_hotkey::{self, Action, HoldState, MIDDLE, X1, X2};
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Mutex, OnceLock,
    },
};
use tauri::Emitter;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows_sys::Win32::{
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::*,
};

static STATE: OnceLock<Mutex<HoldState>> = OnceLock::new();
fn state() -> &'static Mutex<HoldState> {
    STATE.get_or_init(|| Mutex::new(HoldState::default()))
}
static ACTIONS: OnceLock<mpsc::Sender<(Action, isize)>> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static KEYBOARD_REGISTERED: AtomicBool = AtomicBool::new(false);
const RECONFIGURE: u32 = WM_APP + 41;
const HOTKEY_ID: i32 = 0x4601;

fn notify_configuration() {
    let thread = THREAD_ID.load(Ordering::SeqCst);
    if thread != 0 {
        unsafe {
            PostThreadMessageW(thread, RECONFIGURE, 0, 0);
        }
    }
}

fn dispatch(actions: Vec<Action>, foreground: isize) {
    if let Some(sender) = ACTIONS.get() {
        for action in actions {
            let _ = sender.send((action, foreground));
        }
    }
}
fn trace(message: &str) {
    use std::io::Write;
    if let Some(root) = dirs::data_dir() {
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("Ferrofluid Voice/hotkey.log"))
        {
            let _ = writeln!(log, "{} {message}", chrono::Utc::now().to_rfc3339());
        }
    }
}
pub fn configure(value: &str) {
    trace(&format!("configured {value}"));
    let mut current = state().lock().unwrap();
    let changed = current.configured != hold_hotkey::parse(value);
    let actions = current.configure(value);
    drop(current);
    dispatch(actions, 0);
    if changed {
        notify_configuration();
    }
}
pub fn capture(enabled: bool) {
    IS_RECORDING_HOTKEY.store(enabled, Ordering::SeqCst);
    let actions = state().lock().unwrap().capture(enabled);
    dispatch(actions, 0);
    notify_configuration();
}
pub fn parse_hotkey_display(value: &str) -> String {
    if value == "unassigned" {
        return "Unassigned".into();
    }
    let keys = hold_hotkey::parse(value);
    if keys.is_empty() {
        return value.to_string();
    }
    keys.into_iter()
        .map(|key| match key {
            MIDDLE => "Middle Click".into(),
            X1 => "Side Button 4 (X1)".into(),
            X2 => "Side Button 5 (X2)".into(),
            91 => "Win".into(),
            _ => get_key_display_name(key),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn handle_input(key: u32, down: bool) -> bool {
    // This lock protects only the small in-memory state machine, never settings,
    // audio, filesystem or UI calls. The hook must return before Windows' timeout.
    let mut held_state = state().lock().unwrap();
    let observed = held_state.configured.contains(&hold_hotkey::normalize(key));
    let (consume, mut actions) = held_state.event(key, down);
    drop(held_state);
    #[cfg(not(test))]
    if observed {
        actions.insert(0, Action::Observed(key, down));
    }
    let foreground = if actions.iter().any(|a| *a == Action::Start) {
        unsafe { GetForegroundWindow() as isize }
    } else {
        0
    };
    dispatch(actions, foreground);
    consume
}
unsafe extern "system" fn keyboard_hook(code: i32, message: usize, data: isize) -> isize {
    // Windows owns keyboard activation; do not swallow its native hotkey or
    // depend on low-level key-up delivery after the widget changes focus.
    if KEYBOARD_REGISTERED.load(Ordering::SeqCst) && !IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
        return CallNextHookEx(ptr::null_mut(), code, message, data);
    }
    if code >= 0
        && matches!(
            message as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
    {
        let event = &*(data as *const KBDLLHOOKSTRUCT);
        if handle_input(
            event.vkCode,
            matches!(message as u32, WM_KEYDOWN | WM_SYSKEYDOWN),
        ) {
            return 1;
        }
    }
    CallNextHookEx(ptr::null_mut(), code, message, data)
}
unsafe extern "system" fn mouse_hook(code: i32, message: usize, data: isize) -> isize {
    if code >= 0 {
        let event = &*(data as *const MSLLHOOKSTRUCT);
        let input = match message as u32 {
            WM_MBUTTONDOWN => Some((MIDDLE, true)),
            WM_MBUTTONUP => Some((MIDDLE, false)),
            WM_XBUTTONDOWN | WM_XBUTTONUP => Some((
                if event.mouseData >> 16 == 1 { X1 } else { X2 },
                message as u32 == WM_XBUTTONDOWN,
            )),
            _ => None,
        };
        if let Some((key, down)) = input {
            if handle_input(key, down) {
                return 1;
            }
        }
    }
    CallNextHookEx(ptr::null_mut(), code, message, data)
}
pub fn start_hook_thread() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    let _ = ACTIONS.set(sender);
    if let Some(settings) = get_cached_settings() {
        configure(&settings.hotkey_type);
    }
    std::thread::spawn(move || {
        let mut owns_recording = false;
        for (action, foreground) in receiver {
            match action {
                Action::Observed(key, down) => trace(&format!("input VK={key} down={down}")),
                Action::Start => {
                    trace(&format!(
                        "start requested recording={} transcribing={}",
                        IS_RECORDING.load(Ordering::SeqCst),
                        IS_TRANSCRIBING.load(Ordering::SeqCst)
                    ));
                    if !IS_RECORDING.load(Ordering::SeqCst)
                        && !IS_TRANSCRIBING.load(Ordering::SeqCst)
                    {
                        trigger_start_recording(Some(foreground));
                        owns_recording = IS_RECORDING.load(Ordering::SeqCst);
                        trace(&format!("start completed owns_recording={owns_recording}"));
                    }
                }
                Action::Stop => {
                    trace(&format!("stop requested owns_recording={owns_recording}"));
                    if owns_recording {
                        trigger_stop_recording();
                        owns_recording = false;
                    }
                }
                Action::Captured(value) => {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref().cloned() {
                        let _ = app.emit(
                            "hotkey-recorded",
                            serde_json::json!({
                                "hotkeyType": value, "displayName": parse_hotkey_display(&value),
                            }),
                        );
                    }
                }
                Action::CancelCapture => {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref().cloned() {
                        let _ = app.emit("hotkey-capture-cancelled", ());
                    }
                }
            }
        }
    });
    install_hooks();
}

fn install_hooks() {
    std::thread::spawn(|| unsafe {
        // Force creation of the message queue before publishing the thread id.
        let mut message: MSG = std::mem::zeroed();
        PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
        let module = GetModuleHandleW(ptr::null());
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
        trace(&format!(
            "hooks installed keyboard={} mouse={}",
            !keyboard.is_null(),
            !mouse.is_null()
        ));
        if keyboard.is_null() || mouse.is_null() {
            if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref().cloned() {
                let _ = app.emit(
                    "hotkey-recording-error",
                    serde_json::json!({"message": "Cannot install global keyboard/mouse hooks"}),
                );
            }
        }
        let mut registered = register_keyboard();
        let mut holding = false;
        let mut armed = true;
        let timer = SetTimer(ptr::null_mut(), 0, 25, None);
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            if message.message == RECONFIGURE {
                if holding {
                    dispatch(vec![Action::Stop], 0);
                }
                holding = false;
                armed = true;
                UnregisterHotKey(ptr::null_mut(), HOTKEY_ID);
                KEYBOARD_REGISTERED.store(false, Ordering::SeqCst);
                registered = register_keyboard();
            } else if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as usize {
                if armed && registered.is_some() {
                    holding = true;
                    armed = false;
                    dispatch(vec![Action::Start], GetForegroundWindow() as isize);
                }
            } else if message.message == WM_TIMER {
                if let Some(keys) = registered.as_ref() {
                    if holding && keys.iter().any(|key| GetAsyncKeyState(*key as i32) >= 0) {
                        holding = false;
                        dispatch(vec![Action::Stop], 0);
                    }
                    if keys.iter().all(|key| GetAsyncKeyState(*key as i32) >= 0) {
                        armed = true;
                    }
                }
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        KillTimer(ptr::null_mut(), timer);
        UnregisterHotKey(ptr::null_mut(), HOTKEY_ID);
        KEYBOARD_REGISTERED.store(false, Ordering::SeqCst);
        if !keyboard.is_null() {
            UnhookWindowsHookEx(keyboard);
        }
        if !mouse.is_null() {
            UnhookWindowsHookEx(mouse);
        }
        THREAD_ID.store(0, Ordering::SeqCst);
    });
}

unsafe fn register_keyboard() -> Option<Vec<u32>> {
    if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
        return None;
    }
    let keys = state().lock().unwrap().configured.clone();
    let mut modifiers = MOD_NOREPEAT;
    let mut primary = None;
    for key in &keys {
        match *key {
            16 => modifiers |= MOD_SHIFT,
            17 => modifiers |= MOD_CONTROL,
            18 => modifiers |= MOD_ALT,
            91 => modifiers |= MOD_WIN,
            value if value < 256 && primary.is_none() => primary = Some(value),
            _ => return None,
        }
    }
    let primary = primary?;
    if RegisterHotKey(ptr::null_mut(), HOTKEY_ID, modifiers, primary) == 0 {
        trace("RegisterHotKey failed: binding may be owned by another application; using hook fallback");
        return None;
    }
    KEYBOARD_REGISTERED.store(true, Ordering::SeqCst);
    trace("keyboard activation registered with Windows; release polling active");
    Some(keys.into_iter().collect())
}
pub fn stop_hook_thread() {
    let thread_id = THREAD_ID.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, mouse_event, KEYEVENTF_KEYUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    };
    fn input(key: u8, down: bool) {
        unsafe {
            keybd_event(key, 0, if down { 0 } else { KEYEVENTF_KEYUP }, 0);
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    #[test]
    fn native_hooks_keep_release_queued_while_worker_is_busy() {
        // Ctrl+F24 avoids common application shortcuts. Test receives real
        // Windows low-level hook input, without constructing Tauri or a microphone.
        let (sender, receiver) = mpsc::channel();
        assert!(ACTIONS.set(sender).is_ok());
        configure("chord_17+135");
        install_hooks();
        for _ in 0..100 {
            if THREAD_ID.load(Ordering::SeqCst) != 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(50));
        input(162, true);
        input(135, true);
        // Leave worker receiver unread to simulate slow audio initialization.
        input(135, true);
        input(162, false);
        input(135, false);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Start
        );
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Stop
        );
        assert!(receiver.try_recv().is_err());

        capture(true);
        input(163, true);
        input(135, true);
        assert!(receiver.try_recv().is_err()); // Modifier alone is not captured.
        input(135, false);
        input(163, false);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Captured("chord_17+135".into())
        );
        capture(false);
        input(163, true);
        input(135, true);
        input(135, false);
        input(163, false);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Start
        );
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Stop
        );
        configure("mouse_middle");
        std::thread::sleep(Duration::from_millis(100));
        unsafe {
            mouse_event(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0, 0);
        }
        std::thread::sleep(Duration::from_millis(30));
        unsafe {
            mouse_event(MOUSEEVENTF_MIDDLEUP, 0, 0, 0, 0);
        }
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Start
        );
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().0,
            Action::Stop
        );
        stop_hook_thread();
    }
}

pub fn get_key_display_name(vk_code: u32) -> String {
    match vk_code {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x10 | 0xA0 | 0xA1 => "Shift".into(),
        0x11 | 0xA2 | 0xA3 => "Control".into(),
        0x12 | 0xA4 | 0xA5 => "Alt".into(),
        0x13 => "Pause".into(),
        0x14 => "Caps Lock".into(),
        0x1B => "Escape".into(),
        0x20 => "Space".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left Arrow".into(),
        0x26 => "Up Arrow".into(),
        0x27 => "Right Arrow".into(),
        0x28 => "Down Arrow".into(),
        0x2C => "Print Screen".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 => format!("{}", (vk_code - 0x30) as u8 as char),
        0x41..=0x5A => format!("{}", (vk_code - 0x41 + 65) as u8 as char),
        0x5F => "Sleep".into(),
        0x60..=0x69 => format!("Num {}", vk_code - 0x60),
        0x6A => "Num *".into(),
        0x6B => "Num +".into(),
        0x6C => "Num Separator".into(),
        0x6D => "Num -".into(),
        0x6E => "Num .".into(),
        0x6F => "Num /".into(),
        0x70..=0x87 => format!("F{}", vk_code - 0x70 + 1),
        0x90 => "Num Lock".into(),
        0x91 => "Scroll Lock".into(),
        0xA6 => "Browser Back".into(),
        0xA7 => "Browser Forward".into(),
        0xA8 => "Browser Refresh".into(),
        0xA9 => "Browser Stop".into(),
        0xAA => "Browser Search".into(),
        0xAB => "Browser Favorites".into(),
        0xAC => "Browser Home".into(),
        0xAD => "Volume Mute".into(),
        0xAE => "Volume Down".into(),
        0xAF => "Volume Up".into(),
        0xB0 => "Next Track".into(),
        0xB1 => "Previous Track".into(),
        0xB2 => "Stop Media".into(),
        0xB3 => "Play/Pause Media".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Key {:#X}", vk_code),
    }
}
