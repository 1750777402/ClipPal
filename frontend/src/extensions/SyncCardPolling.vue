<template><!-- 状态由历史列表卡片展示。 --></template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { apiInvoke, isSuccess } from '../utils/api';
import { appCapabilities, loadAppCapabilities } from '../utils/capabilities';
import type { SyncCardStates } from '../types/global';

const props = defineProps<{ contentKeys: string[] }>();
const emit = defineEmits<{ change: [states: SyncCardStates] }>();

let stopped = false;
let pending = false;
let refreshRequested = false;
let timer: ReturnType<typeof setTimeout> | undefined;
let lastSnapshot = '';

function schedule(delay: number) {
  clearTimeout(timer);
  if (!stopped && !pending && appCapabilities.value?.cloud_sync) timer = setTimeout(refresh, delay);
}

async function refresh() {
  pending = true;
  let delay = 500;
  try {
    if (document.hidden || !await getCurrentWindow().isVisible()) {
      delay = 2500;
      return;
    }
    const response = await apiInvoke<SyncCardStates>('get_sync_card_states', {
      contentKeys: props.contentKeys,
    });
    if (!stopped && !refreshRequested && isSuccess(response)) {
      const snapshot = JSON.stringify(response.data);
      if (lastSnapshot !== snapshot) {
        lastSnapshot = snapshot;
        emit('change', response.data);
      }
    }
  } catch {
    delay = 2500;
  } finally {
    pending = false;
    const nextDelay = refreshRequested ? 0 : delay;
    refreshRequested = false;
    schedule(nextDelay);
  }
}

// 如果切换时已有请求在途，忽略旧快照并在它结束后立即补读。
const requestRefresh = () => {
  if (pending) refreshRequested = true;
  else schedule(0);
};
watch(() => appCapabilities.value?.cloud_sync, requestRefresh);
onMounted(() => {
  document.addEventListener('visibilitychange', requestRefresh);
  window.addEventListener('sync-card-refresh', requestRefresh);
  void loadAppCapabilities().then(requestRefresh);
});

onBeforeUnmount(() => {
  stopped = true;
  clearTimeout(timer);
  document.removeEventListener('visibilitychange', requestRefresh);
  window.removeEventListener('sync-card-refresh', requestRefresh);
});
</script>
