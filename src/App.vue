<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { applyAppTheme, normalizeSettings } from "./theme";
import SettingsPanel from "./components/SettingsPanel.vue";
import ManualInputPanel from "./components/ManualInputPanel.vue";
import type {
  HistoryRecord,
  SelectionResult,
  TranslateDonePayload,
  TranslateResult,
  TranslateSettings,
  TranslatorsFile,
} from "./types";

const defaultSettings = (): TranslateSettings => ({
  enabled: true,
  hotkey: "Ctrl+T",
  replaceHotkey: "Ctrl+Shift+T",
  bubbleReplaceHotkey: "Shift+Enter",
  restoreClipboard: true,
  copyDelayMs: 200,
  translateClipboardGuardSec: 1,
  targetLang: "auto",
  primaryProvider: "baidu",
  fallbackEnabled: true,
  timeoutSec: 20,
  cacheTtlSec: 30,
  bubbleAutoCloseSec: 12,
  usePythonCapture: true,
  appTheme: "light",
  bubbleOpacity: 92,
  bubbleBackground: "",
  bubbleBgLayouts: {
    loading: { posX: 50, posY: 50, zoom: 100 },
    success: { posX: 50, posY: 50, zoom: 100 },
  },
  bubbleTextColor: "",
  bubbleMutedColor: "",
  historyMaxCount: 200,
  launchAtStartup: false,
});

const settings = ref<TranslateSettings>(defaultSettings());
const providers = ref<TranslatorsFile["providers"]>([]);
const history = ref<HistoryRecord[]>([]);
const lastSelection = ref<SelectionResult | null>(null);
const lastTranslate = ref<TranslateResult | null>(null);
const translating = ref(false);
const showSettings = ref(false);
const showManualInput = ref(false);
const saving = ref(false);

const hotkeyHint = computed(() => settings.value.hotkey);

let unlistenDone: (() => void) | undefined;
let unlistenStarted: (() => void) | undefined;
let unlistenHistory: (() => void) | undefined;

function formatTime(ms: number) {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function truncate(text: string, max = 120) {
  const chars = [...text];
  if (chars.length <= max) return text;
  return `${chars.slice(0, max).join("")}…`;
}

function onTranslateDone(payload: TranslateDonePayload) {
  translating.value = false;
  lastSelection.value = payload.selection;
  lastTranslate.value = payload.translate;
}

async function loadSettings() {
  settings.value = normalizeSettings({
    ...defaultSettings(),
    ...(await invoke<TranslateSettings>("get_translate_settings")),
  });
  applyAppTheme(settings.value);
}

async function loadProviders() {
  const file = await invoke<TranslatorsFile>("list_translators");
  providers.value = file.providers ?? [];
}

async function loadHistory() {
  history.value = await invoke<HistoryRecord[]>("list_history");
}

async function saveSettings() {
  saving.value = true;
  try {
    await invoke("save_translate_settings", { settings: settings.value });
    applyAppTheme(settings.value);
    showSettings.value = false;
  } finally {
    saving.value = false;
  }
}

async function copyText(text: string) {
  const t = text.trim();
  if (!t) return;
  await writeText(t);
}

async function deleteRecord(id: string) {
  await invoke("delete_history_record", { id });
}

async function retranslateItem(item: HistoryRecord) {
  const text = item.sourceText.trim();
  if (!text) return;
  translating.value = true;
  try {
    await invoke<TranslateResult>("translate_text", { text });
    await loadHistory();
  } finally {
    translating.value = false;
  }
}

async function submitManualTranslate(text: string) {
  const t = text.trim();
  if (!t) return;
  translating.value = true;
  try {
    await invoke<TranslateResult>("translate_text", { text: t });
    showManualInput.value = false;
    await loadHistory();
  } finally {
    translating.value = false;
  }
}

async function clearHistory() {
  await invoke("clear_history");
  history.value = [];
}

async function startDrag(e: MouseEvent) {
  if (e.button !== 0) return;
  const t = e.target as HTMLElement;
  if (t.closest("button, a, input, select, textarea, .history-acts, .settings-overlay, .manual-overlay")) return;
  try {
    await getCurrentWindow().startDragging();
  } catch {
    /* ignore */
  }
}

async function hideWindow() {
  await invoke("collapse_main_to_tray");
}

watch(
  () => settings.value.appTheme,
  () => applyAppTheme(settings.value),
);

onMounted(async () => {
  await Promise.all([loadSettings(), loadProviders(), loadHistory()]);
  unlistenStarted = await listen("translate:started", () => {
    translating.value = true;
  });
  unlistenDone = await listen<TranslateDonePayload>("translate:done", (e) => {
    onTranslateDone(e.payload);
  });
  unlistenHistory = await listen<HistoryRecord[]>("history:updated", (e) => {
    history.value = e.payload;
  });
  await listen<TranslateSettings>("settings:updated", (e) => {
    settings.value = normalizeSettings({ ...settings.value, ...e.payload });
    applyAppTheme(settings.value);
  });
});

onUnmounted(() => {
  unlistenDone?.();
  unlistenStarted?.();
  unlistenHistory?.();
});
</script>

<template>
  <div class="app-shell">
    <header class="titlebar" @mousedown="startDrag">
      <div class="brand">
        <span class="brand-name" title="GLC Quick Translate">GLC Quick Translate</span>
      </div>
      <div class="titlebar-actions">
        <span v-if="translating" class="status-pill">翻译中…</span>
        <button
          type="button"
          class="icon-btn"
          title="手动输入"
          aria-label="手动输入"
          @click.stop="showManualInput = true"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path
              fill="currentColor"
              d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
            />
          </svg>
        </button>
        <button
          type="button"
          class="icon-btn gear-btn"
          title="设置"
          aria-label="设置"
          @click.stop="showSettings = true"
        >
          <svg class="gear-icon" viewBox="0 0 24 24" width="17" height="17" aria-hidden="true">
            <path
              fill="currentColor"
              d="M12 8.4a3.6 3.6 0 1 1 0 7.2 3.6 3.6 0 0 1 0-7.2zm9.2 4.8c.1-.4.1-.8.1-1.2s0-.8-.1-1.2l2-1.5a.6.6 0 0 0 .1-.8l-1.9-3.3a.6.6 0 0 0-.7-.3l-2.4 1c-.5-.4-1.1-.7-1.7-1l-.4-2.5a.6.6 0 0 0-.6-.5h-3.8a.6.6 0 0 0-.6.5l-.4 2.5c-.6.3-1.2.6-1.7 1l-2.4-1a.6.6 0 0 0-.7.3l-1.9 3.3a.6.6 0 0 0 .1.8l2 1.5c-.1.4-.1.8-.1 1.2s0 .8.1 1.2l-2 1.5a.6.6 0 0 0-.1.8l1.9 3.3c.2.3.5.4.7.3l2.4-1c.5.4 1.1.7 1.7 1l.4 2.5c.1.3.3.5.6.5h3.8c.3 0 .5-.2.6-.5l.4-2.5c.6-.3 1.2-.6 1.7-1l2.4 1c.3.1.6 0 .7-.3l1.9-3.3a.6.6 0 0 0-.1-.8l-2-1.5z"
            />
          </svg>
        </button>
        <button type="button" class="icon-btn" title="隐藏到托盘" @click.stop="hideWindow">×</button>
      </div>
    </header>

    <main class="main">
      <div class="main-toolbar">
        <h2 class="section-title">翻译记录</h2>
        <button
          v-if="history.length"
          type="button"
          class="text-btn"
          @click="clearHistory"
        >
          清空
        </button>
      </div>

      <p class="hint">
        <code>{{ hotkeyHint }}</code> 翻译气泡 ·
        <code>{{ settings.replaceHotkey }}</code> 替换选中内容
      </p>

      <ul v-if="history.length" class="history-list">
        <li
          v-for="item in history"
          :key="item.id"
          class="history-item"
          :class="{ fail: !item.ok }"
        >
          <div class="history-meta">
            <time>{{ formatTime(item.createdAt) }}</time>
            <span v-if="item.provider" class="provider">{{ item.provider }}</span>
            <span v-if="item.fromCache" class="tag">缓存</span>
            <span v-if="!item.ok" class="tag err">失败</span>
            <div class="history-acts">
              <button
                type="button"
                class="icon-act"
                title="复制原文"
                @click="copyText(item.sourceText)"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"
                  />
                </svg>
              </button>
              <button
                v-if="item.ok && item.translatedText"
                type="button"
                class="icon-act"
                title="复制译文"
                @click="copyText(item.translatedText!)"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M18 2H9c-1.1 0-2 .9-2 2v1H5c-1.1 0-2 .9-2 2v15c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V8l-6-6zm-1 7H10V4h7v5zM5 8h2v11h9V8H5z"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="icon-act"
                title="重新翻译"
                @click="retranslateItem(item)"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M17.65 6.35A7.958 7.958 0 0 0 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08a5.99 5.99 0 0 1-5.65 4c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="icon-act danger"
                title="删除"
                @click="deleteRecord(item.id)"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                  />
                </svg>
              </button>
            </div>
          </div>
          <p class="history-src" :title="item.sourceText">{{ truncate(item.sourceText) }}</p>
          <p v-if="item.ok && item.translatedText" class="history-tr">
            {{ item.translatedText }}
          </p>
          <p v-else-if="item.error" class="history-err">{{ item.error }}</p>
        </li>
      </ul>

      <div v-else class="empty">
        <p>暂无记录</p>
        <p class="muted">在任意应用中选中文字，使用快捷键即可翻译</p>
      </div>
    </main>

    <Teleport to="body">
      <ManualInputPanel
        v-if="showManualInput"
        :submitting="translating"
        @submit="submitManualTranslate"
        @close="showManualInput = false"
      />
      <SettingsPanel
        v-if="showSettings"
        :settings="settings"
        :providers="providers"
        :saving="saving"
        @update:settings="settings = $event"
        @save="saveSettings"
        @close="showSettings = false"
      />
    </Teleport>
  </div>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--shell-bg);
  overflow: hidden;
}

.titlebar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px 8px 14px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  backdrop-filter: blur(8px);
  user-select: none;
  cursor: default;
}

.brand-name {
  font-size: 0.72rem;
  font-weight: 800;
  letter-spacing: 0.04em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 168px;
  background: linear-gradient(120deg, #22d3ee 0%, #a5f3fc 50%, #c4b5fd 100%);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.gear-btn:hover .gear-icon {
  transform: rotate(45deg);
}

.gear-icon {
  display: block;
  transition: transform 0.2s ease;
}

.titlebar-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.status-pill {
  font-size: 0.75rem;
  color: var(--accent);
  margin-right: 6px;
}

.icon-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.icon-btn:hover {
  background: var(--btn-ghost-hover);
  color: var(--text);
}

.main {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 8px 10px 12px;
  background: var(--shell-bg);
}

.main-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.section-title {
  margin: 0;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--text);
}

.text-btn {
  border: none;
  background: none;
  color: var(--muted);
  font-size: 0.82rem;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
}

.text-btn:hover {
  color: var(--err);
  background: var(--err-bg);
}

.hint {
  margin: 0 0 10px;
  padding: 6px 8px;
  font-size: 0.8rem;
  color: var(--muted);
  background: var(--surface);
  border-radius: 6px;
  border: 1px solid var(--border);
  backdrop-filter: blur(6px);
}

.hint code {
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--hint-code-bg);
  color: var(--accent);
  font-size: 0.9em;
}

.history-list {
  list-style: none;
  margin: 0;
  padding: 0;
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.history-item {
  padding: 6px 8px 7px;
  border-radius: 8px;
  background: var(--surface);
  border: 1px solid var(--border);
  backdrop-filter: blur(8px);
}

.history-item.fail {
  border-color: color-mix(in srgb, var(--err) 35%, transparent);
}

.history-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
  margin-bottom: 4px;
  font-size: 0.72rem;
  color: var(--muted);
}

.history-acts {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: auto;
  flex-shrink: 0;
}

.icon-act {
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.icon-act:hover {
  color: var(--text);
  background: var(--hint-code-bg);
}

.icon-act.danger:hover {
  color: var(--err);
  background: var(--err-bg);
}

.provider {
  color: var(--accent);
}

.tag {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--hint-code-bg);
  color: var(--accent);
}

.tag.err {
  background: var(--err-bg);
  color: var(--err);
}

.history-src {
  margin: 0 0 4px;
  font-size: 0.82rem;
  color: var(--muted);
  line-height: 1.4;
  word-break: break-word;
}

.history-tr {
  margin: 0;
  font-size: 0.92rem;
  line-height: 1.5;
  color: var(--text);
  word-break: break-word;
}

.history-err {
  margin: 0;
  font-size: 0.85rem;
  color: var(--err);
  line-height: 1.4;
}

.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: var(--muted);
  font-size: 0.9rem;
}

.empty .muted {
  margin-top: 6px;
  font-size: 0.82rem;
  opacity: 0.8;
}
</style>
