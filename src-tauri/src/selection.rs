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
        return Err("未找到目标窗口".to_string());
    }
    focus_target_for_copy(target)?;
    thread::sleep(Duration::from_millis(FOCUS_SETTLE_MS));
    send_paste_sequence()?;
    thread::sleep(Duration::from_millis(POST_COPY_SETTLE_MS));
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
