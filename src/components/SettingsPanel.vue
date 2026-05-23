<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BubblePreview from "./BubblePreview.vue";
import { defaultBubbleBgLayouts, normalizeBubbleBgLayouts } from "../bubbleBgLayout";
import { themeDefaultTextColors } from "../theme";
import type { BubbleBgLayouts, TranslateSettings, TranslatorsFile } from "../types";

const props = defineProps<{
  settings: TranslateSettings;
  providers: TranslatorsFile["providers"];
  saving: boolean;
}>();

const emit = defineEmits<{
  "update:settings": [TranslateSettings];
  save: [];
  close: [];
}>();

const local = computed({
  get: () => props.settings,
  set: (v) => emit("update:settings", v),
});

type SectionKey = "general" | "translate" | "appearance" | "advanced";

const sectionOpen = ref<Record<SectionKey, boolean>>({
  general: false,
  translate: false,
  appearance: false,
  advanced: false,
});

const targetLangOptions = [
  { value: "auto", label: "自动（非中文→中文，中文→英文）" },
  { value: "zh-Hans", label: "简体中文" },
  { value: "zh-Hant", label: "繁体中文" },
  { value: "en", label: "英语" },
  { value: "ja", label: "日语" },
];

const providerOptions = computed(() =>
  props.providers.filter((p) => p.enabled).map((p) => ({ value: p.id, label: p.name })),
);

const bgLocalPath = ref<string | null>(null);
const bgMsg = ref("");
const logOpenMsg = ref("");
const previewHover = ref(false);
const settingsBodyRef = ref<HTMLElement | null>(null);
const settingsPanelRef = ref<HTMLElement | null>(null);

/** 鼠标在预览上时，禁止设置区滚轮滚动/滑块误触 */
function onSettingsWheelCapture(e: WheelEvent) {
  if (!previewHover.value) return;
  e.preventDefault();
}

function onPreviewHoverChange(inside: boolean) {
  previewHover.value = inside;
}

onMounted(() => {
  const opts: AddEventListenerOptions = { passive: false, capture: true };
  settingsBodyRef.value?.addEventListener("wheel", onSettingsWheelCapture, opts);
  settingsPanelRef.value?.addEventListener("wheel", onSettingsWheelCapture, opts);
});

onBeforeUnmount(() => {
  const opts: AddEventListenerOptions = { capture: true };
  settingsBodyRef.value?.removeEventListener("wheel", onSettingsWheelCapture, opts);
  settingsPanelRef.value?.removeEventListener("wheel", onSettingsWheelCapture, opts);
});

const appearanceThemeClass = computed(() =>
  local.value.appTheme === "light" ? "theme-light" : "theme-dark",
);

const hasBackground = computed(
  () => !!(local.value.bubbleBackground?.trim() || bgLocalPath.value),
);

function onPreviewLayouts(layouts: BubbleBgLayouts) {
  emit("update:settings", { ...props.settings, bubbleBgLayouts: layouts });
}

const previewLayouts = computed({
  get: () => normalizeBubbleBgLayouts(local.value.bubbleBgLayouts),
  set: (layouts) => onPreviewLayouts(layouts),
});

const themeTextDefaults = computed(() => themeDefaultTextColors(local.value.appTheme));

const textColorPicker = computed({
  get: () => local.value.bubbleTextColor?.trim() || themeTextDefaults.value.text,
  set: (v: string) => {
    local.value = { ...local.value, bubbleTextColor: v };
  },
});

const mutedColorPicker = computed({
  get: () => local.value.bubbleMutedColor?.trim() || themeTextDefaults.value.muted,
  set: (v: string) => {
    local.value = { ...local.value, bubbleMutedColor: v };
  },
});

function resetTextColor() {
  local.value = { ...local.value, bubbleTextColor: "" };
}

function resetMutedColor() {
  local.value = { ...local.value, bubbleMutedColor: "" };
}

function toggleSection(key: SectionKey) {
  sectionOpen.value[key] = !sectionOpen.value[key];
}

async function persistSettings(next: TranslateSettings) {
  emit("update:settings", next);
  await invoke("save_translate_settings", { settings: next });
}

async function pickBubbleBackground() {
  bgMsg.value = "";
  try {
    const picked = await invoke<{ filename: string; fullPath: string }>("pick_bubble_background");
    bgLocalPath.value = picked.fullPath;
    const next = {
      ...props.settings,
      bubbleBackground: picked.filename,
      bubbleBgLayouts: defaultBubbleBgLayouts(),
    };
    await persistSettings(next);
    bgMsg.value = "已应用";
  } catch (e) {
    bgMsg.value = String(e);
  }
}

async function clearBubbleBackground() {
  bgMsg.value = "";
  try {
    await invoke("clear_bubble_background");
    bgLocalPath.value = null;
    const next = {
      ...props.settings,
      bubbleBackground: "",
      bubbleBgLayouts: defaultBubbleBgLayouts(),
    };
    await persistSettings(next);
    bgMsg.value = "已恢复默认";
  } catch (e) {
    bgMsg.value = String(e);
  }
}

async function openLogsFolder() {
  logOpenMsg.value = "";
  try {
    await invoke<string>("open_python_capture_logs");
    logOpenMsg.value = "已打开";
  } catch (e) {
    logOpenMsg.value = String(e);
  }
}
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <section ref="settingsPanelRef" class="settings-panel" @click.stop>
      <header class="settings-head">
        <h2>设置</h2>
        <button type="button" class="icon-btn" aria-label="关闭" @click="emit('close')">×</button>
      </header>

      <div
        ref="settingsBodyRef"
        class="settings-body"
        :class="{ 'settings-scroll-locked': previewHover }"
      >
        <section class="settings-section">
          <button type="button" class="section-toggle" @click="toggleSection('general')">
            <span>常规</span>
            <span class="chevron" :class="{ open: sectionOpen.general }">›</span>
          </button>
          <div v-show="sectionOpen.general" class="section-body">
            <label class="row">
              <input v-model="local.enabled" type="checkbox" />
              <span>启用全局热键</span>
            </label>
            <label class="row">
              <input v-model="local.launchAtStartup" type="checkbox" />
              <span>开机自动启动</span>
            </label>
            <label class="row">
              <span class="label">历史上限</span>
              <input
                v-model.number="local.historyMaxCount"
                class="input input-narrow"
                type="number"
                min="20"
                max="500"
              />
            </label>
          </div>
        </section>

        <section class="settings-section">
          <button type="button" class="section-toggle" @click="toggleSection('translate')">
            <span>热键与翻译</span>
            <span class="chevron" :class="{ open: sectionOpen.translate }">›</span>
          </button>
          <div v-show="sectionOpen.translate" class="section-body">
            <label class="row">
              <span class="label">翻译</span>
              <input v-model="local.hotkey" class="input" type="text" placeholder="Ctrl+T" />
            </label>
            <label class="row">
              <span class="label">替换</span>
              <input
                v-model="local.replaceHotkey"
                class="input"
                type="text"
                placeholder="Ctrl+Shift+T"
              />
            </label>
            <label class="row">
              <span class="label">目标语言</span>
              <select v-model="local.targetLang" class="input">
                <option v-for="opt in targetLangOptions" :key="opt.value" :value="opt.value">
                  {{ opt.label }}
                </option>
              </select>
            </label>
            <label class="row">
              <span class="label">翻译源</span>
              <select v-model="local.primaryProvider" class="input">
                <option v-for="opt in providerOptions" :key="opt.value" :value="opt.value">
                  {{ opt.label }}
                </option>
              </select>
            </label>
            <label class="row">
              <input v-model="local.fallbackEnabled" type="checkbox" />
              <span>主源失败时尝试其它源</span>
            </label>
            <label class="row">
              <input v-model="local.usePythonCapture" type="checkbox" />
              <span>用 Python 取词（推荐）</span>
            </label>
            <label class="row">
              <input v-model="local.restoreClipboard" type="checkbox" />
              <span>取词后恢复剪贴板</span>
            </label>
            <label class="row">
              <span class="label">复制延迟 (ms)</span>
              <input
                v-model.number="local.copyDelayMs"
                class="input input-narrow"
                type="number"
                min="80"
                max="800"
              />
            </label>
            <label class="row">
              <span class="label">超时 (秒)</span>
              <input
                v-model.number="local.timeoutSec"
                class="input input-narrow"
                type="number"
                min="3"
                max="60"
              />
            </label>
            <label class="row">
              <span class="label">气泡关闭 (秒)</span>
              <input
                v-model.number="local.bubbleAutoCloseSec"
                class="input input-narrow"
                type="number"
                min="3"
                max="60"
              />
            </label>
          </div>
        </section>

        <section class="settings-section">
          <button type="button" class="section-toggle" @click="toggleSection('appearance')">
            <span>外观</span>
            <span class="chevron" :class="{ open: sectionOpen.appearance }">›</span>
          </button>
          <div v-show="sectionOpen.appearance" class="section-body" :class="appearanceThemeClass">
            <BubblePreview
              :settings="{
                appTheme: local.appTheme,
                bubbleOpacity: local.bubbleOpacity,
                bubbleBackground: local.bubbleBackground,
                bubbleTextColor: local.bubbleTextColor,
                bubbleMutedColor: local.bubbleMutedColor,
              }"
              v-model:layouts="previewLayouts"
              :local-image-path="bgLocalPath"
              @hover-change="onPreviewHoverChange"
            />
            <label class="row">
              <span class="label">应用主题</span>
              <select v-model="local.appTheme" class="input">
                <option value="dark">深色</option>
                <option value="light">白色</option>
              </select>
            </label>
            <label class="row row-slider" :class="{ disabled: hasBackground }">
              <span class="label">气泡透明</span>
              <input
                v-model.number="local.bubbleOpacity"
                class="slider"
                type="range"
                min="50"
                max="100"
                step="1"
                :disabled="hasBackground"
              />
              <span class="slider-val">{{ local.bubbleOpacity }}%</span>
            </label>
            <p v-if="hasBackground" class="hint-msg">已设背景图，透明选项不可用</p>
            <label class="row row-color">
              <span class="label">主文字色</span>
              <input v-model="textColorPicker" class="color-input" type="color" />
              <button
                type="button"
                class="btn btn-secondary btn-mini"
                :disabled="!local.bubbleTextColor"
                @click="resetTextColor"
              >
                默认
              </button>
            </label>
            <label class="row row-color">
              <span class="label">次要文字</span>
              <input v-model="mutedColorPicker" class="color-input" type="color" />
              <button
                type="button"
                class="btn btn-secondary btn-mini"
                :disabled="!local.bubbleMutedColor"
                @click="resetMutedColor"
              >
                默认
              </button>
            </label>
            <div class="row col-block">
              <span class="label">气泡背景图</span>
              <div class="bg-actions">
                <button type="button" class="btn btn-secondary" @click="pickBubbleBackground">
                  选择图片
                </button>
                <button
                  type="button"
                  class="btn btn-secondary"
                  :disabled="!local.bubbleBackground && !bgLocalPath"
                  @click="clearBubbleBackground"
                >
                  恢复默认
                </button>
              </div>
              <p v-if="bgMsg" class="hint-msg">{{ bgMsg }}</p>
            </div>
          </div>
        </section>

        <section class="settings-section">
          <button type="button" class="section-toggle" @click="toggleSection('advanced')">
            <span>高级</span>
            <span class="chevron" :class="{ open: sectionOpen.advanced }">›</span>
          </button>
          <div v-show="sectionOpen.advanced" class="section-body">
            <button type="button" class="btn btn-secondary block-btn" @click="openLogsFolder">
              打开日志文件夹
            </button>
            <p v-if="logOpenMsg" class="hint-msg">{{ logOpenMsg }}</p>
          </div>
        </section>
      </div>

      <footer class="settings-foot">
        <button class="btn" type="button" :disabled="saving" @click="emit('save')">保存</button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.settings-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: var(--overlay-bg);
  display: flex;
  justify-content: flex-end;
  align-items: stretch;
}

.settings-panel {
  width: min(100%, 300px);
  height: 100%;
  max-height: 100dvh;
  background: var(--settings-panel-bg);
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.settings-head {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
}

.settings-head h2 {
  margin: 0;
  font-size: 0.95rem;
  color: var(--accent);
}

.settings-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 8px 14px 16px;
  -webkit-overflow-scrolling: touch;
}

.settings-body.settings-scroll-locked {
  overflow: hidden;
}

.settings-section {
  margin-bottom: 6px;
}

.section-toggle {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 4px;
  border: none;
  border-bottom: 1px solid var(--border);
  background: transparent;
  color: var(--accent);
  font-size: 0.82rem;
  font-weight: 700;
  cursor: pointer;
  text-align: left;
}

.section-toggle:hover {
  color: var(--text);
}

.chevron {
  display: inline-block;
  font-size: 1rem;
  line-height: 1;
  color: var(--muted);
  transition: transform 0.15s ease;
  transform: rotate(0deg);
}

.chevron.open {
  transform: rotate(90deg);
}

.section-body {
  padding: 10px 2px 4px;
}

.settings-foot {
  flex-shrink: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 10px 14px;
  border-top: 1px solid var(--border);
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  font-size: 0.82rem;
}

.label {
  min-width: 72px;
  color: var(--muted);
  flex-shrink: 0;
}

.input {
  flex: 1;
  padding: 7px 9px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--settings-input-bg);
  color: var(--text);
  min-width: 0;
  font-size: 0.82rem;
}

.input-narrow {
  max-width: 80px;
}

.row-slider {
  flex-wrap: wrap;
}

.row-slider .slider {
  flex: 1;
  min-width: 100px;
  accent-color: var(--accent);
}

.slider-val {
  min-width: 40px;
  text-align: right;
  color: var(--muted);
  font-size: 0.78rem;
}

.row-slider.disabled {
  opacity: 0.45;
  pointer-events: none;
}

.row-color .color-input {
  width: 36px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}

.btn-mini {
  padding: 4px 8px;
  font-size: 0.72rem;
}

.btn {
  padding: 7px 12px;
  border-radius: 8px;
  border: none;
  background: var(--accent);
  color: #0f1419;
  font-weight: 600;
  font-size: 0.82rem;
  cursor: pointer;
}

.btn-secondary {
  background: transparent;
  color: var(--text);
  border: 1px solid var(--border);
}

.block-btn {
  width: 100%;
}

.col-block {
  flex-direction: column;
  align-items: stretch;
}

.hint-msg {
  margin: 4px 0 0;
  font-size: 0.72rem;
  color: var(--muted);
}

.bg-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 6px 0 4px;
}

.icon-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  font-size: 1.35rem;
  line-height: 1;
  cursor: pointer;
}

.icon-btn:hover {
  background: var(--btn-ghost-hover);
  color: var(--text);
}
</style>
