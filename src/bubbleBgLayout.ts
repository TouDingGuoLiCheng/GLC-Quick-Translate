import type { BubbleBgLayout, BubbleBgLayouts, TranslateSettings } from "./types";

export type BubbleBgVariant = "loading" | "success";

export const DEFAULT_BG_LAYOUT: BubbleBgLayout = { posX: 50, posY: 50, zoom: 100 };

export function defaultBubbleBgLayouts(): BubbleBgLayouts {
  return {
    loading: { ...DEFAULT_BG_LAYOUT },
    success: { ...DEFAULT_BG_LAYOUT },
  };
}

export function clampBgPos(v: number) {
  return Math.min(100, Math.max(0, Math.round(v)));
}

export function clampBgZoom(v: number) {
  return Math.min(250, Math.max(50, Math.round(v)));
}

export function normalizeBgLayout(raw?: Partial<BubbleBgLayout>): BubbleBgLayout {
  return {
    posX: clampBgPos(raw?.posX ?? DEFAULT_BG_LAYOUT.posX),
    posY: clampBgPos(raw?.posY ?? DEFAULT_BG_LAYOUT.posY),
    zoom: clampBgZoom(raw?.zoom ?? DEFAULT_BG_LAYOUT.zoom),
  };
}

export function normalizeBubbleBgLayouts(
  raw?: Partial<{ loading?: Partial<BubbleBgLayout>; success?: Partial<BubbleBgLayout> }>,
): BubbleBgLayouts {
  const legacy = raw as BubbleBgLayouts & {
    bubbleBgPosX?: number;
    bubbleBgPosY?: number;
    bubbleBgZoom?: number;
  };
  if (!raw?.loading && !raw?.success && legacy?.bubbleBgPosX !== undefined) {
    const l = normalizeBgLayout({
      posX: legacy.bubbleBgPosX,
      posY: legacy.bubbleBgPosY,
      zoom: legacy.bubbleBgZoom,
    });
    return { loading: l, success: { ...l } };
  }
  return {
    loading: normalizeBgLayout(raw?.loading),
    success: normalizeBgLayout(raw?.success),
  };
}

export function layoutForPhase(phase: string, layouts: BubbleBgLayouts): BubbleBgLayout {
  return phase === "loading" ? layouts.loading : layouts.success;
}

/** CSS background（气泡与预览展示） */
export function layoutToBackgroundStyle(layout: BubbleBgLayout, dataUrl: string): Record<string, string> {
  const x = layout.posX;
  const y = layout.posY;
  const zoom = layout.zoom;
  const size = zoom === 100 ? "cover" : `${zoom}%`;
  return {
    backgroundImage: `url("${dataUrl}")`,
    backgroundPosition: `${x}% ${y}%`,
    backgroundSize: size,
    backgroundRepeat: "no-repeat",
  };
}

/** 拖动像素 → 焦点百分比变化 */
export function panLayoutByPixels(
  layout: BubbleBgLayout,
  dx: number,
  dy: number,
  boxW: number,
  boxH: number,
): BubbleBgLayout {
  if (boxW <= 0 || boxH <= 0) return layout;
  // 按容器比例平移，缩放越大拖动越细腻
  const scale = Math.max(layout.zoom, 80) / 100;
  const factor = 100 / scale;
  return {
    ...layout,
    posX: clampBgPos(layout.posX - (dx / boxW) * factor),
    posY: clampBgPos(layout.posY - (dy / boxH) * factor),
  };
}

export function zoomLayoutByWheel(layout: BubbleBgLayout, deltaY: number): BubbleBgLayout {
  const step = deltaY > 0 ? -4 : 4;
  return { ...layout, zoom: clampBgZoom(layout.zoom + step) };
}

export function pickLayoutFromSettings(
  settings: Pick<TranslateSettings, "bubbleBgLayouts" | "bubbleBgPosX" | "bubbleBgPosY" | "bubbleBgZoom">,
  variant: BubbleBgVariant,
): BubbleBgLayout {
  const legacy = {
    posX: settings.bubbleBgPosX,
    posY: settings.bubbleBgPosY,
    zoom: settings.bubbleBgZoom,
  };
  const layouts = normalizeBubbleBgLayouts(
    settings.bubbleBgLayouts ?? { loading: legacy, success: legacy },
  );
  return variant === "loading" ? layouts.loading : layouts.success;
}
