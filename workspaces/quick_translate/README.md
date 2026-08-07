# 快捷翻译 · Python 取词脚本

供 `quick-translate` Tauri 应用通过子进程调用。

## 安装依赖

```powershell
cd "d:\VS\quick-translate\workspaces\quick_translate"
python -m pip install -r requirements.txt
```

## 单独测试

```powershell
# 先在记事本选中文字，再执行：
python capture_clipboard.py --delay-ms 250
```

成功时 stdout 示例：

```json
{"ok": true, "text": "选中的内容", "error": null, "restoredClipboard": false}
```
