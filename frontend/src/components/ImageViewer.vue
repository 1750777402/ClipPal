<template>
  <main class="viewer" @keydown="onKeydown">
    <header
      class="toolbar"
      :class="{ 'macos-toolbar': isMacOS }"
      :data-tauri-drag-region="isMacOS ? 'deep' : undefined"
      aria-label="图片查看工具"
    >
      <div class="controls">
        <div class="control-group zoom-controls">
          <button type="button" :disabled="!ready" title="缩小 (-)" aria-label="缩小" @click="zoomBy(1 / 1.25)">−</button>
          <output aria-label="缩放比例">{{ ready ? zoomPercent + '%' : '—' }}</output>
          <button type="button" :disabled="!ready" title="放大 (+)" aria-label="放大" @click="zoomBy(1.25)">+</button>
        </div>
        <div class="control-group">
          <button type="button" :disabled="!ready" :aria-pressed="mode === 'fit'" title="适应窗口 (0)" @click="resetView('fit')">适应窗口</button>
          <button type="button" :disabled="!ready" :aria-pressed="mode === 'actual'" title="原始像素 (1)" aria-label="原始大小" @click="resetView('actual')">1:1</button>
        </div>
      </div>
      <span v-if="ready" class="dimensions">{{ imageWidth }} × {{ imageHeight }}</span>
    </header>
    <div
      ref="stage"
      class="stage"
      :class="{ pannable: canPan, dragging: isDragging }"
      tabindex="0"
      aria-label="图片查看区域"
      @wheel.prevent="onWheel"
      @pointerdown="startDrag"
      @pointermove="moveDrag"
      @pointerup="endDrag"
      @pointercancel="endDrag"
      @lostpointercapture="endDrag"
    >
      <img
        v-if="source && !error"
        :key="revision"
        :data-revision="revision"
        :src="source"
        :class="{ ready }"
        :style="imageStyle"
        alt="剪贴板图片"
        draggable="false"
        @load="onImageLoad"
        @error="onImageError"
      />
      <div v-if="error" class="status" role="alert">{{ error }}</div>
      <div v-else-if="!ready" class="status" role="status">正在加载图片…</div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { apiInvoke, isSuccess } from '../utils/api';
import { detectPlatform } from '../utils/responsive';

interface ViewerImage {
  revision: number;
  filePath: string;
}

type ViewMode = 'fit' | 'actual' | 'custom';

const isMacOS = detectPlatform() === 'macos';
const stage = ref<HTMLElement | null>(null);
const source = ref('');
const error = ref('');
const revision = ref(0);
const imageWidth = ref(0);
const imageHeight = ref(0);
const stageWidth = ref(0);
const stageHeight = ref(0);
const pixelRatio = ref(window.devicePixelRatio || 1);
const scale = ref(1);
const x = ref(0);
const y = ref(0);
const mode = ref<ViewMode>('fit');
const isDragging = ref(false);
let drag: { id: number; startX: number; startY: number; x: number; y: number } | undefined;
let unlisten: UnlistenFn | undefined;
let observer: ResizeObserver | undefined;
let resolution: MediaQueryList | undefined;
let disposed = false;

const ready = computed(() => imageWidth.value > 0 && imageHeight.value > 0 && !error.value);
const zoomPercent = computed(() => Math.round(scale.value * pixelRatio.value * 100));
const canPan = computed(() => ready.value && (
  imageWidth.value * scale.value > stageWidth.value || imageHeight.value * scale.value > stageHeight.value
));
const imageStyle = computed(() => ({
  transform: 'translate(' + x.value + 'px, ' + y.value + 'px) scale(' + scale.value + ')',
}));

// 放不满视口的方向保持居中，超出视口的方向可以拖到图片边缘。
function constrain(offset: number, viewport: number, content: number) {
  return content <= viewport ? (viewport - content) / 2 : Math.max(viewport - content, Math.min(0, offset));
}

function position(left: number, top: number) {
  x.value = constrain(left, stageWidth.value, imageWidth.value * scale.value);
  y.value = constrain(top, stageHeight.value, imageHeight.value * scale.value);
}

function fitScale() {
  return Math.min(1 / pixelRatio.value, stageWidth.value / imageWidth.value, stageHeight.value / imageHeight.value);
}

function resetView(nextMode: 'fit' | 'actual') {
  if (!ready.value || !stageWidth.value || !stageHeight.value) return;
  mode.value = nextMode;
  scale.value = nextMode === 'fit' ? fitScale() : 1 / pixelRatio.value;
  position(
    (stageWidth.value - imageWidth.value * scale.value) / 2,
    (stageHeight.value - imageHeight.value * scale.value) / 2,
  );
}

function zoomBy(factor: number, anchorX = stageWidth.value / 2, anchorY = stageHeight.value / 2) {
  if (!ready.value || !stageWidth.value || !stageHeight.value) return;
  const minScale = Math.min(fitScale(), 0.05 / pixelRatio.value);
  const nextScale = Math.max(minScale, Math.min(8 / pixelRatio.value, scale.value * factor));
  if (nextScale === scale.value) return;
  const ratio = nextScale / scale.value;
  mode.value = 'custom';
  scale.value = nextScale;
  position(anchorX - (anchorX - x.value) * ratio, anchorY - (anchorY - y.value) * ratio);
}

function measureStage() {
  if (!stage.value) return;
  const previousWidth = stageWidth.value;
  const previousHeight = stageHeight.value;
  const previousPixelRatio = pixelRatio.value;
  stageWidth.value = stage.value.clientWidth;
  stageHeight.value = stage.value.clientHeight;
  pixelRatio.value = window.devicePixelRatio || 1;
  if (mode.value === 'fit' || (mode.value === 'actual' && pixelRatio.value !== previousPixelRatio)) {
    resetView(mode.value);
  } else if (ready.value) {
    position(x.value + (stageWidth.value - previousWidth) / 2, y.value + (stageHeight.value - previousHeight) / 2);
  }
}

function watchResolution() {
  resolution?.removeEventListener('change', watchResolution);
  measureStage();
  resolution = window.matchMedia('(resolution: ' + pixelRatio.value + 'dppx)');
  resolution.addEventListener('change', watchResolution);
}

function onWheel(event: WheelEvent) {
  const rect = stage.value?.getBoundingClientRect();
  if (!rect) return;
  const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? rect.height : 1;
  zoomBy(Math.exp(-event.deltaY * unit * 0.002), event.clientX - rect.left, event.clientY - rect.top);
}

function startDrag(event: PointerEvent) {
  if (!canPan.value || event.button !== 0 || drag || !stage.value) return;
  stage.value.setPointerCapture(event.pointerId);
  drag = { id: event.pointerId, startX: event.clientX, startY: event.clientY, x: x.value, y: y.value };
  isDragging.value = true;
}

function moveDrag(event: PointerEvent) {
  if (drag?.id !== event.pointerId) return;
  position(drag.x + event.clientX - drag.startX, drag.y + event.clientY - drag.startY);
}

function endDrag(event?: PointerEvent) {
  if (!drag || (event && drag.id !== event.pointerId)) return;
  const id = drag.id;
  drag = undefined;
  isDragging.value = false;
  if (stage.value?.hasPointerCapture(id)) stage.value.releasePointerCapture(id);
}

function onKeydown(event: KeyboardEvent) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  switch (event.key) {
    case 'Escape':
      void getCurrentWindow().close().catch(() => { error.value = '关闭图片窗口失败'; });
      break;
    case '+':
    case '=':
      zoomBy(1.25);
      break;
    case '-':
      zoomBy(1 / 1.25);
      break;
    case '0':
      resetView('fit');
      break;
    case '1':
      resetView('actual');
      break;
    default:
      return;
  }
  event.preventDefault();
}

function showImage(image: ViewerImage) {
  if (disposed || image.revision <= revision.value) return;
  endDrag();
  revision.value = image.revision;
  imageWidth.value = 0;
  imageHeight.value = 0;
  error.value = '';
  mode.value = 'fit';
  source.value = convertFileSrc(image.filePath);
}

function onImageLoad(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  if (Number(image.dataset.revision) !== revision.value) return;
  imageWidth.value = image.naturalWidth;
  imageHeight.value = image.naturalHeight;
  measureStage();
}

function onImageError(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  if (Number(image.dataset.revision) === revision.value) {
    error.value = '无法读取图片，文件可能已经被移除。';
  }
}

onMounted(async () => {
  stage.value?.focus();
  observer = new ResizeObserver(measureStage);
  if (stage.value) observer.observe(stage.value);
  watchResolution();
  try {
    const stop = await listen<ViewerImage>('image-viewer-changed', ({ payload }) => showImage(payload), {
      target: { kind: 'WebviewWindow', label: getCurrentWindow().label },
    });
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
    const result = await apiInvoke<ViewerImage>('get_image_viewer_image');
    if (isSuccess(result)) {
      showImage(result.data);
    } else if (!disposed && !revision.value) {
      error.value = result.error || '无法读取预览图片。';
    }
  } catch {
    if (!disposed) error.value = '无法打开图片预览。';
  }
});

onUnmounted(() => {
  disposed = true;
  unlisten?.();
  observer?.disconnect();
  resolution?.removeEventListener('change', watchResolution);
});
</script>

<style>
:root {
  color-scheme: dark;
}

body {
  margin: 0;
  background: #191b1e;
  color: #edf0f3;
  font: 14px system-ui, sans-serif;
}
</style>

<style scoped>
.viewer {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  box-sizing: border-box;
  min-height: 44px;
  padding: 4px 12px;
  background: #23262a;
  border-bottom: 1px solid #34383e;
  user-select: none;
}

.macos-toolbar {
  /* 为叠加在页面上的原生窗口按钮留出空间。 */
  padding-inline-start: 92px;
}

.controls,
.control-group {
  display: flex;
  align-items: center;
}

.controls {
  grid-column: 2;
  gap: 8px;
}

.control-group {
  gap: 2px;
  padding: 2px;
  border: 1px solid #393e44;
  border-radius: 8px;
  background: #1d2023;
}

button {
  min-width: 30px;
  min-height: 28px;
  padding: 2px 8px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 13px;
  white-space: nowrap;
}

.zoom-controls button {
  font-size: 17px;
  line-height: 1;
}

button:enabled {
  cursor: pointer;
}

button:enabled:hover {
  background: #343941;
}

button[aria-pressed='true'] {
  background: #284442;
  color: #c9f3eb;
}

button:disabled {
  opacity: 0.55;
}

button:focus-visible {
  outline: 2px solid #9dd6cc;
  outline-offset: 2px;
}

output {
  min-width: 4ch;
  color: #c5cbd3;
  font-size: 12px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.dimensions {
  grid-column: 3;
  justify-self: end;
  color: #aeb6c0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

@media (max-width: 560px) {
  .dimensions {
    display: none;
  }
}

.stage {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: clip;
  outline: none;
  touch-action: none;
  user-select: none;
}

.stage.pannable {
  cursor: grab;
}

.stage.dragging {
  cursor: grabbing;
}

img {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: top left;
  visibility: hidden;
  pointer-events: none;
}

img.ready {
  visibility: visible;
}

.status {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  padding: 20px;
  color: #bcc7c7;
  text-align: center;
}
</style>
