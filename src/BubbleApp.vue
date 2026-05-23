<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { TranslateSettings } from "./types";
import { normalizeBubbleBgLayouts } from "./bubbleBgLayout";
import { applyBubbleTheme, bubbleCardStyle, normalizeSettings, type BubbleAppearance } from "./theme";
import type { BubbleBgLayouts } from "./types";

interface BubblePayload {
  phase: "loading" | "success" | "error";
  sourceText?: string;
  source_text?: string;
  translatedText?: string;
  translated_text?: string;
  provider?: string;
  fromCache?: boolean;
  from_cache?: boolean;
  error?: string;
  theme?: string;
  opacity?: number;
  backgroundDataUrl?: string;
  background_data_url?: string;
  bubbleBgLayouts?: BubbleBgLayouts;
  bubble_bg_layouts?: BubbleBgLayouts;
  bubbleTextColor?: string;
  bubble_text_color?: string;
  bubbleMutedColor?: string;
  bubble_muted_color?: string;
}

const phase = ref<BubblePayload["phase"]>("loading");
const sourceText = ref("");
const translatedText = ref("");
const provider = ref("");
const fromCache = ref(false);
const errorMsg = ref("");
const bgDataUrl = ref<string | null>(null);
const appearance = ref<BubbleAppearance>({
  appTheme: "light",
  bubbleOpacity: 92,
  bubbleBgLayouts: normalizeBubbleBgLayouts(),
  bubbleTextColor: "",
  bubbleMutedColor: "",
});

const cardStyle = computed(() =>
  bubbleCardStyle(
    appearance.value,
    bgDataUrl.value,
    phase.value === "loading" ? "loading" : "success",
  ),
);

let unlisten: (() => void) | undefined;
let unlistenSettings: (() => void) | undefined;

const displaySource = computed(() => sourceText.value.trim());

/** 气泡左上角：统一显示英文引擎名 */
function formatProviderLabel(raw: string): string {
  const s = raw.trim().toLowerCase();
  if (!s) return "";
  if (s === "baidu" || s.includes("百度")) return "baidu";
  if (s === "bing" || s.includes("必应")) return "bing";
  return raw.trim();
}

const providerLabel = computed(() => formatProviderLabel(provider.value));

function pickStr(...vals: (string | undefined)[]) {
  for (const v of vals) {
    if (typeof v === "string" && v.trim()) return v;
  }
  return "";
}

function pickNum(...vals: (number | undefined)[]) {
  for (const v of vals) {
    if (typeof v === "number" && !Number.isNaN(v)) return v;
  }
  return undefined;
}

function applySettingsAppearance(s: TranslateSettings) {
  const n = normalizeSettings(s);
  appearance.value = {
    appTheme: n.appTheme === "light" ? "light" : "dark",
    bubbleOpacity: n.bubbleOpacity,
    bubbleBgLayouts: n.bubbleBgLayouts,
    bubbleTextColor: n.bubbleTextColor ?? "",
    bubbleMutedColor: n.bubbleMutedColor ?? "",
  };
}

function applyAppearance(p: BubblePayload) {
  if (p.theme === "light" || p.theme === "dark") {
    appearance.value = { ...appearance.value, appTheme: p.theme };
  }
  const opacity = pickNum(p.opacity);
  if (opacity !== undefined && opacity >= 50 && opacity <= 100) {
    appearance.value = { ...appearance.value, bubbleOpacity: opacity };
  }
  const layouts = p.bubbleBgLayouts ?? p.bubble_bg_layouts;
  if (layouts) {
    appearance.value = {
      ...appearance.value,
      bubbleBgLayouts: normalizeBubbleBgLayouts(layouts),
    };
  }
  if (p.bubbleTextColor !== undefined || p.bubble_text_color !== undefined) {
    appearance.value = {
      ...appearance.value,
      bubbleTextColor: pickStr(p.bubbleTextColor, p.bubble_text_color),
    };
  }
  if (p.bubbleMutedColor !== undefined || p.bubble_muted_color !== undefined) {
    appearance.value = {
      ...appearance.value,
      bubbleMutedColor: pickStr(p.bubbleMutedColor, p.bubble_muted_color),
    };
  }
  const url = pickStr(p.backgroundDataUrl, p.background_data_url);
  if (url) {
    bgDataUrl.value = url;
  }
  applyBubbleTheme(
    appearance.value,
    bgDataUrl.value,
    phase.value === "loading" ? "loading" : "success",
  );
}

async function refreshBackgroundFromDisk() {
  try {
    bgDataUrl.value = await invoke<string | null>("get_bubble_background_data_url");
  } catch {
    bgDataUrl.value = null;
  }
}

async function loadAppearanceFromSettings() {
  try {
    const raw = await invoke<TranslateSettings>("get_translate_settings");
    applySettingsAppearance(raw);
    bgDataUrl.value = await invoke<string | null>("get_bubble_background_data_url");
    applyBubbleTheme(appearance.value, bgDataUrl.value, phase.value === "loading" ? "loading" : "success");
  } catch {
    appearance.value = {
      appTheme: "light",
      bubbleOpacity: 92,
      bubbleBgLayouts: normalizeBubbleBgLayouts(),
      bubbleTextColor: "",
      bubbleMutedColor: "",
    };
    bgDataUrl.value = null;
    applyBubbleTheme(appearance.value);
  }
}

async function applyPayload(p: BubblePayload) {
  phase.value = p.phase;
  sourceText.value = pickStr(p.sourceText, p.source_text);
  translatedText.value = pickStr(p.translatedText, p.translated_text);
  provider.value = p.provider ?? "";
  fromCache.value = !!(p.fromCache ?? p.from_cache);
  errorMsg.value = p.error ?? "";
  applyAppearance(p);
  if (!bgDataUrl.value) {
    await refreshBackgroundFromDisk();
  }
}

function isButtonTarget(el: EventTarget | null) {
  if (!(el instanceof HTMLElement)) return false;
  return !!el.closest("button, a, .top-btns");
}

async function startWindowDrag(e: MouseEvent) {
  if (e.button !== 0 || isButtonTarget(e.target)) return;
  try {
    await getCurrentWebviewWindow().startDragging();
  } catch {
    /* ignore */
  }
}

function onCloseClick(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  void closeBubble();
}

async function closeBubble() {
  await invoke("dismiss_bubble");
}

async function copyTranslation() {
  const text = translatedText.value.trim();
  if (!text) return;
  await writeText(text);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void closeBubble();
  }
}

onMounted(async () => {
  document.documentElement.classList.add("bubble-root");
  document.body.classList.add("bubble-root");
  await loadAppearanceFromSettings();
  window.addEventListener("keydown", onKeydown);
  unlisten = await listen<BubblePayload>("bubble:update", (e) => {
    void applyPayload(e.payload);
  });
  unlistenSettings = await listen<TranslateSettings>("settings:updated", async (e) => {
    applySettingsAppearance(e.payload);
    bgDataUrl.value = await invoke<string | null>("get_bubble_background_data_url");
    applyBubbleTheme(
      appearance.value,
      bgDataUrl.value,
      phase.value === "loading" ? "loading" : "success",
    );
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  unlisten?.();
  unlistenSettings?.();
});
</script>

<template>
  <div class="bubble-shell">
    <div v-if="phase === 'loading'" class="bubble bubble-loading" :style="cardStyle">
      <div class="top-bar" @mousedown="startWindowDrag">
        <span v-if="providerLabel" class="prov">{{ providerLabel }}</span>
        <span class="spin" aria-hidden="true" />
        <span class="top-title">{{ displaySource || "翻译中…" }}</span>
        <div class="top-btns" @mousedown.stop @pointerdown.stop>
          <button
            type="button"
            class="ibtn"
            title="关闭"
            @mousedown.stop
            @pointerdown.stop
            @click.stop="onCloseClick"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                d="M6 6l12 12M18 6L6 18"
              />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <div v-else-if="phase === 'success'" class="bubble bubble-ok" :style="cardStyle">
      <div class="top-bar" @mousedown="startWindowDrag">
        <span v-if="providerLabel" class="prov">{{ providerLabel }}</span>
        <span v-if="fromCache" class="pill">缓存</span>
        <div class="top-btns" @mousedown.stop @pointerdown.stop>
          <button
            type="button"
            class="ibtn"
            title="复制译文"
            @mousedown.stop
            @pointerdown.stop
            @click.stop="copyTranslation"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
              <path
                fill="currentColor"
                d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"
              />
            </svg>
          </button>
          <button
            type="button"
            class="ibtn"
            title="关闭"
            @mousedown.stop
            @pointerdown.stop
            @click.stop="onCloseClick"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                d="M6 6l12 12M18 6L6 18"
              />
            </svg>
          </button>
        </div>
      </div>
      <div class="line-src">
        <span class="k">原</span>
        <div class="scroll-src" :title="displaySource" @mousedown.stop>
          <p class="v src">{{ displaySource || "—" }}</p>
        </div>
      </div>
      <div class="line-tr">
        <span class="k">译</span>
        <div class="scroll-tr" :title="translatedText" @mousedown.stop>
          <p class="v tr">{{ translatedText }}</p>
        </div>
      </div>
    </div>

    <div v-else class="bubble err" :style="cardStyle">
      <div class="top-bar">
        <span class="top-title err-t">翻译失败</span>
        <div class="top-btns" @mousedown.stop @pointerdown.stop>
          <button
            type="button"
            class="ibtn"
            title="关闭"
            @mousedown.stop
            @pointerdown.stop
            @click.stop="onCloseClick"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                d="M6 6l12 12M18 6L6 18"
              />
            </svg>
          </button>
        </div>
      </div>
      <p class="err-msg">{{ errorMsg }}</p>
    </div>
  </div>
</template>

<style scoped>
.bubble-shell {
  box-sizing: border-box;
  width: 100vw;
  height: 100vh;
  padding: 3px 5px 5px;
  background: transparent;
  display: flex;
  flex-direction: column;
  font-family: "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
}

.bubble {
  box-sizing: border-box;
  height: 100%;
  padding: 6px 8px 6px;
  border-radius: 8px;
  border: 1px solid var(--border);
  backdrop-filter: blur(10px);
  color: var(--bubble-text, var(--text));
  overflow: hidden;
}

.bubble.bubble-ok {
  display: grid;
  grid-template-rows: auto 20px minmax(52px, 1fr);
  gap: 4px;
  min-height: 0;
  isolation: isolate;
}

.top-bar {
  position: relative;
  z-index: 4;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 5px;
  min-height: 20px;
  cursor: grab;
  user-select: none;
}

.top-bar:active {
  cursor: grabbing;
}

.top-title {
  flex: 1;
  min-width: 0;
  font-size: 0.75rem;
  color: var(--bubble-muted, var(--muted));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.prov {
  flex: 1;
  min-width: 0;
  font-size: 0.68rem;
  color: var(--accent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pill {
  flex-shrink: 0;
  padding: 0 4px;
  border-radius: 3px;
  font-size: 0.62rem;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}

.top-btns {
  position: relative;
  z-index: 5;
  flex-shrink: 0;
  display: flex;
  gap: 2px;
  margin-left: auto;
}

.ibtn {
  position: relative;
  z-index: 6;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--bubble-muted, var(--muted));
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  pointer-events: auto;
}

.ibtn svg {
  pointer-events: none;
}

.ibtn:hover {
  background: var(--btn-ghost-hover);
  color: var(--bubble-text, var(--text));
}

.line-src,
.line-tr {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 14px 1fr;
  gap: 5px;
  align-items: start;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.line-tr {
  min-height: 0;
}

.k {
  font-size: 0.65rem;
  line-height: 20px;
  color: var(--bubble-muted, var(--muted));
  font-weight: 600;
}

.scroll-src {
  height: 20px;
  max-height: 20px;
  min-width: 0;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--muted) 50%, transparent) transparent;
}

.scroll-src::-webkit-scrollbar {
  width: 5px;
}

.scroll-src::-webkit-scrollbar-thumb {
  background: color-mix(in srgb, var(--muted) 45%, transparent);
  border-radius: 3px;
}

.scroll-tr {
  max-height: 100%;
  min-height: 48px;
  min-width: 0;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--muted) 50%, transparent) transparent;
}

.scroll-tr::-webkit-scrollbar {
  width: 6px;
}

.scroll-tr::-webkit-scrollbar-thumb {
  background: color-mix(in srgb, var(--muted) 50%, transparent);
  border-radius: 3px;
}

.v {
  margin: 0;
  padding: 0;
}

.v.src {
  font-size: 0.74rem;
  line-height: 20px;
  color: var(--bubble-muted, var(--muted));
  word-break: break-all;
}

.v.tr {
  font-size: 0.82rem;
  line-height: 1.4;
  color: var(--bubble-text, var(--text));
  word-break: break-word;
  padding-bottom: 2px;
}

.bubble-loading .top-bar {
  gap: 6px;
}

.bubble-loading .prov {
  flex: 0 0 auto;
  max-width: 4.5em;
}

.spin {
  flex-shrink: 0;
  width: 12px;
  height: 12px;
  border: 2px solid color-mix(in srgb, var(--accent) 25%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

.err-t {
  color: var(--err);
  font-weight: 600;
}

.err-msg {
  margin: 0;
  font-size: 0.74rem;
  line-height: 1.35;
  overflow-y: auto;
  flex: 1;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
