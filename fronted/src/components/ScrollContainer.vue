<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">Clip Pal</span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
      <div class="header-icons">
        <span class="iconfont icon-weitongbu" title="云同步"></span>
        <span class="iconfont icon-user" title="用户信息"></span>
        <span class="iconfont icon-setting" title="设置"></span>
        <!-- <img :src="sync_cloud" alt="云图标" class="icon-button" title="云同步" />
        <img :src="user_head" alt="用户信息" class="icon-button" title="用户信息" />
        <img :src="settings" alt="设置" class="icon-button" title="设置" @click="showSettings = true" /> -->
      </div>
    </header>

    <div v-if="isLoading" class="loading-container">
      <div class="loading-spinner"></div>
      <span class="loading-text">加载中...</span>
    </div>

    <div class="clip-list" v-else @scroll.passive="handleScroll" ref="scrollContainer">
      <ClipCard v-for="item in cards" :key="item.id" :record="item" :is-mobile="isMobile" @click="handleCardClick" />

      <div v-if="isFetchingMore" class="bottom-loading">
        <div class="loading-spinner small"></div>
        <span>加载更多...</span>
      </div>
      <div v-if="!hasMore && cards.length > 0" class="bottom-loading">没有更多了</div>
    </div>

    <SettingsDialog v-model="showSettings" @save="handleSettingsSave" />
  </div>
</template>


<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import { debounce } from 'lodash-es';
import SettingsDialog from './SettingsDialog.vue';
import ClipCard from './ClipCard.vue';

const search = ref('');
const isLoading = ref(false);
const isFetchingMore = ref(false);
const cards = ref<ClipRecord[]>([]);
const page = ref(1);
const pageSize = 20;
const hasMore = ref(true);

const scrollContainer = ref<HTMLElement | null>(null);

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
  fileSize?: number;
}

const handleCardClick = async (item: ClipRecord) => {
  await invoke('copy_clip_record', { param: { record_id: item.id } });
};

const initEventListeners = async () => {
  try {
    await listen('clip_record_change', () => {
      resetAndFetch();
    });
  } catch (error) {
    console.error('事件监听失败:', error);
  }
};

const resetAndFetch = () => {
  cards.value = [];
  page.value = 1;
  hasMore.value = true;
  fetchClipRecords();
};

const fetchClipRecords = async () => {
  if (!hasMore.value) return;
  const currentPage = page.value;
  try {
    if (currentPage === 1) isLoading.value = true;
    else isFetchingMore.value = true;

    const data: ClipRecord[] = await invoke('get_clip_records', {
      param: {
        page: currentPage,
        size: pageSize,
        search: search.value
      }
    });
    console.log('获取数据成功:{}', data);
    if (data.length < pageSize) hasMore.value = false;
    cards.value.push(...data);
    page.value++;
  } catch (error) {
    console.error('获取数据失败:', error);
  } finally {
    isLoading.value = false;
    isFetchingMore.value = false;
  }
};

const handleScroll = () => {
  if (!scrollContainer.value || isFetchingMore.value || !hasMore.value) return;

  const { scrollTop, clientHeight, scrollHeight } = scrollContainer.value;
  if (scrollTop + clientHeight >= scrollHeight - 200) {
    fetchClipRecords();
  }
};

watch(search, (_newValue, _oldValue) => {
  fetchClipRecordsDebounced();
})

const fetchClipRecordsDebounced = debounce(() => {
  cards.value = [];
  page.value = 1;
  hasMore.value = true;
  fetchClipRecords();
}, 300);

// 添加移动设备检测
const isMobile = ref(window.innerWidth <= 768);

// 监听窗口大小变化
const handleResize = debounce(() => {
  isMobile.value = window.innerWidth <= 768;
}, 200);

const showSettings = ref(false);

const handleSettingsSave = async (newSettings: any) => {
  // 处理设置保存
  console.log('设置已保存:', newSettings);
  // 这里可以添加其他处理逻辑，比如重新加载数据等
};

onMounted(() => {
  window.addEventListener('resize', handleResize);
  fetchClipRecords();
  initEventListeners();
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
});
</script>


<style scoped>
.clipboard-panel {
  width: 100%;
  height: 100%;
  position: fixed;
  top: 0;
  right: 0;
  background: var(--bg-color, #f5f7fa);
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--border-color, #d1d9e6);
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.05);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial;
  transition: background-color 0.3s ease;
}

.panel-header {
  padding: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: var(--header-bg, #2c7a7b);
  border-bottom: 1px solid var(--header-border, #256d6d);
  color: var(--header-text, #e6fffa);
  font-weight: 600;
  font-size: clamp(16px, 2vw, 18px);
  user-select: none;
  flex-shrink: 0;
  min-height: 0;
  position: sticky;
  top: 0;
  z-index: 10;
  backdrop-filter: blur(8px);
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
  font-size: 20px;
}

.search-input {
  flex: 1;
  padding: 8px 16px;
  border-radius: 12px;
  border: 1px solid var(--input-border, #88c0d0);
  font-size: 14px;
  background-color: var(--input-bg, #e0f2f1);
  color: var(--input-text, #004d40);
  transition: all 0.3s ease;
  min-width: 0;
}

.search-input::placeholder {
  color: #4a4a4aaa;
}

.search-input:focus {
  outline: none;
  border-color: var(--input-focus-border, #319795);
  box-shadow: 0 0 12px rgba(49, 151, 149, 0.2);
  background-color: var(--input-focus-bg, #ffffff);
  color: var(--input-focus-text, #222);
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  gap: 16px;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 3px solid var(--spinner-border, #e0f2f1);
  border-top-color: var(--spinner-color, #2c7a7b);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.loading-spinner.small {
  width: 24px;
  height: 24px;
  border-width: 2px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-text {
  color: var(--text-secondary, #666);
  font-size: 14px;
}

.clip-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, #81e6d9) transparent;
  box-sizing: border-box;
  min-height: 0;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
}

.clip-list::-webkit-scrollbar {
  width: 6px;
}

.clip-list::-webkit-scrollbar-thumb {
  background-color: var(--scrollbar-thumb, #81e6d9);
  border-radius: 3px;
}

.clip-list::-webkit-scrollbar-track {
  background: transparent;
}

.bottom-loading {
  padding: 16px;
  text-align: center;
  font-size: 14px;
  color: var(--text-secondary, #666);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.header-icons {
  display: flex;
  gap: 12px;
  align-items: center;
}
.header-icons span {
  font-size: 18px;
}

.icon-button {
  width: 24px;
  height: 24px;
  cursor: pointer;
  transition: transform 0.2s ease;
  opacity: 0.9;
}

.icon-button:hover {
  transform: scale(1.1);
  opacity: 1;
}

/* 响应式布局 */
@media (max-width: 768px) {
  .panel-header {
    padding: 12px;
    gap: 8px;
  }

  .search-input {
    padding: 6px 12px;
    font-size: 13px;
  }

  .clip-card {
    margin: 0 12px 12px 12px;
    padding: 12px;
  }

  .image-container {
    width: 140px;
    height: 100px;
  }

  .file-preview {
    max-width: 100%;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .clipboard-panel {
    --bg-color: #1a1a1a;
    --border-color: #2d2d2d;
    --header-bg: #1e3a3a;
    --header-border: #2c4a4a;
    --header-text: #e6fffa;
    --input-bg: #2d2d2d;
    --input-border: #3d3d3d;
    --input-text: #e6e6e6;
    --input-focus-bg: #333333;
    --input-focus-border: #4a9a9a;
    --input-focus-text: #ffffff;
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --image-bg: #333333;
    --placeholder-bg: #2d2d2d;
    --file-bg: #333333;
    --scrollbar-thumb: #4a9a9a;
    --spinner-border: #333333;
    --spinner-color: #4a9a9a;
    --icon-color: #e6fffa;
  }
}
</style>