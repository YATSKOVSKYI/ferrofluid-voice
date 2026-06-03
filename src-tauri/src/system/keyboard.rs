#[cfg(target_os = "windows")]
mod win {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KEYBDINPUT {
        w_vk: u16,
        w_scan: u16,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union INPUT_UNION {
        ki: KEYBDINPUT,
        mi: [u8; 28], // Pad structure to match larger mouse/hardware union variants in Win32
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct INPUT {
        r#type: u32,
        u: INPUT_UNION,
    }

    const INPUT_KEYBOARD: u32 = 1;
    const KEYEVENTF_KEYUP: u32 = 0x0002;

    const VK_CONTROL: u16 = 0x11;
    const VK_V: u16 = 0x56;
    const VK_RETURN: u16 = 0x0D;

    #[link(name = "user32")]
    extern "system" {
        fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;
    }

    pub fn simulate_paste() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_CONTROL,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_V,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_V,
                        w_scan: 0,
                        dw_flags: KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_CONTROL,
                        w_scan: 0,
                        dw_flags: KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
        ];

        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }

    pub fn simulate_enter() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_RETURN,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                u: INPUT_UNION {
                    ki: KEYBDINPUT {
                        w_vk: VK_RETURN,
                        w_scan: 0,
                        dw_flags: KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
        ];

        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::os::raw::c_void;
    use std::ptr;

    type CGEventRef = *mut c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *mut c_void,
            virtualKey: u16,
            keyDown: bool,
        ) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: u32, event: CGEventRef);
        fn CFRelease(cf: *mut c_void);
    }

    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;

    pub fn simulate_paste() {
        unsafe {
            // macOS V keycode is 9
            let event_down = CGEventCreateKeyboardEvent(ptr::null_mut(), 9, true);
            if !event_down.is_null() {
                CGEventSetFlags(event_down, K_CG_EVENT_FLAG_MASK_COMMAND);
                CGEventPost(K_CG_HID_EVENT_TAP, event_down);
                CFRelease(event_down);
            }

            let event_up = CGEventCreateKeyboardEvent(ptr::null_mut(), 9, false);
            if !event_up.is_null() {
                CGEventSetFlags(event_up, K_CG_EVENT_FLAG_MASK_COMMAND);
                CGEventPost(K_CG_HID_EVENT_TAP, event_up);
                CFRelease(event_up);
            }
        }
    }

    pub fn simulate_enter() {
        unsafe {
            // macOS Return/Enter keycode is 36
            let event_down = CGEventCreateKeyboardEvent(ptr::null_mut(), 36, true);
            if !event_down.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, event_down);
                CFRelease(event_down);
            }

            let event_up = CGEventCreateKeyboardEvent(ptr::null_mut(), 36, false);
            if !event_up.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, event_up);
                CFRelease(event_up);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn simulate_paste() {
    win::simulate_paste();
}

#[cfg(target_os = "windows")]
pub fn simulate_enter() {
    win::simulate_enter();
}

#[cfg(target_os = "macos")]
pub fn simulate_paste() {
    mac::simulate_paste();
}

#[cfg(target_os = "macos")]
pub fn simulate_enter() {
    mac::simulate_enter();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn simulate_paste() {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn simulate_enter() {}
