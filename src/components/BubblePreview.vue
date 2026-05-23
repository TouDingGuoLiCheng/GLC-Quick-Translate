<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  normalizeBgLayout,
  panLayoutByPixels,
  zoomLayoutByWheel,
  type BubbleBgVariant,
} from "../bubbleBgLayout";
import { bubbleCardStyle } from "../theme";
import type { BubbleBgLayout, BubbleBgLayouts, TranslateSettings } from "../types";

const props = defineProps<{
  settings: Pick<
    TranslateSettings,
    "appTheme" | "bubbleOpacity" | "bubbleBackground" | "bubbleTextColor" | "bubbleMutedColor"
  >;
  localImagePath?: string | null;
}>();

const layouts = defineModel<BubbleBgLayouts>("layouts", { required: true });

const emit = defineEmits<{
  /** 鼠标在预览区内：父级应禁用设置面板滚轮滚动 */
  "hover-change": [inside: boolean];
}>();

const bgDataUrl = ref<string | null>(null);
const bgLoading = ref(false);
const activeVariant = ref<BubbleBgVariant | null>(null);
const dragging = ref(false);
const dragStart = ref({ x: 0, y: 0, layout: normalizeBgLayout() });
const loadingRef = ref<HTMLElement | null>(null);
const successRef = ref<HTMLElement | null>(null);
const wrapRef = ref<HTMLElement | null>(null);
const pointerInside = ref(false);

const canEditBg = computed(() => !!bgDataUrl.value && !bgLoading.value);

const appearance = computed(() => ({
  appTheme: props.settings.appTheme,
  bubbleOpacity: props.settings.bubbleOpacity,
  bubbleBgLayouts: layouts.value,
  bubbleTextColor: props.settings.bubbleTextColor,
  bubbleMutedColor: props.settings.bubbleMutedColor,
}));

function bubbleStyle(variant: BubbleBgVariant) {
  return bubbleCardStyle(appearance.value, bgDataUrl.value, variant);
}

function patchLayout(variant: BubbleBgVariant, layout: BubbleBgLayout) {
  layouts.value =
    variant === "loading"
      ? { ...layouts.value, loading: normalizeBgLayout(layout) }
      : { ...layouts.value, success: normalizeBgLayout(layout) };
}

function boxFor(variant: BubbleBgVariant) {
  return variant === "loading" ? loadingRef.value : successRef.value;
}

function variantFromTarget(target: EventTarget | null): BubbleBgVariant | null {
  if (!(target instanceof HTMLElement)) return null;
  if (target.closest(".preview-loading")) return "loading";
  if (target.closest(".preview-success")) return "success";
  return null;
}

function resolveWheelVariant(e: WheelEvent): BubbleBgVariant | null {
  if (activeVariant.value) return activeVariant.value;
  return variantFromTarget(e.target);
}

function onBubblePointerDown(variant: BubbleBgVariant, e: PointerEvent) {
  if (e.button !== 0) return;
  e.stopPropagation();
  activeVariant.value = variant;
  if (!canEditBg.value) return;
  e.preventDefault();
  dragging.value = true;
  const layout = variant === "loading" ? layouts.value.loading : layouts.value.success;
  dragStart.value = { x: e.clientX, y: e.clientY, layout: { ...layout } };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onBubblePointerMove(e: PointerEvent) {
  if (!dragging.value || !canEditBg.value || !activeVariant.value) return;
  e.preventDefault();
  const el = boxFor(activeVariant.value);
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const dx = e.clientX - dragStart.value.x;
  const dy = e.clientY - dragStart.value.y;
  const next = panLayoutByPixels(dragStart.value.layout, dx, dy, rect.width, rect.height);
  patchLayout(activeVariant.value, next);
}

function onBubblePointerUp(e: PointerEvent) {
  if (!dragging.value) return;
  dragging.value = false;
  try {
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  } catch {
    /* ignore */
  }
}

function onShellPointerDown(e: PointerEvent) {
  if (e.target === e.currentTarget) {
    activeVariant.value = null;
    dragging.value = false;
  }
}

function setPointerInside(inside: boolean) {
  pointerInside.value = inside;
  emit("hover-change", inside);
}

/** 捕获阶段拦截滚轮，避免带动设置面板滚动 */
function onWrapWheel(e: WheelEvent) {
  if (!pointerInside.value) return;
  e.preventDefault();
  e.stopPropagation();
  if (!canEditBg.value) return;
  const variant = resolveWheelVariant(e);
  if (!variant) return;
  activeVariant.value = variant;
  const layout = variant === "loading" ? layouts.value.loading : layouts.value.success;
  patchLayout(variant, zoomLayoutByWheel(layout, e.deltaY));
}

async function refreshBackground() {
  bgLoading.value = true;
  try {
    if (props.localImagePath) {
      bgDataUrl.value = await invoke<string | null>("read_image_data_url", {
        path: props.localImagePath,
      });
      return;
    }
    if (!props.settings.bubbleBackground?.trim()) {
      bgDataUrl.value = null;
      return;
    }
    bgDataUrl.value = await invoke<string | null>("get_bubble_background_data_url");
  } catch {
    bgDataUrl.value = null;
  } finally {
    bgLoading.value = false;
  }
}

watch(
  () => [props.settings.bubbleBackground, props.localImagePath],
  () => {
    void refreshBackground();
  },
  { immediate: true },
);

onMounted(() => {
  void refreshBackground();
  const wrap = wrapRef.value;
  if (!wrap) return;
  wrap.addEventListener("wheel", onWrapWheel, { passive: false, capture: true });
});

onBeforeUnmount(() => {
  wrapRef.value?.removeEventListener("wheel", onWrapWheel, { capture: true });
});
</script>

<template>
  <div
    ref="wrapRef"
    class="bubble-preview-wrap"
    @mouseenter="setPointerInside(true)"
    @mouseleave="setPointerInside(false)"
  >
    <div class="preview-shell" @pointerdown="onShellPointerDown">
      <div
        ref="loadingRef"
        class="preview-bubble preview-loading"
        :class="{
          selectable: true,
          active: activeVariant === 'loading',
          'can-edit-bg': canEditBg,
          dragging: dragging && activeVariant === 'loading',
        }"
        :style="bubbleStyle('loading')"
        @pointerdown="onBubblePointerDown('loading', $event)"
        @pointermove="onBubblePointerMove"
        @pointerup="onBubblePointerUp"
        @pointercancel="onBubblePointerUp"
      >
        <div class="preview-content">
          <div class="preview-top">
            <span class="prov">baidu</span>
            <span class="spin" aria-hidden="true" />
            <span class="preview-title">翻译中…</span>
          </div>
        </div>
        <span v-if="activeVariant === 'loading'" class="edit-badge">调整中</span>
      </div>

      <div
        ref="successRef"
        class="preview-bubble preview-success"
        :class="{
          selectable: true,
          active: activeVariant === 'success',
          'can-edit-bg': canEditBg,
          dragging: dragging && activeVariant === 'success',
        }"
        :style="bubbleStyle('success')"
        @pointerdown="onBubblePointerDown('success', $event)"
        @pointermove="onBubblePointerMove"
        @pointerup="onBubblePointerUp"
        @pointercancel="onBubblePointerUp"
      >
        <div class="preview-content">
          <div class="preview-top">
            <span class="prov">baidu</span>
            <span class="preview-btns">□ ×</span>
          </div>
          <div class="preview-line">
            <span class="k">原</span>
            <span class="v muted">Hello world</span>
          </div>
          <div class="preview-line">
            <span class="k">译</span>
            <span class="v tr">你好世界</span>
          </div>
        </div>
        <span v-if="activeVariant === 'success'" class="edit-badge">调整中</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bubble-preview-wrap {
  margin: 0 0 12px;
  overscroll-behavior: contain;
}

.preview-shell {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  border-radius: 8px;
  border: 1px dashed var(--border);
  background: rgba(0, 0, 0, 0.15);
}

.preview-bubble {
  position: relative;
  box-sizing: border-box;
  border-radius: 8px;
  padding: 6px 8px;
  font-family: "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
  overflow: hidden;
  user-select: none;
  touch-action: none;
}

.preview-bubble.selectable {
  cursor: pointer;
}

.preview-bubble.selectable.active {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.preview-bubble.can-edit-bg.active {
  cursor: grab;
}

.preview-bubble.can-edit-bg.dragging {
  cursor: grabbing;
}

.preview-content {
  position: relative;
  z-index: 1;
  pointer-events: none;
}

.preview-loading {
  min-height: 44px;
}

.preview-success {
  min-height: 108px;
  display: grid;
  grid-template-rows: auto 18px 1fr;
  gap: 4px;
}

.preview-top {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 20px;
}

.prov {
  font-size: 0.68rem;
  color: var(--accent);
  flex-shrink: 0;
}

.preview-title,
.preview-btns,
.k,
.v.muted {
  color: var(--bubble-muted);
}

.v.tr {
  color: var(--bubble-text);
}

.preview-title {
  flex: 1;
  font-size: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-btns {
  font-size: 0.65rem;
  letter-spacing: 2px;
}

.preview-line {
  display: grid;
  grid-template-columns: 14px 1fr;
  gap: 5px;
  align-items: start;
  min-width: 0;
}

.k {
  font-size: 0.65rem;
  font-weight: 600;
  line-height: 1.35;
}

.v {
  font-size: 0.74rem;
  line-height: 1.35;
  word-break: break-word;
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

.edit-badge {
  position: absolute;
  right: 6px;
  bottom: 4px;
  z-index: 2;
  font-size: 0.6rem;
  padding: 1px 5px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--accent);
  pointer-events: none;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
