use crate::capture::SelectionResult;
use crate::capture_log;
use crate::selection::TargetHwnd;
use crate::settings::TranslateSettings;
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CAPTURE_SCRIPT_ENV: &str = "QT_CAPTURE_SCRIPT";
const CLIPBOARD_BACKUP_ENV: &str = "QT_CLIPBOARD_BACKUP";

/// Windows：子进程不弹出 cmd 窗口（避免替换/取词时闪终端、抢焦点）
#[cfg(windows)]
fn configure_hidden_subprocess(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}

#[cfg(not(windows))]
fn configure_hidden_subprocess(cmd: &mut Command) {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PythonCaptureOutput {
    ok: bool,
    text: Option<String>,
    error: Option<String>,
    restored_clipboard: Option<bool>,
    diagnostics: Option<serde_json::Value>,
}

struct CaptureRunContext {
    python_bin: PathBuf,
    python_extra: Vec<String>,
    script: PathBuf,
    python_home: Option<PathBuf>,
    embedded_python: bool,
    exe_dir: Option<PathBuf>,
    delay_ms: u64,
    restore: bool,
    clipboard_backup: Option<String>,
}

fn is_no_selection_message(msg: &str) -> bool {
    let m = msg.trim();
    m.contains("请先选中") || m.contains("empty-clipboard") || m.contains("无文本") || m.contains("未选中")
}

pub fn capture_via_python(
    settings: &TranslateSettings,
    target: TargetHwnd,
) -> SelectionResult {
    let started = Instant::now();

    if let Err(e) = crate::selection::focus_target_for_copy(target) {
        return SelectionResult {
            ok: false,
            text: None,
            error: Some(e),
            restored_clipboard: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let clipboard_backup = read_system_clipboard();
    let copy_delay = settings.copy_delay_ms.max(80);

    // 纯 Python 取词：pyautogui Ctrl+C + pyperclip（不使用 Rust 模拟复制，避免多余子进程/终端）
    let script = match capture_script_path() {
        Ok(p) => p,
        Err(e) => {
            log_capture_failure("resolve-script", &e, None);
            return SelectionResult {
                ok: false,
                text: None,
                error: Some(format!("{e}\n{}", capture_log::log_file_hint_block())),
                restored_clipboard: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    let (python_bin, python_extra) = match resolve_python_bin() {
        Ok(v) => v,
        Err(e) => {
            log_capture_failure("resolve-python", &e, None);
            return SelectionResult {
                ok: false,
                text: None,
                error: Some(format!("{e}\n{}", capture_log::log_file_hint_block())),
                restored_clipboard: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    let python_home = embedded_python_home(&python_bin);
    let embedded_python = python_home
        .as_ref()
        .map(|h| is_embedded_python_home(h))
        .unwrap_or(false);

    // Python 发 Ctrl+C 前再次聚焦目标窗口（热键后焦点可能已偏移）
    let _ = crate::selection::focus_target_for_copy(target);
    thread::sleep(Duration::from_millis(80));

    let ctx = CaptureRunContext {
        python_home,
        embedded_python,
        exe_dir: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| subprocess_path(d))),
        python_bin: subprocess_path(&python_bin),
        python_extra,
        script: subprocess_path(&script),
        delay_ms: copy_delay,
        restore: settings.restore_clipboard,
        clipboard_backup,
    };

    let output = match run_capture_script(&ctx) {
        Ok(o) => o,
        Err(e) => {
            log_capture_failure("spawn", &e, Some(&ctx));
            return SelectionResult {
                ok: false,
                text: None,
                error: Some(format_user_error("无法启动 Python 取词", &e, &ctx)),
                restored_clipboard: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    log_capture_output(&ctx, &output);

    let stdout = crate::text_encoding::decode_bytes(&output.stdout);
    let stderr = crate::text_encoding::decode_bytes(&output.stderr);
    if stdout.trim().is_empty() {
        let detail = build_process_detail(&ctx, &output, &stdout, &stderr);
        let user_msg = format_user_error("Python 取词无输出", &detail, &ctx);
        return SelectionResult {
            ok: false,
            text: None,
            error: Some(user_msg),
            restored_clipboard: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let parsed: PythonCaptureOutput = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => {
            let detail = build_process_detail(&ctx, &output, &stdout, &stderr);
            return SelectionResult {
                ok: false,
                text: None,
                error: Some(format!(
                    "解析 Python 输出失败: {e}\n{detail}\n{}",
                    capture_log::log_file_hint_block()
                )),
                restored_clipboard: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    if !parsed.ok {
        let mut msg = parsed
            .error
            .unwrap_or_else(|| "Python 取词失败".to_string());
        let is_no_selection = msg.contains("剪贴板无")
            || msg.contains("empty-clipboard")
            || msg.contains("无文本")
            || is_no_selection_message(&msg);
        if is_no_selection {
            msg = "请先选中要翻译的文字".to_string();
        }
        if !is_no_selection {
            if let Some(diag) = parsed.diagnostics {
                msg.push_str(&format!("\n诊断: {diag}"));
            }
            msg.push_str(&format!("\n{}", capture_log::log_file_hint_block()));
        }
        return SelectionResult {
            ok: false,
            text: None,
            error: Some(msg),
            restored_clipboard: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let text = parsed.text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return SelectionResult {
            ok: false,
            text: None,
            error: Some("请先选中要翻译的文字".to_string()),
            restored_clipboard: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    SelectionResult {
        ok: true,
        text: Some(text),
        error: None,
        restored_clipboard: parsed.restored_clipboard.unwrap_or(false),
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

/// 去掉 Windows 扩展路径前缀 `\\?\`，避免传给 Python 子进程时异常。
fn subprocess_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let stripped = s
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| s.strip_prefix(r"\\?\").map(|rest| rest.to_string()))
        .unwrap_or_else(|| s.into_owned());
    PathBuf::from(stripped)
}

fn quote_path(path: &Path) -> String {
    format!("{path:?}")
}

fn is_embedded_python_home(home: &Path) -> bool {
    if home.join("python312.zip").is_file() {
        return true;
    }
    std::fs::read_dir(home)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|x| x == "zip"))
        })
        .unwrap_or(false)
}

fn log_capture_failure(tag: &str, message: &str, ctx: Option<&CaptureRunContext>) {
    let mut block = format!("\n=== [{tag}] {} ===\n{message}\n", local_timestamp());
    if let Some(c) = ctx {
        block.push_str(&format_context(c));
    }
    capture_log::append(&block);
}

fn log_capture_output(ctx: &CaptureRunContext, output: &Output) {
    let stdout = crate::text_encoding::decode_bytes(&output.stdout);
    let stderr = crate::text_encoding::decode_bytes(&output.stderr);
    let mut block = format!(
        "\n=== [capture] {} exit={:?} ===\n",
        local_timestamp(),
        output.status.code()
    );
    block.push_str(&format_context(ctx));
    block.push_str(&format!(
        "stdout ({} bytes):\n{}\nstderr ({} bytes):\n{}\n",
        output.stdout.len(),
        truncate(&stdout, 4000),
        output.stderr.len(),
        truncate(&stderr, 4000),
    ));
    capture_log::append(&block);
}

fn local_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix={secs}")
}

fn format_context(ctx: &CaptureRunContext) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "app_exe_dir: {}",
        ctx.exe_dir
            .as_ref()
            .map(|p| quote_path(p))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(s, "python_bin: {}", quote_path(&ctx.python_bin));
    let _ = writeln!(
        s,
        "python_bin_exists: {}",
        ctx.python_bin.is_file()
    );
    let _ = writeln!(s, "python_extra: {:?}", ctx.python_extra);
    let _ = writeln!(
        s,
        "python_home: {}",
        ctx.python_home
            .as_ref()
            .map(|p| quote_path(p))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(s, "embedded_python: {}", ctx.embedded_python);
    let _ = writeln!(s, "script: {}", quote_path(&ctx.script));
    let _ = writeln!(s, "script_exists: {}", ctx.script.is_file());
    let _ = writeln!(s, "delay_ms: {}", ctx.delay_ms);
    let _ = writeln!(s, "restore_clipboard: {}", ctx.restore);
    let _ = writeln!(s, "run_mode: python_only_pyautogui");
    let _ = writeln!(
        s,
        "clipboard_backup_len: {}",
        ctx.clipboard_backup.as_ref().map(|b| b.len()).unwrap_or(0)
    );
    s
}

fn read_system_clipboard() -> Option<String> {
    crate::clipboard_util::read_arboard()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max).collect();
    format!("{t}…")
}

fn build_process_detail(
    ctx: &CaptureRunContext,
    output: &Output,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut s = format_context(ctx);
    let _ = writeln!(
        s,
        "exit_code: {:?}",
        output.status.code()
    );
    if !stdout.is_empty() {
        let _ = writeln!(s, "stdout: {}", truncate(stdout, 800));
    }
    if !stderr.is_empty() {
        let _ = writeln!(s, "stderr: {}", truncate(stderr, 2000));
    }
    s
}

fn format_user_error(title: &str, detail: &str, ctx: &CaptureRunContext) -> String {
    format!(
        "{title}\n{}\n{}\nPython: {}\n脚本: {}",
        truncate(detail, 1200),
        capture_log::log_file_hint_block(),
        quote_path(&ctx.python_bin),
        quote_path(&ctx.script),
    )
}

fn bundled_python_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    // 仅使用 pythonw.exe，避免 python.exe 弹出终端
    for rel in [
        "python/pythonw.exe",
        "resources/python/pythonw.exe",
    ] {
        let p = dir.join(rel);
        if p.is_file() {
            return Some(subprocess_path(&p));
        }
    }
    None
}

fn embedded_python_home(bin: &Path) -> Option<PathBuf> {
    let name = bin.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name == "python.exe" || name == "pythonw.exe" || name == "python3.exe" {
        return bin.parent().map(|p| subprocess_path(p));
    }
    None
}

fn resolve_python_bin() -> Result<(PathBuf, Vec<String>), String> {
    if let Some(p) = bundled_python_exe() {
        return Ok((p, vec![]));
    }

    #[cfg(windows)]
    {
        for bin in ["pythonw", "pythonw.exe"] {
            if python_usable(bin, &[]) {
                return Ok((PathBuf::from(bin), vec![]));
            }
        }
    }

    if let Some(found) = attempts_dev_python() {
        return found;
    }

    Err(
        "未找到 pythonw.exe（安装目录应有 python/pythonw.exe）。请执行 npm run prepare:python 后重新打包。"
            .to_string(),
    )
}

#[cfg(windows)]
fn attempts_dev_python() -> Option<Result<(PathBuf, Vec<String>), String>> {
    if python_usable("pythonw", &[]) {
        return Some(Ok((PathBuf::from("pythonw"), vec![])));
    }
    None
}

#[cfg(not(windows))]
fn attempts_dev_python() -> Option<Result<(PathBuf, Vec<String>), String>> {
    if python_usable("python3", &[]) {
        return Some(Ok((PathBuf::from("python3"), vec![])));
    }
    if python_usable("python", &[]) {
        return Some(Ok((PathBuf::from("python"), vec![])));
    }
    None
}

fn python_usable(bin: &str, extra_args: &[&str]) -> bool {
    let mut cmd = Command::new(bin);
    configure_hidden_subprocess(&mut cmd);
    for a in extra_args {
        cmd.arg(a);
    }
    if let Some(home) = embedded_python_home(Path::new(bin)) {
        apply_python_env(&mut cmd, &home, is_embedded_python_home(&home));
    }
    cmd.arg("-c")
        .arg("import sys")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 嵌入式 Python 靠 `._pth` + current_dir，不设 PYTHONHOME（路径含空格时更稳）。
fn apply_python_env(cmd: &mut Command, home: &Path, embedded: bool) {
    let home = subprocess_path(home);
    cmd.current_dir(&home);
    cmd.env_remove("PYTHONHOME");
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONNOUSERSITE", "1");
    if !embedded {
        cmd.env("PYTHONHOME", &home);
    }
    let site = home.join("Lib").join("site-packages");
    if site.is_dir() {
        cmd.env("PYTHONPATH", &site);
    }
}

fn capture_runner_code(delay_ms: u64, restore: bool) -> String {
    let restore_lit = if restore { "True" } else { "False" };
    format!(
        r#"import os, runpy, sys
script = os.environ[{env_key:?}]
delay = {delay_ms}
restore = {restore_lit}
argv = [script, "--delay-ms", str(delay)]
if restore:
    argv.append("--restore")
sys.argv = argv
runpy.run_path(script, run_name="__main__")
"#,
        env_key = CAPTURE_SCRIPT_ENV,
    )
}

fn build_capture_command(ctx: &CaptureRunContext) -> Command {
    let mut cmd = Command::new(&ctx.python_bin);
    configure_hidden_subprocess(&mut cmd);
    for arg in &ctx.python_extra {
        cmd.arg(arg);
    }
    if let Some(home) = &ctx.python_home {
        apply_python_env(&mut cmd, home, ctx.embedded_python);
    } else {
        cmd.env("PYTHONIOENCODING", "utf-8");
        cmd.env("PYTHONUTF8", "1");
    }
    cmd.env(CAPTURE_SCRIPT_ENV, &ctx.script);
    if let Some(ref backup) = ctx.clipboard_backup {
        cmd.env(CLIPBOARD_BACKUP_ENV, backup);
    } else {
        cmd.env_remove(CLIPBOARD_BACKUP_ENV);
    }
    cmd.arg("-u")
        .arg("-c")
        .arg(capture_runner_code(ctx.delay_ms, ctx.restore));
    cmd
}

fn run_capture_script(ctx: &CaptureRunContext) -> Result<Output, String> {
    build_capture_command(ctx)
        .output()
        .map_err(|e| format!("无法启动 {}: {e}", quote_path(&ctx.python_bin)))
}

fn capture_script_path() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let dir = subprocess_path(dir);
            for rel in [
                "resources/capture_clipboard.py",
                "scripts/capture_clipboard.py",
                "capture_clipboard.py",
            ] {
                let p = dir.join(rel);
                if p.is_file() {
                    return Ok(subprocess_path(&p));
                }
            }
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for p in [
        manifest.join("resources/capture_clipboard.py"),
        manifest.join("../../workspaces/quick_translate/capture_clipboard.py"),
    ] {
        if p.is_file() {
            return Ok(subprocess_path(&p));
        }
    }
    Err("找不到取词脚本 capture_clipboard.py".to_string())
}
