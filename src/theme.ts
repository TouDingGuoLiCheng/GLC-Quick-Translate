import {
  layoutForPhase,
  layoutToBackgroundStyle,
  normalizeBubbleBgLayouts,
  pickLayoutFromSettings,
  type BubbleBgVariant,
} from "./bubbleBgLayout";
import type { TranslateSettings } from "./types";

type ThemeKey = "dark" | "light";

export type BubbleAppearance = Pick<
  TranslateSettings,
  "appTheme" | "bubbleOpacity" | "bubbleBgLayouts" | "bubbleTextColor" | "bubbleMutedColor"
>;

const PALETTE: Record<
  ThemeKey,
  { shell: string; surface: string; settings: string; settingsInput: string; text: string; muted: string }
> = {
  dark: {
    shell: "rgb(20, 20, 26)",
    surface: "rgb(36, 36, 46)",
    settings: "rgb(28, 28, 36)",
    settingsInput: "rgb(20, 20, 28)",
    text: "#e8e8ef",
    muted: "#9a9aad",
  },
  light: {
    shell: "rgb(245, 245, 248)",
    surface: "rgb(255, 255, 255)",
    settings: "rgb(255, 255, 255)",
    settingsInput: "rgb(245, 245, 250)",
    text: "#1a1a22",
    muted: "#5c5c6e",
  },
};

const BUBBLE_RGB: Record<ThemeKey, [number, number, number]> = {
  dark: [30, 30, 40],
  light: [255, 255, 255],
};

function clampPercent(v: number | undefined, fallback = 92) {
  const n = typeof v === "number" && !Number.isNaN(v) ? v : fallback;
  return Math.min(100, Math.max(50, n)) / 100;
}

function themeKey(appTheme: string): ThemeKey {
  return appTheme === "light" ? "light" : "dark";
}

export function themeDefaultTextColors(appTheme: string) {
  const p = PALETTE[themeKey(appTheme)];
  return { text: p.text, muted: p.muted };
}

/** 气泡文字 CSS 变量 */
export function bubbleTextCssVars(
  settings: Pick<TranslateSettings, "appTheme" | "bubbleTextColor" | "bubbleMutedColor">,
): Record<string, string> {
  const fallback = themeDefaultTextColors(settings.appTheme);
  const text = settings.bubbleTextColor?.trim() || fallback.text;
  const muted = settings.bubbleMutedColor?.trim() || fallback.muted;
  return {
    "--bubble-text": text,
    "--bubble-muted": muted,
  };
}

export function hasBubbleBackground(
  settings: Pick<TranslateSettings, "bubbleBackground">,
  dataUrl?: string | null,
) {
  return !!(settings.bubbleBackground?.trim() || dataUrl?.trim());
}

/** 主窗口：纯色背景，无透明度调节 */
export function applyAppTheme(settings: Pick<TranslateSettings, "appTheme">) {
  const root = document.documentElement;
  const key = themeKey(settings.appTheme);
  root.classList.remove("theme-dark", "theme-light");
  root.classList.add(`theme-${key}`);

  const p = PALETTE[key];
  root.style.setProperty("--shell-bg", p.shell);
  root.style.setProperty("--surface", p.surface);
  root.style.setProperty("--settings-panel-bg", p.settings);
  root.style.setProperty("--settings-input-bg", p.settingsInput);

  document.body.style.background = p.shell;
  const app = document.getElementById("app");
  if (app) app.style.background = p.shell;
}

/** 气泡卡片样式（用于气泡窗口与设置内预览） */
export function bubbleCardStyle(
  settings: BubbleAppearance,
  backgroundDataUrl: string | null | undefined,
  variant: BubbleBgVariant | string = "success",
): Record<string, string> {
  const key = themeKey(settings.appTheme);
  const dataUrl = backgroundDataUrl?.trim();
  const phase = variant === "loading" ? "loading" : "success";
  const layouts = normalizeBubbleBgLayouts(settings.bubbleBgLayouts);
  const textVars = bubbleTextCssVars(settings);

  if (dataUrl) {
    const layout = layoutForPhase(phase, layouts);
    return {
      ...layoutToBackgroundStyle(layout, dataUrl),
      ...textVars,
      border: "1px solid var(--border)",
      color: "var(--bubble-text)",
    };
  }

  const a = clampPercent(settings.bubbleOpacity);
  const [r, g, b] = BUBBLE_RGB[key];
  return {
    ...textVars,
    backgroundColor: `rgba(${r}, ${g}, ${b}, ${a})`,
    border: "1px solid var(--border)",
    color: "var(--bubble-text)",
  };
}

/** 气泡窗口：应用主题类与文字色变量 */
export function applyBubbleTheme(
  settings: BubbleAppearance,
  backgroundDataUrl?: string | null,
  variant: BubbleBgVariant | string = "success",
) {
  const root = document.documentElement;
  const key = themeKey(settings.appTheme);
  root.classList.remove("theme-dark", "theme-light");
  root.classList.add(`theme-${key}`);

  const vars = bubbleTextCssVars(settings);
  for (const [k, v] of Object.entries(vars)) {
    root.style.setProperty(k, v);
  }

  const style = bubbleCardStyle(settings, backgroundDataUrl, variant);
  for (const [k, v] of Object.entries(style)) {
    if (k.startsWith("--")) {
      root.style.setProperty(k, v);
    }
  }
  root.style.removeProperty("--bubble-bg-image");
}

export function normalizeSettings(raw: TranslateSettings): TranslateSettings {
  const r = raw as TranslateSettings & { bubbleTheme?: string };
  return {
    ...raw,
    appTheme: raw.appTheme ?? r.bubbleTheme ?? "light",
    bubbleOpacity: raw.bubbleOpacity ?? 92,
    bubbleBackground: raw.bubbleBackground ?? "",
    bubbleBgLayouts: normalizeBubbleBgLayouts(
      raw.bubbleBgLayouts ?? {
        loading: { posX: raw.bubbleBgPosX, posY: raw.bubbleBgPosY, zoom: raw.bubbleBgZoom },
        success: { posX: raw.bubbleBgPosX, posY: raw.bubbleBgPosY, zoom: raw.bubbleBgZoom },
      },
    ),
    bubbleTextColor: raw.bubbleTextColor?.trim() ?? "",
    bubbleMutedColor: raw.bubbleMutedColor?.trim() ?? "",
    historyMaxCount: raw.historyMaxCount ?? 200,
    launchAtStartup: raw.launchAtStartup ?? false,
    translateClipboardGuardSec: raw.translateClipboardGuardSec ?? 1,
    bubbleReplaceHotkey: raw.bubbleReplaceHotkey?.trim() || "Shift+Enter",
  };
}

export { pickLayoutFromSettings, normalizeBubbleBgLayouts };
