use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};
use crate::commands::{
    start_recording_internal, stop_recording_internal, AppState, GLOBAL_APP_HANDLE,
    IS_RECORDING,
};
use crate::errors::AppError;
use crate::stt::model_manager::AppSettings;

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

fn trigger_start_recording() {
    println!("[RUST HOOK] trigger_start_recording entry");
    if !IS_RECORDING.swap(true, Ordering::SeqCst) {
        println!("[RUST HOOK] trigger_start_recording: swapped successfully, calling start_recording_internal");
        
        #[cfg(target_os = "windows")]
        unsafe {
            PREV_FOREGROUND_WINDOW = GetForegroundWindow();
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
mod win {
    use std::sync::atomic::Ordering;
    use std::ptr;
    use tauri::Emitter;
    use crate::commands::{
        GLOBAL_APP_HANDLE, H_HOOK_KEYBOARD, H_HOOK_MOUSE, HOOK_THREAD_ID,
        IS_RECORDING_HOTKEY,
    };
    use super::{get_cached_settings, trigger_start_recording, trigger_stop_recording};

    type HWND = *mut std::ffi::c_void;
    type HHOOK = *mut std::ffi::c_void;
    type HINSTANCE = *mut std::ffi::c_void;
    type WPARAM = usize;
    type LPARAM = isize;
    type LRESULT = isize;
    type HOOKPROC = Option<unsafe extern "system" fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT>;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MSG {
        hwnd: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
        time: u32,
        pt: POINT,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KBDLLHOOKSTRUCT {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MSLLHOOKSTRUCT {
        pt: POINT,
        mouse_data: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    const WH_KEYBOARD_LL: i32 = 13;
    const WH_MOUSE_LL: i32 = 14;

    const WM_KEYDOWN: usize = 0x0100;
    const WM_KEYUP: usize = 0x0101;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const WM_SYSKEYUP: usize = 0x0105;

    const WM_MOUSEMOVE: usize = 0x0200;
    const WM_LBUTTONDOWN: usize = 0x0201;
    const WM_RBUTTONDOWN: usize = 0x0204;
    const WM_MBUTTONDOWN: usize = 0x0207;
    const WM_MBUTTONUP: usize = 0x0208;
    const WM_XBUTTONDOWN: usize = 0x020B;
    const WM_XBUTTONUP: usize = 0x020C;

    const WM_QUIT: u32 = 0x0012;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(idHook: i32, lpfn: HOOKPROC, hmod: HINSTANCE, dwThreadId: u32) -> HHOOK;
        fn UnhookWindowsHookEx(hhk: HHOOK) -> i32;
        fn CallNextHookEx(hhk: HHOOK, nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        fn GetMessageW(lpMsg: *mut MSG, hwnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
        fn TranslateMessage(lpMsg: *const MSG) -> i32;
        fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
        fn PostThreadMessageW(idThread: u32, msg: u32, wParam: WPARAM, lParam: LPARAM) -> i32;
        fn GetCurrentThreadId() -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
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
            if let Ok(vk_code) = vk_str.parse::<u32>() {
                return get_key_display_name(vk_code);
            }
        }
        // Fallback for legacy structures
        match hotkey_str {
            "keyboard_f10" => "F10".into(),
            "keyboard_caps" => "Caps Lock".into(),
            "keyboard_scroll" => "Scroll Lock".into(),
            "keyboard_insert" => "Insert".into(),
            _ => hotkey_str.to_string(),
        }
    }

    fn is_keyboard_hotkey_match(vk_code: u32, hotkey_str: &str) -> bool {
        if hotkey_str == "unassigned" {
            return false;
        }
        if let Some(vk_str) = hotkey_str.strip_prefix("key_") {
            if let Ok(target_vk) = vk_str.parse::<u32>() {
                // Shift generic (16), Left Shift (160), Right Shift (161)
                if (target_vk == 16 || target_vk == 160 || target_vk == 161) &&
                   (vk_code == 16 || vk_code == 160 || vk_code == 161) {
                    return true;
                }
                // Control generic (17), Left Control (162), Right Control (163)
                if (target_vk == 17 || target_vk == 162 || target_vk == 163) &&
                   (vk_code == 17 || vk_code == 162 || vk_code == 163) {
                    return true;
                }
                // Alt generic (18), Left Alt (164), Right Alt (165)
                if (target_vk == 18 || target_vk == 164 || target_vk == 165) &&
                   (vk_code == 18 || vk_code == 164 || vk_code == 165) {
                    return true;
                }
                return vk_code == target_vk;
            }
        }
        match hotkey_str {
            "keyboard_f10" => vk_code == 0x79,
            "keyboard_caps" => vk_code == 0x14,
            "keyboard_scroll" => vk_code == 0x91,
            "keyboard_insert" => vk_code == 0x2D,
            _ => false,
        }
    }

    fn is_mouse_hotkey_match(wparam: usize, hook_struct: &MSLLHOOKSTRUCT, hotkey_str: &str) -> bool {
        if hotkey_str == "unassigned" {
            return false;
        }
        if hotkey_str == "mouse_middle" || hotkey_str == "" {
            return wparam == WM_MBUTTONDOWN || wparam == WM_MBUTTONUP;
        }

        match hotkey_str {
            "mouse_left" | "mouse_right" => false,
            "mouse_x1" => {
                if wparam == WM_XBUTTONDOWN || wparam == WM_XBUTTONUP {
                    let xbutton = (hook_struct.mouse_data >> 16) as u16;
                    return xbutton == 1;
                }
                false
            }
            "mouse_x2" => {
                if wparam == WM_XBUTTONDOWN || wparam == WM_XBUTTONUP {
                    let xbutton = (hook_struct.mouse_data >> 16) as u16;
                    return xbutton == 2;
                }
                false
            }
            _ => false,
        }
    }

    fn is_mouse_down_event(wparam: usize) -> bool {
        wparam == WM_LBUTTONDOWN
            || wparam == WM_RBUTTONDOWN
            || wparam == WM_MBUTTONDOWN
            || wparam == WM_XBUTTONDOWN
    }

    unsafe extern "system" fn low_level_keyboard_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let hook_struct = *(lparam as *const KBDLLHOOKSTRUCT);
            let vk_code = hook_struct.vk_code;
            println!("[RUST HOOK] low_level_keyboard_proc triggered. vk_code={}, wparam={}", vk_code, wparam);

            // 1. Interactive Hotkey Recording Mode
            if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
                if wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);
                    let hotkey_type = format!("key_{vk_code}");
                    let display_name = get_key_display_name(vk_code);
                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                        let _ = app.emit("hotkey-recorded", serde_json::json!({
                            "hotkeyType": hotkey_type,
                            "displayName": display_name,
                        }));
                    }
                }
                return 1; // Consume input in recording mode
            }

            // 2. Standard Recording Trigger
            if let Some(settings) = get_cached_settings() {
                if is_keyboard_hotkey_match(vk_code, &settings.hotkey_type) {
                    if wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN {
                        trigger_start_recording();
                    } else if wparam == WM_KEYUP || wparam == WM_SYSKEYUP {
                        trigger_stop_recording();
                    }
                    return 1;
                }
            }
        }
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }

    unsafe extern "system" fn low_level_mouse_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            // Bypass high-frequency mouse movements instantly for zero CPU overhead
            if wparam == WM_MOUSEMOVE {
                return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
            }

            println!("[RUST HOOK] low_level_mouse_proc triggered. wparam={}", wparam);

            let hook_struct = *(lparam as *const MSLLHOOKSTRUCT);
            // 1. Interactive Hotkey Recording Mode
            if IS_RECORDING_HOTKEY.load(Ordering::SeqCst) {
                if wparam == WM_LBUTTONDOWN
                    || wparam == WM_RBUTTONDOWN
                    || wparam == WM_MBUTTONDOWN
                    || wparam == WM_XBUTTONDOWN
                {
                    IS_RECORDING_HOTKEY.store(false, Ordering::SeqCst);

                    if wparam == WM_LBUTTONDOWN || wparam == WM_RBUTTONDOWN {
                        if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                            let _ = app.emit("hotkey-recording-error", serde_json::json!({
                                "message": "Left and right mouse clicks cannot be used as global hotkeys.",
                            }));
                        }
                        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
                    }

                    let (hotkey_type, display_name) = if wparam == WM_MBUTTONDOWN {
                        ("mouse_middle".to_string(), "Middle Click".to_string())
                    } else {
                        let xbutton = (hook_struct.mouse_data >> 16) as u16;
                        if xbutton == 1 {
                            ("mouse_x1".to_string(), "Side Button 4 (X1)".to_string())
                        } else {
                            ("mouse_x2".to_string(), "Side Button 5 (X2)".to_string())
                        }
                    };

                    if let Some(app) = GLOBAL_APP_HANDLE.lock().unwrap().as_ref() {
                        let _ = app.emit("hotkey-recorded", serde_json::json!({
                            "hotkeyType": hotkey_type,
                            "displayName": display_name,
                        }));
                    }
                }
                return 1; // Consume input in recording mode
            }

            // 2. Standard Recording Trigger
            if let Some(settings) = get_cached_settings() {
                println!("[RUST HOOK] settings.hotkey_type = '{}'", settings.hotkey_type);
                if is_mouse_hotkey_match(wparam, &hook_struct, &settings.hotkey_type) {
                    if is_mouse_down_event(wparam) {
                        trigger_start_recording();
                    } else {
                        trigger_stop_recording();
                    }
                    return 1; // Consume clicks to avoid losing target window focus
                }
            }
        }
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }

    pub fn start_hook_thread() {
        std::thread::spawn(|| unsafe {
            HOOK_THREAD_ID = GetCurrentThreadId();

            let hinst = GetModuleHandleW(ptr::null());
            H_HOOK_KEYBOARD = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                hinst,
                0,
            );
            H_HOOK_MOUSE = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(low_level_mouse_proc),
                hinst,
                0,
            );
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if !H_HOOK_KEYBOARD.is_null() {
                UnhookWindowsHookEx(H_HOOK_KEYBOARD);
                H_HOOK_KEYBOARD = ptr::null_mut();
            }
            if !H_HOOK_MOUSE.is_null() {
                UnhookWindowsHookEx(H_HOOK_MOUSE);
                H_HOOK_MOUSE = ptr::null_mut();
            }
        });
    }

    pub fn stop_hook_thread() {
        unsafe {
            if HOOK_THREAD_ID != 0 {
                PostThreadMessageW(HOOK_THREAD_ID, WM_QUIT, 0, 0);
                HOOK_THREAD_ID = 0;
            }
        }
    }
}

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
                        trigger_start_recording();
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
                        trigger_start_recording();
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
                        trigger_start_recording();
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
