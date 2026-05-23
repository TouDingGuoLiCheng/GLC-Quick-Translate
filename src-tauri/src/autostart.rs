//! Windows 当前用户开机启动（注册表 Run）

#[cfg(windows)]
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const APP_RUN_NAME: &str = "GLC Quick Translate";

#[cfg(windows)]
pub fn apply(enabled: bool) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE)
        .map_err(|e| format!("打开注册表失败: {e}"))?;

    if enabled {
        run.set_value(APP_RUN_NAME, &exe)
            .map_err(|e| format!("写入开机启动失败: {e}"))?;
    } else {
        let _ = run.delete_value(APP_RUN_NAME);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn apply(_enabled: bool) -> Result<(), String> {
    Ok(())
}
