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
BACKUP_SEQ_ENV = "QT_CLIPBOARD_BACKUP_SEQ"
COPY_MAX_WAIT_ENV = "QT_COPY_MAX_WAIT_SEC"


def _copy_max_wait_sec() -> float:
    raw = os.environ.get(COPY_MAX_WAIT_ENV, "5")
    try:
        sec = float(raw)
    except ValueError:
        sec = 5.0
    return max(1.0, min(sec, 30.0))


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


def _load_backup_seq() -> int | None:
    raw = os.environ.get(BACKUP_SEQ_ENV, "").strip()
    if not raw:
        return None
    try:
        return int(raw)
    except ValueError:
        return None


def _clipboard_sequence_number() -> int | None:
    if sys.platform != "win32":
        return None
    try:
        import ctypes

        return int(ctypes.windll.user32.GetClipboardSequenceNumber())
    except Exception:
        return None


def _copy_succeeded(
    backup_norm: str, backup_seq: int | None, text: str, current_seq: int | None
) -> bool:
    text = text.strip()
    if not text:
        return False
    if backup_seq is not None and current_seq is not None:
        return current_seq != backup_seq
    return not backup_norm or text != backup_norm


def _simulate_copy() -> None:
    import pyautogui

    pyautogui.FAILSAFE = False
    pyautogui.PAUSE = 0.02
    pyautogui.hotkey("ctrl", "c")


def _clear_clipboard() -> None:
    import pyperclip

    pyperclip.copy("")
    if sys.platform != "win32":
        return
    try:
        import ctypes

        user32 = ctypes.windll.user32
        if user32.OpenClipboard(0):
            try:
                user32.EmptyClipboard()
            finally:
                user32.CloseClipboard()
    except Exception:
        pass


def _read_captured(
    backup_norm: str,
    backup_seq: int | None,
    delay_ms: int,
    after_hotkey: bool,
    copy_started: float | None,
) -> str:
    import pyperclip

    delay_sec = max(delay_ms, 80) / 1000.0
    if after_hotkey:
        if delay_sec > 0:
            time.sleep(delay_sec)
    elif delay_sec > 0:
        time.sleep(min(delay_sec, 0.15))

    max_wait = _copy_max_wait_sec()
    poll_interval = 0.05
    # 等待窗口从复制延迟之后算起，与设置里的「复制延迟」不是同一参数
    deadline = time.monotonic() + max_wait

    captured = ""
    while time.monotonic() <= deadline:
        try:
            captured = pyperclip.paste() or ""
        except Exception as exc:
            _emit(False, error=f"pyperclip 读取失败: {exc}", stage="clipboard-read")
            raise SystemExit(1) from exc

        text = captured.strip()
        current_seq = _clipboard_sequence_number()
        if _copy_succeeded(backup_norm, backup_seq, text, current_seq):
            return text
        time.sleep(poll_interval)

    try:
        final = (pyperclip.paste() or "").strip()
    except Exception as exc:
        _emit(False, error=f"pyperclip 读取失败: {exc}", stage="clipboard-read")
        raise SystemExit(1) from exc

    final_seq = _clipboard_sequence_number()
    if _copy_succeeded(backup_norm, backup_seq, final, final_seq):
        return final

    elapsed = time.monotonic() - (copy_started or time.monotonic())
    if not final:
        _emit(False, error="请先选中要翻译的文字", stage="empty-clipboard")
        raise SystemExit(1)
    if (
        backup_seq is not None
        and final_seq is not None
        and final_seq == backup_seq
    ):
        _emit(
            False,
            error=(
                f"复制失败：超过 {max_wait:.0f} 秒剪贴板序号未变化，"
                f"可能未成功复制选中内容（等待 {elapsed:.2f}s）"
            ),
            stage="copy-timeout-stale",
        )
        raise SystemExit(1)
    _emit(
        False,
        error=(
            f"复制失败：超过 {max_wait:.0f} 秒剪贴板仍未更新，"
            f"可能仍在使用复制前的旧内容（等待 {elapsed:.2f}s）"
        ),
        stage="copy-timeout-stale",
    )
    raise SystemExit(1)


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

    copy_started: float | None = None
    if args.clipboard_only:
        backup = _load_backup()
        backup_seq = _load_backup_seq()
        copy_started = time.monotonic()
    else:
        backup = _load_backup()
        copy_started = time.monotonic()
        # 复制前一刻的序号（与 Ctrl+C 同进程）；不用宿主热键时的序号，避免子进程启动间隔造成误判
        seq_before_copy = _clipboard_sequence_number()
        try:
            _simulate_copy()
        except ImportError as exc:
            _emit(False, error=f"未安装 pyautogui: {exc}", stage="import-pyautogui")
            return 1
        except Exception as exc:
            _emit(False, error=f"模拟 Ctrl+C 失败: {exc}", stage="hotkey")
            return 1
        backup_seq = seq_before_copy

    backup_norm = backup.strip()
    captured = _read_captured(
        backup_norm,
        backup_seq,
        args.delay_ms,
        after_hotkey=not args.clipboard_only,
        copy_started=copy_started,
    )
    if not captured:
        _emit(False, error="请先选中要翻译的文字", stage="empty-clipboard")
        return 1

    restored = False
    if args.restore:
        try:
            if backup_norm:
                pyperclip.copy(backup)
            else:
                _clear_clipboard()
            restored = True
        except Exception:
            restored = False

    diag = _diagnostics()
    diag["stage"] = "done"
    diag["clipboardBackupSeq"] = backup_seq
    diag["clipboardSeqAfter"] = _clipboard_sequence_number()
    payload = {
        "ok": True,
        "text": captured,
        "error": None,
        "restoredClipboard": restored,
        "diagnostics": diag,
    }
    print(json.dumps(payload, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except Exception:
        _emit(False, error=traceback.format_exc(), stage="fatal")
        raise SystemExit(1) from None
