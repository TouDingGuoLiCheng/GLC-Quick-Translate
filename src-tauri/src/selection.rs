use std::thread;
use std::time::Duration;

const FOCUS_SETTLE_MS: u64 = 80;
const KEY_GAP_MS: u64 = 12;
const POST_COPY_SETTLE_MS: u64 = 50;

/// 热键按下瞬间的前台窗口句柄（必须在抢焦点之前同步捕获）
#[derive(Debug, Clone, Copy)]
pub struct TargetHwnd(pub isize);

impl TargetHwnd {
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

pub fn is_target_window_valid(target: TargetHwnd) -> bool {
    if !target.is_valid() {
        return false;
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;
        return unsafe { IsWindow(HWND(target.0 as *mut _)).as_bool() };
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(windows)]
pub fn capture_foreground_target() -> TargetHwnd {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let h = GetForegroundWindow();
        TargetHwnd(h.0 as isize)
    }
}

#[cfg(not(windows))]
pub fn capture_foreground_target() -> TargetHwnd {
    TargetHwnd(0)
}

/// 仅激活目标窗口，供 Python/pyautogui 发送 Ctrl+C 前使用
pub fn focus_target_for_copy(target: TargetHwnd) -> Result<(), String> {
    #[cfg(windows)]
    {
        if !target.is_valid() {
            return Err("未找到前台窗口，请先聚焦到要翻译的应用".to_string());
        }
        use windows::Win32::Foundation::HWND;
        let hwnd = HWND(target.0 as *mut _);
        focus_target_window(hwnd)?;
        thread::sleep(Duration::from_millis(FOCUS_SETTLE_MS));
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        Ok(())
    }
}

#[cfg(windows)]
pub fn simulate_copy_to(target: TargetHwnd) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;

    if !target.is_valid() {
        return Err("未找到前台窗口，请先聚焦到要翻译的应用".to_string());
    }

    let hwnd = HWND(target.0 as *mut _);
    focus_target_for_copy(target)?;

    let focus_hwnd = focused_control(hwnd);
    send_wm_copy(focus_hwnd)?;
    thread::sleep(Duration::from_millis(40));

    // 部分应用只响应快捷键
    send_copy_sequence()?;
    thread::sleep(Duration::from_millis(POST_COPY_SETTLE_MS));
    Ok(())
}

#[cfg(not(windows))]
pub fn simulate_copy_to(_target: TargetHwnd) -> Result<(), String> {
    Err("当前平台不支持模拟复制".to_string())
}

/// 将译文粘贴到目标窗口（需先写入剪贴板并聚焦目标）
#[cfg(windows)]
pub fn simulate_paste_to(target: TargetHwnd) -> Result<(), String> {
    if !target.is_valid() {
        crate::capture_log::append(&format!(
            "=== [simulate-paste] {} abort: invalid_target ===\n",
            crate::capture_log::timestamp(),
        ));
        return Err("未找到目标窗口".to_string());
    }

    let fg_before = capture_foreground_target();
    crate::capture_log::append(&format!(
        "=== [simulate-paste:start] {} ===\ntarget_hwnd: {}\nforeground_before: {}\n",
        crate::capture_log::timestamp(),
        target.0,
        fg_before.0,
    ));

    // 必须在 Alt 抢焦点 / Ctrl+V 之前松开；否则 Shift+Enter 会变成 Alt+Shift 或 Ctrl+Shift+V
    let stuck = release_stuck_modifiers();
    crate::capture_log::append(&format!(
        "=== [simulate-paste:modifiers] {} ===\nreleased: {stuck}\n",
        crate::capture_log::timestamp(),
    ));
    if stuck != "none" {
        thread::sleep(Duration::from_millis(30));
    }

    if let Err(e) = focus_target_for_copy(target) {
        crate::capture_log::append(&format!(
            "=== [simulate-paste:focus] {} failed: {e} ===\n",
            crate::capture_log::timestamp(),
        ));
        return Err(e);
    }
    thread::sleep(Duration::from_millis(FOCUS_SETTLE_MS));

    let fg_after_focus = capture_foreground_target();
    let focus_ok = fg_after_focus.0 == target.0;
    crate::capture_log::append(&format!(
        "=== [simulate-paste:focus] {} ===\nforeground_after: {}\nfocus_matched_target: {focus_ok}\n",
        crate::capture_log::timestamp(),
        fg_after_focus.0,
    ));

    if let Err(e) = send_paste_sequence() {
        crate::capture_log::append(&format!(
            "=== [simulate-paste:send] {} failed: {e} ===\n",
            crate::capture_log::timestamp(),
        ));
        return Err(e);
    }
    thread::sleep(Duration::from_millis(POST_COPY_SETTLE_MS));

    let fg_after_paste = capture_foreground_target();
    crate::capture_log::append(&format!(
        "=== [simulate-paste:done] {} ===\nresult: ok\nforeground_after_paste: {}\n",
        crate::capture_log::timestamp(),
        fg_after_paste.0,
    ));
    Ok(())
}

#[cfg(not(windows))]
pub fn simulate_paste_to(_target: TargetHwnd) -> Result<(), String> {
    Err("当前平台不支持模拟粘贴".to_string())
}

#[cfg(windows)]
fn send_paste_sequence() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, VK_CONTROL, VK_V};

    let steps: &[(windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, bool)] = &[
        (VK_CONTROL, false),
        (VK_V, false),
        (VK_V, true),
        (VK_CONTROL, true),
    ];

    for (vk, up) in steps {
        let input = key_event(*vk, *up);
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent != 1 {
                return Err(format!("粘贴 SendInput 失败（已发送 {sent}/1）"));
            }
        }
        thread::sleep(Duration::from_millis(KEY_GAP_MS));
    }
    Ok(())
}

/// Shift+Enter 等热键触发时，物理修饰键常仍按住；若不先松开，Ctrl+V 会变成 Ctrl+Shift+V。
#[cfg(windows)]
fn release_stuck_modifiers() -> String {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, VK_CONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
        VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };

    let keys: &[(windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, &str)] = &[
        (VK_SHIFT, "Shift"),
        (VK_LSHIFT, "LShift"),
        (VK_RSHIFT, "RShift"),
        (VK_CONTROL, "Ctrl"),
        (VK_MENU, "Alt"),
        (VK_LMENU, "LAlt"),
        (VK_RMENU, "RAlt"),
        (VK_LWIN, "LWin"),
        (VK_RWIN, "RWin"),
    ];

    let mut released = Vec::new();
    for (vk, name) in keys {
        let down = unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 };
        if !down {
            continue;
        }
        let input = key_event(*vk, true);
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent == 1 {
                released.push(*name);
            }
        }
        thread::sleep(Duration::from_millis(KEY_GAP_MS));
    }

    if released.is_empty() {
        "none".to_string()
    } else {
        released.join(",")
    }
}

#[cfg(windows)]
fn focus_target_window(hwnd: windows::Win32::Foundation::HWND) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, SetForegroundWindow, ShowWindow, SW_SHOW,
    };

    unsafe {
        if hwnd == HWND::default() {
            return Err("无效窗口".to_string());
        }

        // 轻点 Alt，绕过 Windows 对 SetForegroundWindow 的限制
        keybd_event(VK_MENU.0 as u8, 0, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(
            VK_MENU.0 as u8,
            0,
            KEYEVENTF_KEYUP,
            0,
        );

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd)
            .ok()
            .map_err(|e| format!("无法激活目标窗口: {e}"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn focused_control(hwnd: windows::Win32::Foundation::HWND) -> windows::Win32::Foundation::HWND {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    unsafe {
        let thread = GetWindowThreadProcessId(hwnd, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(thread, &mut info).is_ok() && info.hwndFocus != HWND::default() {
            return info.hwndFocus;
        }
    }
    hwnd
}

#[cfg(windows)]
fn send_wm_copy(hwnd: windows::Win32::Foundation::HWND) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_COPY};

    unsafe {
        if hwnd == HWND::default() {
            return Ok(());
        }
        let _ = SendMessageW(hwnd, WM_COPY, WPARAM(0), LPARAM(0));
    }
    Ok(())
}

#[cfg(windows)]
fn send_copy_sequence() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, VK_CONTROL, VK_C};

    let steps: &[(windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, bool)] = &[
        (VK_CONTROL, false),
        (VK_C, false),
        (VK_C, true),
        (VK_CONTROL, true),
    ];

    for (vk, up) in steps {
        let input = key_event(*vk, *up);
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent != 1 {
                return Err(format!("SendInput 失败（已发送 {sent}/1）"));
            }
        }
        thread::sleep(Duration::from_millis(KEY_GAP_MS));
    }
    Ok(())
}

#[cfg(windows)]
fn key_event(
    vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    key_up: bool,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYBD_EVENT_FLAGS,
    };

    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS::default()
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
