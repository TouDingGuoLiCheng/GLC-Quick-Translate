<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";

defineProps<{
  submitting: boolean;
}>();

const emit = defineEmits<{
  submit: [text: string];
  close: [];
}>();

const text = ref("");
const inputRef = ref<HTMLTextAreaElement | null>(null);

onMounted(() => {
  void nextTick(() => inputRef.value?.focus());
});

function onSubmit() {
  const t = text.value.trim();
  if (!t) return;
  emit("submit", t);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
    return;
  }
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    onSubmit();
  }
}
</script>

<template>
  <div class="manual-overlay" @click.self="emit('close')" @keydown="onKeydown">
    <section class="manual-panel" @click.stop>
      <header class="manual-head">
        <h2>手动输入</h2>
        <button type="button" class="icon-btn" aria-label="关闭" @click="emit('close')">×</button>
      </header>
      <div class="manual-body">
        <textarea
          ref="inputRef"
          v-model="text"
          class="manual-textarea"
          rows="6"
          placeholder="粘贴或输入要翻译的原文…"
          :disabled="submitting"
          @keydown="onKeydown"
        />
        <p class="manual-hint">Ctrl+Enter 翻译</p>
      </div>
      <footer class="manual-foot">
        <button type="button" class="btn ghost" :disabled="submitting" @click="emit('close')">
          取消
        </button>
        <button
          type="button"
          class="btn"
          :disabled="submitting || !text.trim()"
          @click="onSubmit"
        >
          {{ submitting ? "翻译中…" : "翻译" }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.manual-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: var(--overlay-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.manual-panel {
  width: min(100%, 280px);
  background: var(--settings-panel-bg);
  border: 1px solid var(--border);
  border-radius: 10px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 8px 28px color-mix(in srgb, #000 28%, transparent);
}

.manual-head {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}

.manual-head h2 {
  margin: 0;
  font-size: 0.92rem;
  color: var(--accent);
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
  font-size: 1.2rem;
  line-height: 1;
}

.icon-btn:hover {
  background: var(--btn-ghost-hover);
  color: var(--text);
}

.manual-body {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.manual-textarea {
  width: 100%;
  box-sizing: border-box;
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--settings-input-bg);
  color: var(--text);
  font: inherit;
  font-size: 0.86rem;
  line-height: 1.45;
  resize: vertical;
  min-height: 110px;
  outline: none;
}

.manual-textarea:focus {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.manual-textarea:disabled {
  opacity: 0.7;
}

.manual-textarea::placeholder {
  color: var(--muted);
}

.manual-hint {
  margin: 0;
  font-size: 0.72rem;
  color: var(--muted);
}

.manual-foot {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 12px 12px;
  border-top: 1px solid var(--border);
}

.btn {
  padding: 6px 14px;
  border: none;
  border-radius: 7px;
  background: var(--accent);
  color: #fff;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
}

.btn:hover:not(:disabled) {
  filter: brightness(1.06);
}

.btn:disabled {
  opacity: 0.55;
  cursor: default;
}

.btn.ghost {
  background: transparent;
  color: var(--muted);
  border: 1px solid var(--border);
}

.btn.ghost:hover:not(:disabled) {
  color: var(--text);
  background: var(--btn-ghost-hover);
  filter: none;
}
</style>
