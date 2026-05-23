#!/usr/bin/env python3
"""
热键取词：默认由本脚本模拟 Ctrl+C 并读取剪贴板；宿主可先聚焦目标窗口。

stdout 输出 JSON：{ ok, text?, error?, restoredClipboard?, diagnostics? }
"""

from __future__ import annotations

import json
import os
import sys
import time
import traceback

if hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

BACKUP_ENV = "QT_CLIPBOARD_BACKUP"


def _diagnostics() -> dict:
    return {
        "executable": sys.executable,
        "version": sys.version.replace("\n", " "),
        "cwd": os.getcwd(),
        "pythonhome": os.environ.get("PYTHONHOME"),
        "pythonpath": os.environ.get("PYTHONPATH"),
        "sysPathHead": sys.path[:12],
        "stage": "runtime",
    }


def _emit(
    ok: bool,
    *,
    text: str | None = None,
    error: str | None = None,
    restored_clipboard: bool = False,
    stage: str = "main",
) -> None:
    diag = _diagnostics()
    diag["stage"] = stage
    payload = {
        "ok": ok,
        "text": text,
        "error": error,
        "restoredClipboard": restored_clipboard,
        "diagnostics": diag,
    }
    print(json.dumps(payload, ensure_ascii=False), flush=True)


def _load_backup() -> str:
    if BACKUP_ENV in os.environ:
        return os.environ.get(BACKUP_ENV) or ""
    try:
        import pyperclip

        return pyperclip.paste() or ""
    except Exception:
        return ""


def _simulate_copy() -> None:
    import pyautogui

    pyautogui.FAILSAFE = False
    pyautogui.PAUSE = 0.02
    pyautogui.hotkey("ctrl", "c")


def _read_captured(backup_norm: str, delay_ms: int, after_hotkey: bool) -> str:
    import pyperclip

    delay_sec = max(delay_ms, 80) / 1000.0
    if after_hotkey:
        if delay_sec > 0:
            time.sleep(delay_sec)
    elif delay_sec > 0:
        time.sleep(min(delay_sec, 0.15))

    captured = ""
    for _ in range(24):
        try:
            captured = pyperclip.paste() or ""
        except Exception as exc:
            _emit(False, error=f"pyperclip 读取失败: {exc}", stage="clipboard-read")
            raise SystemExit(1) from exc

        text = captured.strip()
        if text and (not backup_norm or text != backup_norm):
            break
        time.sleep(0.05)

    if not captured.strip():
        # 热键触发取词时，如果剪贴板未更新，视为未选中文本，避免回退旧剪贴板内容。
        if after_hotkey:
            return ""
        if backup_norm:
            return backup_norm
        return ""
    return captured.strip()


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description="取词：模拟复制并读取剪贴板")
    parser.add_argument("--delay-ms", type=int, default=250, help="复制后等待毫秒")
    parser.add_argument(
        "--clipboard-only",
        action="store_true",
        help="不发送 Ctrl+C，仅读取剪贴板（宿主已完成复制）",
    )
    parser.add_argument(
        "--restore",
        action="store_true",
        help="读取后把剪贴板恢复为复制前内容",
    )
    args = parser.parse_args()

    try:
        import pyperclip
    except ImportError as exc:
        _emit(False, error=f"未安装 pyperclip: {exc}", stage="import-pyperclip")
        return 1

    backup = _load_backup() if args.clipboard_only else ""
    if not args.clipboard_only:
        try:
            backup = pyperclip.paste() or ""
        except Exception as exc:
            _emit(False, error=f"备份剪贴板失败: {exc}", stage="clipboard-backup")
            return 1
        try:
            _simulate_copy()
        except ImportError as exc:
            _emit(False, error=f"未安装 pyautogui: {exc}", stage="import-pyautogui")
            return 1
        except Exception as exc:
            _emit(False, error=f"模拟 Ctrl+C 失败: {exc}", stage="hotkey")
            return 1

    backup_norm = backup.strip()
    captured = _read_captured(backup_norm, args.delay_ms, after_hotkey=not args.clipboard_only)
    if not captured:
        _emit(False, error="请先选中要翻译的文字", stage="empty-clipboard")
        return 1

    restored = False
    if args.restore and backup_norm:
        try:
            pyperclip.copy(backup)
            restored = True
        except Exception:
            restored = False

    _emit(True, text=captured, restored_clipboard=restored, stage="done")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except Exception:
        _emit(False, error=traceback.format_exc(), stage="fatal")
        raise SystemExit(1) from None
