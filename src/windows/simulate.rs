use crate::rdev::{Button, EventType, SimulateError};
use crate::windows::keycodes::get_win_codes;
use std::convert::{TryFrom, TryInto};
use std::mem::size_of;
use std::ptr::null_mut;
use winapi::ctypes::{c_int, c_short};
use winapi::shared::minwindef::{DWORD, LOWORD, UINT, WORD};
use winapi::shared::ntdef::LONG;
use winapi::um::winuser::{
    GetForegroundWindow, GetKeyboardLayout, GetSystemMetrics, GetWindowThreadProcessId, INPUT_u,
    MapVirtualKeyExW, SendInput, VkKeyScanW, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_KEYUP, MAPVK_VSC_TO_VK_EX, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK,
    MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, WHEEL_DELTA,
};
/// Not defined in win32 but define here for clarity
static KEYEVENTF_KEYDOWN: DWORD = 0;
// KEYBDINPUT
static KEYUP: u16 = 0x0002;
static KEYDOWN: u16 = 0;
static UNICODE: u16 = 0x0004;

fn sim_mouse_event(flags: DWORD, data: DWORD, dx: LONG, dy: LONG) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.mi_mut() };
    *inner_union = MOUSEINPUT {
        dx,
        dy,
        mouseData: data,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };
    let mut input = [INPUT {
        type_: INPUT_MOUSE,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

fn sim_keyboard_event(flags: DWORD, vk: WORD, scan: WORD) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.ki_mut() };
    *inner_union = KEYBDINPUT {
        wVk: vk,
        wScan: scan,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };
    let mut input = [INPUT {
        type_: INPUT_KEYBOARD,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    match event_type {
        EventType::KeyPress(key) => {
            let layout = unsafe {
                let current_window_thread_id =
                    GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
                GetKeyboardLayout(current_window_thread_id)
            };
            let (code, scancode) = get_win_codes(*key);
            let code = if code == 165 && LOWORD(layout as usize as u32) == 0x0412 {
                winapi::um::winuser::VK_HANGUL as u32
            } else if code == 165 {
                // altgr
                165
            } else if scancode != 0 {
                unsafe { MapVirtualKeyExW(scancode as _, MAPVK_VSC_TO_VK_EX, layout) }
            } else {
                code
            };
            sim_keyboard_event(KEYEVENTF_KEYDOWN, code.try_into().unwrap(), 0)
        }
        EventType::KeyRelease(key) => {
            let (code, scancode) = get_win_codes(*key);
            let code = if code == 165 {
                // altgr
                165
            } else if scancode != 0 {
                unsafe {
                    let current_window_thread_id =
                        GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
                    let layout = GetKeyboardLayout(current_window_thread_id);
                    MapVirtualKeyExW(scancode as _, MAPVK_VSC_TO_VK_EX, layout)
                }
            } else {
                code
            };
            sim_keyboard_event(KEYEVENTF_KEYUP, code.try_into().unwrap(), 0)
        }
        EventType::ButtonPress(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTDOWN, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XDOWN, 0, 0, (*code).into()),
        },
        EventType::ButtonRelease(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEUP, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTUP, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XUP, 0, 0, (*code).into()),
        },
        EventType::Wheel { delta_x, delta_y } => {
            if *delta_x != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_HWHEEL,
                    (c_short::try_from(*delta_x).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }

            if *delta_y != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_WHEEL,
                    (c_short::try_from(*delta_y).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }
            Ok(())
        }
        EventType::MouseMove { x, y } => {
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if width == 0 || height == 0 {
                return Err(SimulateError);
            }

            sim_mouse_event(
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                0,
                (*x as i32 + 1) * 65535 / width,
                (*y as i32 + 1) * 65535 / height,
            )
        }
    }
}

pub fn simulate_char(chr: char, pressed: bool) -> Result<(), SimulateError> {
    // send char
    let res = unsafe { VkKeyScanW(chr as u16) };
    let (vk, scan, flags): (i32, u16, u16) = if (res >> 8) & 0xFF == 0 {
        ((res & 0xFF).into(), 0, 0)
    } else {
        (0, chr as _, UNICODE)
    };

    let state_flags = if pressed { KEYDOWN } else { KEYUP };
    sim_keyboard_event((flags | state_flags).into(), vk as _, scan)
}
