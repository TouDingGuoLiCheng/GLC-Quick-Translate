# GLC Quick Translate

一个基于 **Tauri 2 + Vue 3** 的 Windows 桌面划词翻译工具。  
选中文字后按快捷键即可快速翻译，支持气泡展示和原位替换两种模式。

## 功能特性

- 全局快捷键触发：跨应用选中文本后即时翻译
- 两种工作模式：
  - `Ctrl+T`：显示翻译气泡
  - `Ctrl+Shift+T`：用译文替换选中内容
- 托盘常驻：关闭主窗口不退出，右键托盘菜单可退出程序
- 翻译记录：支持查看历史、复制原文/译文、重译、删除
- 可配置项：快捷键、目标语言、翻译引擎、超时、缓存、外观主题等
- 嵌入式 Python 取词兜底：发布包可开箱即用，无需用户本地安装 Python

## 技术栈

- 前端：`Vue 3`、`Vite`、`TypeScript`
- 桌面壳：`Tauri 2`
- 后端：`Rust`
- 剪贴板与快捷键：`tauri-plugin-clipboard-manager`、`tauri-plugin-global-shortcut`

## 快速开始（开发）

### 1) 环境要求

- Node.js 18+
- Rust stable（建议通过 [rustup](https://rustup.rs/) 安装）
- Windows 下用于编译的 Visual Studio C++ Build Tools

### 2) 启动开发

```powershell
cd "d:\VS\工具箱开发\quick-translate"
npm install
npm run tauri dev
```

说明：

- 前端开发端口：`1422`
- 主窗口关闭后应用仍驻留托盘

## 配置说明

默认快捷键：

- 气泡翻译：`Ctrl+T`
- 替换翻译：`Ctrl+Shift+T`

常用设置：

- `targetLang`：目标语言（默认 `auto`）
- `primaryProvider`：主翻译引擎（默认 `baidu`）
- `fallbackEnabled`：主引擎失败后是否回退
- `bubbleAutoCloseSec`：气泡自动关闭时间

## 打包发布（Windows）

### 一键构建

```powershell
cd "d:\VS\工具箱开发\quick-translate"
npm install
npm run tauri build
```

常见产物路径：

- 安装包：`src-tauri\target\release\bundle\nsis\GLC Quick Translate_0.1.0_x64-setup.exe`
- 可执行文件：`src-tauri\target\release\GLC Quick Translate.exe`

### 可选：预先准备内置 Python

```powershell
npm run prepare:python
```

该命令会生成 `src-tauri/bundled-python/`，用于发布包内置 Python 运行环境。

## 目录结构

```text
quick-translate/
├─ src/                  # Vue 前端
├─ src-tauri/            # Rust + Tauri 后端
├─ scripts/              # 构建辅助脚本
└─ README.md
```

## 常见问题

### 1) 为什么关闭窗口后程序还在？

这是设计行为：程序会最小化为托盘常驻，便于全局快捷键随时可用。

### 2) 替换模式会覆盖原文吗？

仅在成功取词且翻译成功时替换；空白选择不会替换原有文字。

### 3) 取词失败怎么办？

请确认目标应用中已实际选中文本，再触发快捷键。部分应用对模拟复制响应较慢，可在设置里适当调大复制等待时间。

## 开发备注

- 项目开发方案可参考：[`../快捷翻译-开发方案.md`](../快捷翻译-开发方案.md)
- 当前版本：`0.1.0`

## License

如需开源发布，建议补充本节许可证信息（例如 MIT / Apache-2.0）。
