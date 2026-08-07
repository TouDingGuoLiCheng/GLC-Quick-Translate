#!/usr/bin/env python3

"""

热键取词：读取剪贴板文本。复制由 Rust 侧完成（避免 pyautogui Ctrl+C 中断子进程）。

stdout 输出 JSON：{ ok, text?, error?, restoredClipboard?, diagnostics? }

"""

from __future__ import annotations



import json

import os

import sys

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





def main() -> int:

    import argparse

    import time



    parser = argparse.ArgumentParser(description="读取剪贴板（复制由宿主完成）")

    parser.add_argument("--delay-ms", type=int, default=250, help="读取前等待毫秒")

    parser.add_argument(

        "--clipboard-only",

        action="store_true",

        help="不发送 Ctrl+C，仅读取剪贴板",

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

            import pyautogui



            pyautogui.FAILSAFE = False

            pyautogui.PAUSE = 0.02

            pyautogui.hotkey("ctrl", "c")

        except ImportError as exc:

            _emit(False, error=f"未安装 pyautogui: {exc}", stage="import-pyautogui")

            return 1

        except Exception as exc:

            _emit(False, error=f"模拟 Ctrl+C 失败: {exc}", stage="hotkey")

            return 1



    delay_sec = max(args.delay_ms, 80) / 1000.0

    if not args.clipboard_only:

        time.sleep(delay_sec)

    elif delay_sec > 0:

        time.sleep(min(delay_sec, 0.15))



    backup_norm = backup.strip()

    captured = ""

    for _ in range(24):

        try:

            captured = pyperclip.paste() or ""

        except Exception as exc:

            _emit(False, error=f"pyperclip 读取失败: {exc}", stage="clipboard-read")

            return 1



        text = captured.strip()

        if text and (not backup_norm or text != backup_norm):

            break

        time.sleep(0.05)



    if not captured.strip():
        # 热键触发取词时，如果剪贴板未更新，视为未选中文本，避免回退旧剪贴板内容。
        if not args.clipboard_only:
            _emit(False, error="剪贴板无文本，请先选中可复制的内容", stage="empty-clipboard")
            return 1
        if backup_norm:
            captured = backup_norm
        else:
            _emit(False, error="剪贴板无文本，请先选中可复制的内容", stage="empty-clipboard")
            return 1



    restored = False

    if args.restore and backup_norm:

        try:

            pyperclip.copy(backup)

            restored = True

        except Exception:

            restored = False



    _emit(True, text=captured.strip(), restored_clipboard=restored, stage="done")

    return 0





if __name__ == "__main__":

    try:

        raise SystemExit(main())

    except SystemExit:

        raise

    except Exception:

        _emit(False, error=traceback.format_exc(), stage="fatal")

        raise SystemExit(1) from None

