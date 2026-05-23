use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

const MAIN_MARGIN: i32 = 14;

/// 主窗口：屏幕右下角（工作区内）
pub fn place_main_bottom_right(window: &WebviewWindow) -> Result<(), String> {
    let size = window
        .outer_size()
        .map_err(|e| format!("读取窗口尺寸失败: {e}"))?;
    let (area_x, area_y, area_w, area_h) = monitor_work_area_near_cursor()?;
    let w = size.width as i32;
    let h = size.height as i32;
    let x = area_x + area_w as i32 - w - MAIN_MARGIN;
    let y = area_y + area_h as i32 - h - MAIN_MARGIN;
    window
        .set_position(PhysicalPosition::new(x.max(area_x), y.max(area_y)))
        .map_err(|e| format!("设置主窗口位置失败: {e}"))?;
    Ok(())
}

/// 光标所在显示器的工作区 (x, y, width, height)
pub fn monitor_work_area_near_cursor() -> Result<(i32, i32, i32, i32), String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{POINT, RECT};
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt).map_err(|e| format!("GetCursorPos 失败: {e}"))?;
            let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(hmon, &mut info).as_bool() {
                return Err("GetMonitorInfo 失败".to_string());
            }
            let r: RECT = info.rcWork;
            Ok((r.left, r.top, r.right - r.left, r.bottom - r.top))
        }
    }
    #[cfg(not(windows))]
    {
        Ok((0, 0, 1920, 1080))
    }
}

#[allow(dead_code)]
pub fn clamp_size(width: u32, height: u32) -> PhysicalSize<u32> {
    PhysicalSize::new(width, height)
}
