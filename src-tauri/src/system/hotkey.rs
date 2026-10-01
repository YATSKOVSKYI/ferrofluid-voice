use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};
use crate::commands::{
    start_recording_internal, stop_recording_internal, AppState, GLOBAL_APP_HANDLE,
    IS_RECORDING,
};
use crate::errors::AppError;
use crate::stt::model_manager::AppSettings;

pub fn configure(value: &str) {
    #[cfg(target_os = "windows")]
    win::configure(value);
}
pub fn capture(enabled: bool) {
    #[cfg(target_os = "windows")]
    win::capture(enabled);
    #[cfg(not(target_os = "windows"))]
    crate::commands::IS_RECORDING_HOTKEY.store(enabled, Ordering::SeqCst);
}
pub fn valid_hotkey(value: &str) -> bool { super::hold_hotkey::valid(value) }

#[cfg(target_os = "windows")]
use crate::commands::PREV_FOREGROUND_WINDOW;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
}

fn get_cached_settings() -> Option<AppSettings> {
    let app_guard = match GLOBAL_APP_HANDLE.lock() {
        Ok(guard) => guard,
        Err(_) => {
            println!("[RUST HOTKEY] GLOBAL_APP_HANDLE lock poisoned");
            return None;
        }
    };
    let app = match app_guard.as_ref() {
        Some(a) => a,
        None => {
            println!("[RUST HOTKEY] GLOBAL_APP_HANDLE is None");
            return None;
        }
    };
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => {
            println!("[RUST HOTKEY] AppState not found in managed state");
            return None;
        }
    };
    let settings = match state.settings.lock() {
        Ok(s) => s,
        Err(_) => {
            println!("[RUST HOTKEY] AppState settings lock poisoned");
            return None;
        }
    };
    Some(settings.clone())
}

fn trigger_start_recording(foreground: Option<isize>) {
    println!("[RUST HOOK] trigger_start_recording entry");
    if !IS_RECORDING.swap(true, Ordering::SeqCst) {
        println!("[RUST HOOK] trigger_start_recording: swapped successfully, calling start_recording_internal");
        
        #[cfg(target_os = "windows")]
        unsafe {
            PREV_FOREGROUND_WINDOW.store(foreground.unwrap_or_else(|| GetForegroundWindow() as isize), Ordering::SeqCst);
        }

        let should_reveal_widget = get_cached_settings()
            .map(|settings| !settings.always_on)
            .unwrap_or(true);

        let app = GLOBAL_APP_HANDLE
            .lock()
            .unwrap()
            .as_ref()
            .cloned();

        if let Some(app) = app {
            let start_result = if let Some(state) = app.try_state::<AppState>() {
                start_recording_internal(&state)
            } else {
                Err(AppError::Audio("Application state is not ready.".into()))
            };

            if let Err(error) = start_result {
                println!("[RUST HOOK] trigger_start_recording: start_recording_internal error={:?}", error);
                IS_RECORDING.store(false, Ordering::SeqCst);
                let _ = app.emit("hotkey-recording-error", serde_json::json!({
                    "message": error.to_string(),
                }));
                return;
            }

            println!("[RUST HOOK] trigger_start_recording: start_recording_internal success. should_reveal_widget={}", should_reveal_widget);
            if let Some(main_win) = app.get_webview_window("main") {
                if should_reveal_widget {
                    let _ = main_win.show();
                    let _ = main_win.unminimize();
                    let _ = main_win.set_focus();
                    println!("[RUST HOOK] trigger_start_recording: main window shown and focused");
                }
            }
            
            println!("[RUST HOOK] trigger_start_recording: emitting hotkey-start-recording event");
            let _ = app.emit("hotkey-start-recording", serde_json::json!({
                "source": "hotkey",
                "alreadyStarted": true,
            }));
        }
    } else {
        println!("[RUST HOOK] trigger_start_recording: swap failed, IS_RECORDING was already true");
    }
}

fn trigger_stop_recording() {
    println!("[RUST HOOK] trigger_stop_recording entry");
    if IS_RECORDING.swap(false, Ordering::SeqCst) {
        println!("[RUST HOOK] trigger_stop_recording: swapped successfully, calling stop_recording_internal");
        let app = GLOBAL_APP_HANDLE
            .lock()
            .unwrap()
            .as_ref()
            .cloned();

        if let Some(app) = app {
            let stop_result = if let Some(state) = app.try_state::<AppState>() {
                stop_recording_internal(&state)
            } else {
                Err(AppError::Audio("Application state is not ready.".into()))
            };

            if let Err(error) = stop_result {
                println!("[RUST HOOK] trigger_stop_recording: stop_recording_internal error={:?}", error);
                if !matches!(error, AppError::RecordingNotRunning) {
                    let _ = app.emit("hotkey-recording-error", serde_json::json!({
                        "message": error.to_string(),
                    }));
                }
                return;
            }

            println!("[RUST HOOK] trigger_stop_recording: stop_recording_internal success. emitting hotkey-stop-recording event");
            let _ = app.emit("hotkey-stop-recording", serde_json::json!({
                "source": "hotkey",
                "alreadyStopped": true,
            }));
        }
    } else {
        println!("[RUST HOOK] trigger_stop_recording: swap failed, IS_RECORDING was already false");
    }
}

#[cfg(target_os = "windows")]
#[path = "hotkey_win.rs"]
mod win;

#[cfg(target_os = "macos")]
mod mac {
    use std::sync::atomic::Ordering;
    use std::ptr;
    use std::os::raw::c_void;
    use tauri::Emitter;

    use crate::commands::{GLOBAL_APP_HANDLE, IS_RECORDING_HOTKEY};
    use super::{get_cached_settings, trigger_start_recording, trigger_stop_recording};

    type CFMachPortRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type CGEventRef = *mut c_void;

    type CGEventTapCallBack = Option<
        unsafe extern "C" fn(
            proxy: CGEventTapProxy,
            type_: u32,
            event: CGEventRef,
            refcon: *mut c_void,
        ) -> CGEventRef,
    >;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            eventsOfInterest: u64,
            callback: CGEventTapCallBack,
            refcon: *mut c_void,
        ) -> CFMachPortRef;

        fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        fn CGEventGetFlags(event: CGEventRef) -> u64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: *mut c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> *mut c_void;

        fn CFRunLoopGetCurrent() -> *mut c_void;
        
        fn CFRunLoopAddSource(
            rl: *mut c_void,
            source: *mut c_void,
            mode: *const c_void,
        );
        
        fn CFRunLoopRun();
        fn CFRunLoopStop(rl: *mut c_void);
        fn CFRelease(cf: *mut c_void);
        
        static kCFRunLoopCommonModes: *const c_void;
    }

    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

    const K_CG_EVENT_KEY_DOWN: u32 = 10;
    const K_CG_EVENT_KEY_UP: u32 = 11;
    const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
    const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;

    const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    const K_CG_MOUSE_EVENT_BUTTON_NUMBER: u32 = 3;

    static mut RUN_LOOP: *mut c_void = ptr::null_mut();

    fn mac_keycode_to_js(keycode: u16) -> u32 {
        match keycode {
            0 => 65,  // A
            1 => 83,  // S
            2 => 68,  // D
            3 => 70,  // F
            4 => 72,  // H
            5 => 71,  // G
            6 => 90,  // Z
            7 => 88,  // X
            8 => 67,  // C
            9 => 86,  // V
            11 => 66, // B
            12 => 81, // Q
            13 => 87, // W
            14 => 69, // E
            15 => 82, // R
            16 => 89, // Y
            17 => 84, // T
            18 => 49, // 1
            19 => 50, // 2
            20 => 51, // 3
            21 => 52, // 4
            22 => 54, // 6
            23 => 53, // 5
            24 => 187, // =
            25 => 57, // 9
            26 => 55, // 7
            27 => 189, // -
            28 => 56, // 8
            29 => 48, // 0
            30 => 221, // ]
            31 => 79, // O
            32 => 85, // U
            33 => 219, // [
            34 => 73, // I
            35 => 80, // P
            36 => 13, // Return
            37 => 76, // L
            38 => 74, // J
            39 => 222, // '
            40 => 75, // K
            41 => 186, // ;
            42 => 220, // \
            43 => 188, // ,
            44 => 191, // /
            45 => 78, // N
            46 => 77, // M
            47 => 190, // .
            48 => 9,  // Tab
            49 => 32, // Space
            50 => 192, // `
            51 => 8,  // Delete
            53 => 27, // Escape
            54 | 55 => 91, // Command
            56 | 60 => 16, // Shift
            57 => 20, // Caps Lock
            58 | 61 => 18, // Option/Alt
            59 | 62 => 17, // Control
            96 => 116, // F5
            97 => 117, // F6
            98 => 118, // F7
            99 => 114, // F3
            100 => 119, // F8
            101 => 120, // F9
            109 => 121, // F10
            111 => 123, // F12
            113 => 126, // F15
            115 => 36, // Home
            116 => 33, // PageUp
            117 => 46, // ForwardDelete
            118 => 115, // F4
            119 => 35, // End
            120 => 113, // F2
            121 => 34, // PageDown
            122 => 112, // F1
            123 => 37, // Left Arrow
            124 => 39, // Right Arrow
            125 => 40, // Down Arrow
            126 => 38, // Up Arrow
            _ => keycode as u32,
        }
    }

    pub fn get_key_display_name(js_code: u32) -> String {
        match js_code {
            8 => "Backspace".into(),
            9 => "Tab".into(),
            13 => "Enter".into(),
            16 => "Shift".into(),
            17 => "Control".into(),
            18 => "Option/Alt".into(),
            20 => "Caps Lock".into(),
            27 => "Escape".into(),
            32 => "Space".into(),
            33 => "Page Up".into(),
            34 => "Page Down".into(),
            35 => "End".into(),
            36 => "Home".into(),
            37 => "Left Arrow".into(),
            38 => "Up Arrow".into(),
            39 => "Right Arrow".into(),
            40 => "Down Arrow".into(),
            46 => "Forward Delete".into(),
            48..=57 => format!("{}", (js_code - 48) as u8 as char),
            65..=90 => format!("{}", js_code as u8 as char),
            91 => "Command".into(),
            112..=123 => format!("F{}", js_code - 112 + 1),
            186 => ";".into(),
            187 => "=".into(),
            188 => ",".into(),
            189 => "-".into(),
            190 => ".".into(),
            191 => "/".into(),
            192 => "`".into(),
            219 => "[".into(),
            220 => "\\".into(),
            221 => "]".into(),
            222 => "'".into(),
            _ => format!("Key {:#X}", js_code),
        }
    }

    pub fn parse_hotkey_display(hotkey_str: &str) -> String {
        if hotkey_str == "unassigned" {
            return "Unassigned".into();
        }
        if hotkey_str == "mouse_middle" || hotkey_str == "" {
            return "Middle Click".into();
        }
        if hotkey_str == "mouse_left" {
            return "Left Click".into();
        }
        if hotkey_str == "mouse_right" {
            return "Right Click".into();
        }
        if hotkey_str == "mouse_x1" {
            return "Side Button 4 (X1)".into();
        }
        if hotkey_str == "mouse_x2" {
            return "Side Button 5 (X2)".into();
        }
        if let Some(vk_str) = hotkey_str.strip_prefix("key_") {
            if let Ok(js_code) = vk_str.parse::<u32>() {
                return get_key_display_name(js_code);
            }
        }
        hotkey_str.to_string()
    }

    fn is_keyboard_hotkey_match(js_code: u32, hotkey_str: &str) -> bool {
        if hotkey_str == "unassigned" {
            return false;
        }
        if let Some(vk_str) = hotkey_str.strip_prefix("key_") {
            if let Ok(target_js) = vk_str.parse::<u32>() {
                return js_code == target_js;
            }
        }
        false
    }

    fn is_mouse_hotkey_match(button: u32, hotkey_str: &str) -> bool {
        if hotkey_str == "unassigned" {
            return false;
        }
        if hotkey_str == "mouse_middle" || hotkey_str == "" {
            return button == 2;
        }
        match hotkey_str {
            "mouse_x1" => button == 3,
            "mouse_x2" => button == 4,
            _ => false,
        }
    }

    fn mouse_button_to_hotkey_type(button: u32) -> Option<String> {
        match button {
            2 => Some("mouse_middle".to_string()),
            3 => Some("mouse_x1".to_string()),
            4 => Some("mouse_x2".to_string()),
            _ => None,
        }
    }

    unsafe extern "C" fn event_tap_callback(
        _proxy: CGEventTapProxy,
        type_: u32,
        event: CGEventRef,
        _refcon: *mut c_void,
    ) -> CGEventRef {
        let mut consume = false;

        if type_ == K_CG_EVENT_KEY_DOWN || type_ == K_CG_EVENT_KEY_UP {
            let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
            let js_code = mac_keycode_to_js(keycode);
            let is_down = type_ == K_CG_EVENT_KEY_DOWN;

            if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
                if is_down {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    let hotkey_type = format!("key_{js_code}");
                    let display_name = get_key_display_name(js_code);
                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                        let _ = app.emit("hotkey-recorded", serde_json::json!({
                            "hotkeyType": hotkey_type,
                            "displayName": display_name,
                        }));
                    }
                }
                consume = true;
            } else if let Some(settings) = get_cached_settings() {
                if is_keyboard_hotkey_match(js_code, &settings.hotkey_type) {
                    if is_down {
                        trigger_start_recording(None);
                    } else {
                        trigger_stop_recording();
                    }
                    consume = true;
                }
            }
        } else if type_ == K_CG_EVENT_FLAGS_CHANGED {
            let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
            let js_code = mac_keycode_to_js(keycode);
            
            let flags = CGEventGetFlags(event);
            let is_down = match keycode {
                56 | 60 => (flags & 0x00020000) != 0, // Shift
                59 | 62 => (flags & 0x00040000) != 0, // Control
                58 | 61 => (flags & 0x00080000) != 0, // Option/Alt
                55 | 54 => (flags & 0x00100000) != 0, // Command
                57 => (flags & 0x00010000) != 0,      // Caps Lock
                _ => false,
            };

            if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
                if is_down {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    let hotkey_type = format!("key_{js_code}");
                    let display_name = get_key_display_name(js_code);
                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                        let _ = app.emit("hotkey-recorded", serde_json::json!({
                            "hotkeyType": hotkey_type,
                            "displayName": display_name,
                        }));
                    }
                }
                consume = true;
            } else if let Some(settings) = get_cached_settings() {
                if is_keyboard_hotkey_match(js_code, &settings.hotkey_type) {
                    if is_down {
                        trigger_start_recording(None);
                    } else {
                        trigger_stop_recording();
                    }
                    consume = true;
                }
            }
        } else if type_ == K_CG_EVENT_OTHER_MOUSE_DOWN || type_ == K_CG_EVENT_OTHER_MOUSE_UP {
            let button_number = CGEventGetIntegerValueField(event, K_CG_MOUSE_EVENT_BUTTON_NUMBER) as u32;
            let is_down = type_ == K_CG_EVENT_OTHER_MOUSE_DOWN;

            if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
                if is_down {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    
                    if let Some(hotkey_type) = mouse_button_to_hotkey_type(button_number) {
                        let display_name = match button_number {
                            2 => "Middle Click".to_string(),
                            3 => "Side Button 4 (X1)".to_string(),
                            4 => "Side Button 5 (X2)".to_string(),
                            _ => format!("Mouse Button {button_number}"),
                        };
                        if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                            let _ = app.emit("hotkey-recorded", serde_json::json!({
                                "hotkeyType": hotkey_type,
                                "displayName": display_name,
                            }));
                        }
                    } else {
                        if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                            let _ = app.emit("hotkey-recording-error", serde_json::json!({
                                "message": "Only Middle click and side mouse buttons are supported.",
                            }));
                        }
                    }
                }
                consume = true;
            } else if let Some(settings) = get_cached_settings() {
                if is_mouse_hotkey_match(button_number, &settings.hotkey_type) {
                    if is_down {
                        trigger_start_recording(None);
                    } else {
                        trigger_stop_recording();
                    }
                    consume = true;
                }
            }
        }

        if consume {
            ptr::null_mut()
        } else {
            event
        }
    }

    pub fn start_hook_thread() {
        std::thread::spawn(|| unsafe {
            let event_mask = (1 << K_CG_EVENT_KEY_DOWN)
                | (1 << K_CG_EVENT_KEY_UP)
                | (1 << K_CG_EVENT_FLAGS_CHANGED)
                | (1 << K_CG_EVENT_OTHER_MOUSE_DOWN)
                | (1 << K_CG_EVENT_OTHER_MOUSE_UP);

            let tap = CGEventTapCreate(
                K_CG_HID_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_DEFAULT,
                event_mask,
                Some(event_tap_callback),
                ptr::null_mut(),
            );

            if tap.is_null() {
                println!("[RUST HOOK] Failed to create CGEventTap. Please ensure Accessibility permissions are granted.");
                return;
            }

            let run_loop_source = CFMachPortCreateRunLoopSource(
                ptr::null_mut(),
                tap,
                0,
            );

            if !run_loop_source.is_null() {
                let run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
                RUN_LOOP = run_loop;
                
                CFRunLoopRun();
                
                RUN_LOOP = ptr::null_mut();
                CFRelease(run_loop_source);
            }
            CFRelease(tap);
        });
    }

    pub fn stop_hook_thread() {
        unsafe {
            if !RUN_LOOP.is_null() {
                CFRunLoopStop(RUN_LOOP);
                RUN_LOOP = ptr::null_mut();
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_hook_thread() {
    win::start_hook_thread();
}

#[cfg(target_os = "windows")]
pub fn stop_hook_thread() {
    win::stop_hook_thread();
}

#[cfg(target_os = "windows")]
pub fn parse_hotkey_display(hotkey_str: &str) -> String {
    win::parse_hotkey_display(hotkey_str)
}

#[cfg(target_os = "macos")]
pub fn start_hook_thread() {
    mac::start_hook_thread();
}

#[cfg(target_os = "macos")]
pub fn stop_hook_thread() {
    mac::stop_hook_thread();
}

#[cfg(target_os = "macos")]
pub fn parse_hotkey_display(hotkey_str: &str) -> String {
    mac::parse_hotkey_display(hotkey_str)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn start_hook_thread() {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn stop_hook_thread() {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn parse_hotkey_display(hotkey_str: &str) -> String {
    hotkey_str.to_string()
}
