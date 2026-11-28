<template>
  <slot :status-text="statusText" :checked="status?.requested_enabled ?? false"
    :disabled="!available || busy || !status" :set-enabled="setEnabled" />
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { apiInvoke, isSuccess } from '../utils/api';

const props = defineProps<{ available: boolean }>();
type TaskStatus = 'idle' | 'working' | 'retrying' | 'blocked';
interface SyncStatus {
  requested_enabled: boolean;
  enabled: boolean;
  waiting_for_limits: boolean;
  upload: TaskStatus;
  pull: TaskStatus;
  notifications: 'connecting' | 'connected' | 'retrying' | 'blocked';
}

const status = ref<SyncStatus | null>(null);
const busy = ref(false);
const errorMessage = ref('');
let changeVersion = 0;
const statusText = computed(() => {
  if (!props.available) return '此自建包未开启云同步部署配置';
  if (busy.value) return '正在切换同步状态';
  if (errorMessage.value) return errorMessage.value;
  const current = status.value;
  if (!current) return '正在读取状态';
  if (!current.requested_enabled) return '已关闭';
  if (!current.enabled) return '启动失败，请检查部署配置';
  if (current.upload === 'blocked' || current.pull === 'blocked') return '同步异常，待上传记录已保留';
  if (current.upload === 'retrying' || current.pull === 'retrying') return '连接或同步失败，等待重试';
  if (current.waiting_for_limits) return '正在连接同步服务';
  if (current.upload === 'working' || current.pull === 'working') return '正在同步';
  if (current.notifications === 'blocked') return '同步通知连接异常';
  if (current.notifications === 'retrying') return '同步通知已断开，正在重连';
  if (current.notifications === 'connecting') return '正在连接同步服务';
  return '已连接，自动同步';
});

/** 点击即保存独立的同步偏好。后台确认前继续显示上一次保存的值。 */
async function setEnabled(event: Event) {
  const input = event.target as HTMLInputElement;
  const requested = input.checked;
  input.checked = status.value?.requested_enabled ?? false;
  if (busy.value || !props.available || !status.value) return;
  busy.value = true;
  errorMessage.value = '';
  changeVersion += 1;
  try {
    const response = await apiInvoke<SyncStatus>('set_sync_enabled', { enabled: requested });
    if (isSuccess(response)) {
      status.value = response.data;
      window.dispatchEvent(new Event('sync-card-refresh'));
    } else {
      errorMessage.value = response.error || '云同步开关保存失败，请重试';
    }
  } finally {
    busy.value = false;
  }
}

// 设置窗口打开、打包配置支持同步时才查询。切换开关前发出的查询结果会被丢弃。
watch(() => props.available, (available, _, onCleanup) => {
  if (!available) {
    status.value = null;
    return;
  }
  let stopped = false;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => {
    stopped = true;
    clearTimeout(timeout);
  });
  const refresh = async () => {
    if (!busy.value) {
      const version = changeVersion;
      const response = await apiInvoke<SyncStatus>('get_sync_status');
      if (stopped) return;
      if (version === changeVersion && isSuccess(response)) status.value = response.data;
    }
    if (!stopped) timeout = setTimeout(refresh, 2000);
  };
  void refresh();
}, { immediate: true });
</script>
